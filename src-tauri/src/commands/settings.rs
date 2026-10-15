use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use super::search::{provider_info, ProviderInfo};
use crate::cache::{cache_stats, prepare_cache_dir};
use crate::error::AppResult;
use crate::models::CacheStats;
use crate::settings::normalize_api_key;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSettingsView {
    #[serde(flatten)]
    pub info: ProviderInfo,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub cache_dir: String,
    pub default_cache_dir: String,
    pub providers: Vec<ProviderSettingsView>,
}

fn settings_view(state: &AppState) -> SettingsView {
    let providers = state
        .providers
        .iter()
        .map(|provider| ProviderSettingsView {
            info: provider_info(state, provider.as_ref()),
            api_key: state.settings.provider(provider.id()).api_key,
        })
        .collect();
    SettingsView {
        cache_dir: state.settings.cache_dir().display().to_string(),
        default_cache_dir: state.settings.default_cache_dir().display().to_string(),
        providers,
    }
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> AppResult<SettingsView> {
    Ok(settings_view(&state))
}

#[tauri::command]
pub async fn update_provider_settings(
    state: State<'_, AppState>,
    provider_id: String,
    enabled: bool,
    api_key: Option<String>,
) -> AppResult<SettingsView> {
    state.providers.require(&provider_id)?;
    let api_key = normalize_api_key(api_key);
    state.settings.update(|settings| {
        let provider = settings.providers.entry(provider_id).or_default();
        provider.enabled = enabled;
        provider.api_key = api_key;
    })?;
    Ok(settings_view(&state))
}

/// Changes where new downloads are stored. `None` restores the default
/// location. Files already downloaded stay where they are.
#[tauri::command]
pub async fn set_cache_dir(
    app: AppHandle,
    state: State<'_, AppState>,
    path: Option<String>,
) -> AppResult<SettingsView> {
    let custom = path.map(|path| PathBuf::from(path.trim())).filter(|path| !path.as_os_str().is_empty());
    let target = custom.clone().unwrap_or_else(|| state.settings.default_cache_dir().to_path_buf());
    prepare_cache_dir(&target)?;
    app.asset_protocol_scope().allow_directory(&target, true)?;
    let is_default = target == state.settings.default_cache_dir();
    state.settings.update(|settings| {
        settings.cache_dir = if is_default { None } else { custom };
    })?;
    Ok(settings_view(&state))
}

#[tauri::command]
pub async fn get_cache_stats(state: State<'_, AppState>) -> AppResult<CacheStats> {
    cache_stats(state.settings.cache_dir()).await
}
