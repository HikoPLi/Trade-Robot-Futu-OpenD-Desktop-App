mod commands;
mod state;

use commands::*;
use state::AppState;
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use trader_core::paths;
use trader_shared::EngineEvent;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let engine = tauri::async_runtime::block_on(async { trader_core::EngineHandle::new().await })?;
            let state = AppState { engine: engine.clone() };
            app.manage(state);

            // Forward engine events to the frontend as Tauri events.
            let app_handle = app.handle().clone();
            let mut rx = engine.subscribe_events();
            tauri::async_runtime::spawn(async move {
                loop {
                    match rx.recv().await {
                        Ok(evt) => {
                            let _ = app_handle.emit("engine_event", &evt);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            let _ = app_handle.emit(
                                "engine_event",
                                &EngineEvent::Info {
                                    message: "UI lagged; some events were dropped".to_string(),
                                },
                            );
                        }
                    }
                }
            });

            // Register global kill-switch hotkey from active profile (best-effort).
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let snap = engine.snapshot().await;
                let hotkey = snap.active_profile.kill_switch_hotkey;
                let Ok(shortcut) = hotkey.parse::<tauri_plugin_global_shortcut::Shortcut>() else {
                    tracing::warn!(hotkey = %hotkey, "invalid hotkey string");
                    return;
                };
                let mgr = app_handle.global_shortcut();
                let engine2 = engine.clone();
                if let Err(e) = mgr.on_shortcut(shortcut, move |_app, _shortcut, event| {
                    if event.state != tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        return;
                    }
                    let engine3 = engine2.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = engine3
                            .engage_kill_switch("Global kill-switch hotkey engaged".to_string())
                            .await;
                    });
                }) {
                    tracing::warn!(err = %e, "failed to register global shortcut");
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            engine_snapshot,
            engine_set_watchlist,
            engine_get_candles,
            engine_place_order,
            engine_cancel_order,
            engine_start_strategy,
            engine_stop_strategy,
            engine_engage_kill_switch,
            engine_update_risk_limits,
            engine_update_opend_config,
            engine_update_opend_trade_env,
            engine_update_time_controls,
            engine_test_opend_connection,
            engine_secret_status,
            engine_set_secret,
            engine_clear_secret,
            engine_generate_sample_candles_csv,
            engine_set_kill_switch_hotkey,
            engine_set_active_profile,
            engine_list_audit_events,
            engine_export_audit_jsonl,
            engine_run_backtest,
            strategy_catalog,
            model_catalog,
            engine_list_strategy_defs,
            engine_upsert_strategy_def,
            engine_delete_strategy_def,
            engine_start_strategy_def,
            engine_list_models,
            engine_register_model,
            engine_delete_model,
            engine_evaluate_model,
            enable_live_trading_unlock,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                // Best-effort: clear crash marker on clean shutdown to avoid safe mode next startup.
                if let Ok(p) = paths::app_paths() {
                    let _ = std::fs::remove_file(p.crash_marker_path);
                }
            }
        });
}
