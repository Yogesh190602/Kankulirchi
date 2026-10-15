use serde::Serialize;
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::models::SearchResponse;
use crate::providers::{
    safe_search, search_providers, ApiKeyRequirement, ProviderMetadata, SearchTarget, WallpaperProvider,
};
use crate::settings::ProviderSettings;
use crate::state::AppState;

const MAX_QUERY_LENGTH: usize = 200;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    #[serde(flatten)]
    pub metadata: ProviderMetadata,
    pub enabled: bool,
    pub has_api_key: bool,
    /// Whether the provider can be searched with the current settings.
    pub ready: bool,
}

/// Whether the provider is turned on and has the API key it needs.
fn is_ready(provider: &dyn WallpaperProvider, settings: &ProviderSettings) -> bool {
    settings.enabled
        && (provider.metadata().api_key_requirement != ApiKeyRequirement::Required || settings.api_key.is_some())
}

pub(crate) fn provider_info(state: &AppState, provider: &dyn WallpaperProvider) -> ProviderInfo {
    let settings = state.settings.provider(provider.id());
    ProviderInfo {
        metadata: provider.metadata().clone(),
        enabled: settings.enabled,
        has_api_key: settings.api_key.is_some(),
        ready: is_ready(provider, &settings),
    }
}

#[tauri::command]
pub async fn list_providers(state: State<'_, AppState>) -> AppResult<Vec<ProviderInfo>> {
    Ok(state.providers.iter().map(|provider| provider_info(&state, provider.as_ref())).collect())
}

/// Searches the given providers, or every ready provider when the list is
/// empty. Searching everything skips providers that still need an API key,
/// so their error does not appear with every search; asking for one by name
/// reports what is missing. Results already in the library carry their id,
/// favorite flag and local file.
#[tauri::command]
pub async fn search_wallpapers(
    state: State<'_, AppState>,
    query: String,
    providers: Vec<String>,
    page: u32,
) -> AppResult<SearchResponse> {
    let keyword = query.trim();
    if keyword.is_empty() {
        return Err(AppError::Validation("Type a keyword to search for wallpapers.".into()));
    }
    if keyword.chars().count() > MAX_QUERY_LENGTH {
        return Err(AppError::Validation(format!("Search keywords can be up to {MAX_QUERY_LENGTH} characters.")));
    }
    safe_search::check_keyword(keyword)?;
    let page = page.max(1);

    let targets: Vec<SearchTarget> = state
        .providers
        .iter()
        .filter_map(|provider| {
            let settings = state.settings.provider(provider.id());
            let wanted = if providers.is_empty() {
                is_ready(provider.as_ref(), &settings)
            } else {
                settings.enabled && providers.iter().any(|id| id == provider.id())
            };
            wanted.then(|| SearchTarget { provider: provider.clone(), api_key: settings.api_key })
        })
        .collect();
    if targets.is_empty() {
        return Err(AppError::Validation(
            "No provider is ready to search. Turn one on in Settings, or add the API key it needs.".into(),
        ));
    }

    let target_count = targets.len();
    let result = search_providers(targets, keyword, page).await;
    if result.errors.len() == target_count {
        let message = result.errors.iter().map(|error| error.message.as_str()).collect::<Vec<_>>().join(" ");
        return Err(AppError::Provider(message));
    }

    let mut wallpapers = result.wallpapers;
    state.db.annotate(&mut wallpapers)?;
    Ok(SearchResponse { wallpapers, page, providers_with_more: result.providers_with_more, errors: result.errors })
}
