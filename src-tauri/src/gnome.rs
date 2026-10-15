//! GNOME settings, read and written through `gsettings`.

use std::io;

use tokio::process::Command;

use crate::error::{AppError, AppResult};

/// Runs `gsettings` and returns what it printed.
pub async fn gsettings(args: &[&str]) -> AppResult<String> {
    let output = Command::new("gsettings").args(args).output().await.map_err(|error| match error.kind() {
        io::ErrorKind::NotFound => {
            AppError::Wallpaper("gsettings was not found. Install glib2 and run Kankulirchi in a GNOME session.".into())
        }
        _ => AppError::Wallpaper(format!("Could not run gsettings: {error}")),
    })?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(AppError::Wallpaper(format!("GNOME did not accept the change: {}", stderr.trim())))
    }
}

/// A setting's value as GVariant text, such as `'zoom'`, or `None` if this
/// GNOME version does not have the setting.
pub async fn get(schema: &str, key: &str) -> Option<String> {
    gsettings(&["get", schema, key]).await.ok().map(|value| value.trim().to_owned())
}

/// Sets a setting from GVariant text.
pub async fn set(schema: &str, key: &str, value: &str) -> AppResult<()> {
    gsettings(&["set", schema, key, value]).await.map(drop)
}

/// Returns a setting to GNOME's default.
pub async fn reset(schema: &str, key: &str) -> AppResult<()> {
    gsettings(&["reset", schema, key]).await.map(drop)
}

/// Whether the setting exists and can be changed.
pub async fn writable(schema: &str, key: &str) -> bool {
    gsettings(&["writable", schema, key]).await.is_ok_and(|output| output.trim() == "true")
}

/// Serializes `value` as a GVariant string literal, the format `gsettings
/// set` expects.
pub fn gvariant_string(value: &str) -> String {
    let mut literal = String::with_capacity(value.len() + 2);
    literal.push('\'');
    for c in value.chars() {
        match c {
            '\\' => literal.push_str("\\\\"),
            '\'' => literal.push_str("\\'"),
            c => literal.push(c),
        }
    }
    literal.push('\'');
    literal
}

/// The text of a GVariant string literal printed by `gsettings get`, or
/// `None` if `literal` is not one.
pub fn parse_gvariant_string(literal: &str) -> Option<String> {
    let inner = literal.trim().strip_prefix('\'')?.strip_suffix('\'')?;
    let mut text = String::with_capacity(inner.len());
    let mut characters = inner.chars();
    while let Some(c) = characters.next() {
        if c == '\\' {
            text.push(characters.next()?);
        } else {
            text.push(c);
        }
    }
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_gvariant_strings() {
        assert_eq!(gvariant_string("file:///a/b.jpg"), "'file:///a/b.jpg'");
        assert_eq!(gvariant_string("file:///it's/a\\b"), "'file:///it\\'s/a\\\\b'");
    }

    #[test]
    fn gvariant_strings_round_trip() {
        for text in ["file:///a/b.jpg", "it's", "a\\b", ""] {
            assert_eq!(parse_gvariant_string(&gvariant_string(text)).as_deref(), Some(text));
        }
        assert_eq!(parse_gvariant_string("@as []"), None);
    }
}
