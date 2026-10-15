use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

/// Every fallible operation in the backend returns this error type. It is
/// serialized as `{ kind, message }` so the frontend can show a readable
/// message and branch on the kind when it needs to.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("{}", describe_network_error(.0))]
    Http(reqwest::Error),

    #[error("File system error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Unexpected data: {0}")]
    Json(#[from] serde_json::Error),

    #[error("The downloaded file is not a usable image: {0}")]
    Image(#[from] image::ImageError),

    #[error("{0}")]
    Provider(String),

    #[error("{0}")]
    NotFound(String),

    #[error("{0}")]
    Validation(String),

    #[error("{0}")]
    Wallpaper(String),

    #[error("{0}")]
    Internal(String),
}

impl AppError {
    pub fn kind(&self) -> &'static str {
        match self {
            AppError::Database(_) => "database",
            AppError::Http(_) => "network",
            AppError::Io(_) => "io",
            AppError::Json(_) => "data",
            AppError::Image(_) => "image",
            AppError::Provider(_) => "provider",
            AppError::NotFound(_) => "notFound",
            AppError::Validation(_) => "validation",
            AppError::Wallpaper(_) => "wallpaper",
            AppError::Internal(_) => "internal",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("AppError", 2)?;
        state.serialize_field("kind", self.kind())?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

/// What could not be reached, and why when it is a common cause.
fn describe_network_error(error: &reqwest::Error) -> String {
    let host = error.url().and_then(|url| url.host_str());
    // Errors reading a response body do not always say where it came from.
    let (server, the_server) = host.map_or(("The server", "the server"), |host| (host, host));
    if error.is_connect() {
        format!("Could not connect to {the_server}. Check your internet connection.")
    } else if error.is_timeout() {
        format!("{server} took too long to respond. Try again later.")
    } else if let Some(status) = error.status() {
        format!("{server} answered with an error ({status}).")
    } else if is_unreadable_data(error) {
        format!("{server} sent a response Kankulirchi could not read.")
    } else if error.is_body() || error.is_decode() {
        format!("The connection to {the_server} was interrupted. Try again.")
    } else {
        format!("Network error: {error}")
    }
}

/// Whether a response arrived but was not the JSON expected. Other errors
/// reading a response mean the connection dropped partway through: no
/// response is compressed, so nothing else can go wrong while reading one.
fn is_unreadable_data(error: &reqwest::Error) -> bool {
    let mut source = std::error::Error::source(error);
    while let Some(inner) = source {
        if inner.is::<serde_json::Error>() {
            return true;
        }
        source = inner.source();
    }
    false
}

impl From<reqwest::Error> for AppError {
    /// Drops the query string from the URL that network errors quote, since
    /// it can hold an API key, so the key never reaches a log or the screen.
    fn from(mut error: reqwest::Error) -> Self {
        if let Some(url) = error.url_mut() {
            url.set_query(None);
        }
        AppError::Http(error)
    }
}

impl From<tauri::Error> for AppError {
    fn from(error: tauri::Error) -> Self {
        AppError::Internal(error.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn network_errors_do_not_quote_query_strings() {
        // Port 9 (discard) on localhost refuses the connection at once.
        let error =
            AppError::from(reqwest::get("http://127.0.0.1:9/api/search?q=lake&apikey=SECRET123").await.unwrap_err());
        let AppError::Http(inner) = &error else { panic!("expected a network error") };
        assert_eq!(inner.url().map(|url| url.as_str()), Some("http://127.0.0.1:9/api/search"));
        assert!(!format!("{error} {error:?}").contains("SECRET123"));
    }

    /// Serves one response that promises 1000 bytes and hangs up after 10.
    fn cut_off_server() -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            let (mut stream, _) = listener.accept().unwrap();
            let _ = stream.read(&mut [0; 1024]);
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\n0123456789").unwrap();
        });
        format!("http://{address}/image.jpg")
    }

    #[tokio::test]
    async fn cut_off_responses_say_the_connection_was_interrupted() {
        let response = reqwest::get(cut_off_server()).await.unwrap();
        let address = response.url().clone();
        let error = AppError::from(response.bytes().await.unwrap_err().with_url(address));
        assert_eq!(error.to_string(), "The connection to 127.0.0.1 was interrupted. Try again.");
    }

    #[tokio::test]
    async fn unexpected_responses_are_not_mistaken_for_interruptions() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            let (mut stream, _) = listener.accept().unwrap();
            let _ = stream.read(&mut [0; 1024]);
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\n<html></html>").unwrap();
        });
        let response = reqwest::get(format!("http://{address}/api")).await.unwrap();
        let error = AppError::from(response.json::<serde_json::Value>().await.unwrap_err());
        assert_eq!(error.to_string(), "The server sent a response Kankulirchi could not read.");
    }

    #[tokio::test]
    async fn connection_failures_suggest_checking_the_connection() {
        let error = AppError::from(reqwest::get("http://127.0.0.1:9/").await.unwrap_err());
        assert_eq!(error.to_string(), "Could not connect to 127.0.0.1. Check your internet connection.");
    }
}
