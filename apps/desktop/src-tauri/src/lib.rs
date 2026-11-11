//! Punto de entrada de la app de escritorio. Registra plugins, estado y comandos.

mod commands;
mod detect;
mod models;
mod recording;
mod state;

use tauri::Manager;

pub fn run() {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        // El aviso de reunión se coloca solo; restaurarlo lo enseñaría y activaría la app.
        .plugin(tauri_plugin_window_state::Builder::new().with_denylist(&[detect::WINDOW]).build())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("juanola.sqlite");
            let db = tauri::async_runtime::block_on(juanola_db::Db::open(&db_path))?;
            let settings = tauri::async_runtime::block_on(async {
                let get = |k| juanola_db::repo::settings::get::<bool>(&db, k);
                detect::DetectionSettings {
                    meetings: get(detect::KEY_MEETINGS).await.ok().flatten().unwrap_or(true),
                    calendar_reminders: get(detect::KEY_REMINDERS).await.ok().flatten().unwrap_or(true),
                }
            });
            let state = state::AppState::new(db, data_dir, settings);
            match state.models.status() {
                Ok(models) => {
                    for m in models {
                        tracing::info!(slot = m.slot, installed = m.installed, dir = %m.dir.display(), "modelo");
                    }
                }
                Err(e) => tracing::warn!(error = %e, "estado de los modelos"),
            }
            app.manage(state);
            detect::spawn(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::list_notes,
            commands::create_note,
            commands::get_note,
            commands::set_note_title,
            commands::set_note_raw_doc,
            commands::set_note_enhanced_doc,
            commands::open_privacy_settings,
            commands::mic_permission_status,
            commands::frontend_error,
            commands::reveal_data_dir,
            commands::models_status,
            commands::install_models,
            commands::cancel_model_install,
            commands::trash_note,
            commands::restore_note,
            commands::list_trashed_notes,
            commands::delete_note_forever,
            commands::list_folders,
            commands::create_folder,
            commands::rename_folder,
            commands::delete_folder,
            commands::list_folder_notes,
            commands::get_note_folder,
            commands::set_note_folder,
            commands::autotest_config,
            commands::autotest_log,
            commands::autotest_say,
            commands::autotest_play,
            commands::autotest_env,
            commands::autotest_quit,
            commands::list_segments,
            commands::start_recording,
            commands::pause_recording,
            commands::resume_recording,
            commands::stop_recording,
            commands::enhance_note,
            commands::list_audio_devices,
            commands::coming_up,
            commands::calendar_access,
            commands::calendar_diagnostics,
            commands::request_calendar_access,
            commands::open_event_note,
            commands::note_event,
            commands::stt_glossary,
            commands::set_stt_glossary,
            commands::detection_settings,
            commands::set_detection_settings,
            commands::notification_payload,
            commands::notification_take_notes,
            commands::notification_dismiss,
            commands::simulate_detection,
        ])
        .build(tauri::generate_context!())
        .expect("error al arrancar Juanola")
        .run(|app, event| {
            // Al salir con el aviso abierto: devolverle su clase antes de que
            // tao destruya las ventanas (ver `detect::float_over_fullscreen`).
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit = event {
                use tauri::Manager;
                if let Some(w) = app.get_webview_window(detect::WINDOW) {
                    detect::restore_class(&w);
                }
            }
            if let tauri::RunEvent::Exit = event {
                use tauri::Manager;
                app.state::<state::AppState>().unload_llm();
            }
        });
}
