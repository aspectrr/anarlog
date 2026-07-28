use std::str::FromStr;

use hypr_transcription_core::listener2 as core;
use tauri_specta::Event;

use crate::TranscriptionParams;
use crate::listener2::Listener2PluginExt;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum DiarizeDownloadEvent {
    #[serde(rename = "progress")]
    Progress { percentage: u8 },
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed { error: String },
}

#[tauri::command]
#[specta::specta]
pub async fn start_transcription<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    params: TranscriptionParams,
) -> Result<(), String> {
    app.listener2()
        .start_transcription(params)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn stop_transcription<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
) -> Result<(), String> {
    app.listener2().stop_transcription(session_id).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn parse_subtitle<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    path: String,
) -> Result<core::Subtitle, String> {
    app.listener2().parse_subtitle(path)
}

#[tauri::command]
#[specta::specta]
pub async fn export_to_vtt<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
    words: Vec<core::VttWord>,
) -> Result<String, String> {
    app.listener2().export_to_vtt(session_id, words)
}

#[tauri::command]
#[specta::specta]
pub async fn run_denoise<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    params: core::DenoiseParams,
) -> Result<(), String> {
    app.listener2()
        .run_denoise(params)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn is_supported_languages_batch<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
    provider: String,
    model: Option<String>,
    languages: Vec<String>,
) -> Result<bool, String> {
    let languages_parsed = languages
        .iter()
        .map(|s| hypr_language::Language::from_str(s))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("unknown_language: {}", e))?;

    core::is_supported_languages_batch(&provider, model.as_deref(), &languages_parsed)
}

#[tauri::command]
#[specta::specta]
pub async fn suggest_providers_for_languages_batch<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
    languages: Vec<String>,
) -> Result<Vec<String>, String> {
    let languages_parsed = languages
        .iter()
        .map(|s| hypr_language::Language::from_str(s))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("unknown_language: {}", e))?;

    Ok(core::suggest_providers_for_languages_batch(
        &languages_parsed,
    ))
}

#[tauri::command]
#[specta::specta]
pub async fn list_documented_language_codes_batch<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
) -> Result<Vec<String>, String> {
    Ok(core::list_documented_language_codes_batch())
}

#[tauri::command]
#[specta::specta]
pub async fn is_diarize_models_downloaded() -> Result<bool, String> {
    Ok(diarize::model_paths::models_exist())
}

#[tauri::command]
#[specta::specta]
pub async fn download_diarize_models<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<(), String> {
    let app_handle = app.clone();
    tokio::spawn(async move {
        let progress: diarize::download::ProgressFn = Box::new(move |pct: u8| {
            let _ = DiarizeDownloadEvent::Progress { percentage: pct }.emit(&app_handle);
        });

        match diarize::download::download_models(Some(progress)).await {
            Ok(()) => {
                let _ = DiarizeDownloadEvent::Completed.emit(&app);
            }
            Err(e) => {
                let _ = DiarizeDownloadEvent::Failed {
                    error: e.to_string(),
                }
                .emit(&app);
            }
        }
    });
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_diarize_models() -> Result<(), String> {
    diarize::download::delete_models().map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn diarize_models_size_bytes() -> Result<u64, String> {
    Ok(diarize::model_paths::TOTAL_SIZE_BYTES)
}
