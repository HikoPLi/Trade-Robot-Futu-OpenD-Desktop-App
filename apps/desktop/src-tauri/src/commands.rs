use crate::state::AppState;
use tauri::{AppHandle, State};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use trader_core::{EngineSnapshot, StartStrategyRequest};
use trader_shared::{
    BacktestParams, ModelEvalParams, ModelEvalRunResult, ModelRegisterRequest, OpenDConfig,
    OpenDTradeEnv, Order, OrderRequest, RegisteredModel, RiskLimits, StrategyDefinition,
    StrategyUpsertRequest, TimeControls,
};

#[tauri::command]
pub async fn engine_snapshot(state: State<'_, AppState>) -> Result<EngineSnapshot, String> {
    Ok(state.engine.snapshot().await)
}

#[tauri::command]
pub async fn engine_set_watchlist(state: State<'_, AppState>, symbols: Vec<String>) -> Result<(), String> {
    state
        .engine
        .set_watchlist(symbols)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn engine_get_candles(
    state: State<'_, AppState>,
    symbol: String,
    interval_sec: u32,
    limit: u32,
) -> Result<Vec<trader_shared::Candle>, String> {
    state
        .engine
        .get_candles(&symbol, interval_sec, limit as usize)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn engine_place_order(state: State<'_, AppState>, req: OrderRequest) -> Result<Order, String> {
    state
        .engine
        .place_order(req, None)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn engine_cancel_order(state: State<'_, AppState>, order_id: String) -> Result<Order, String> {
    state
        .engine
        .cancel_order(&order_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn engine_generate_sample_candles_csv(
    state: State<'_, AppState>,
    symbol: String,
    interval_sec: u32,
    limit: u32,
) -> Result<String, String> {
    state
        .engine
        .generate_sample_candles_csv(&symbol, interval_sec, limit as usize)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn engine_update_opend_config(state: State<'_, AppState>, opend: OpenDConfig) -> Result<(), String> {
    state
        .engine
        .update_opend_config(opend)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn engine_update_opend_trade_env(
    state: State<'_, AppState>,
    env: OpenDTradeEnv,
) -> Result<(), String> {
    state
        .engine
        .update_opend_trade_env(env)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn engine_update_time_controls(
    state: State<'_, AppState>,
    time_controls: TimeControls,
) -> Result<(), String> {
    state
        .engine
        .update_time_controls(time_controls)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn engine_test_opend_connection(state: State<'_, AppState>) -> Result<(), String> {
    state.engine.test_opend_connection().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn engine_secret_status(
    state: State<'_, AppState>,
    key: String,
) -> Result<bool, String> {
    state.engine.secret_status(&key).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn engine_set_secret(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<(), String> {
    state
        .engine
        .set_secret(&key, &value)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn engine_clear_secret(
    state: State<'_, AppState>,
    key: String,
) -> Result<(), String> {
    state
        .engine
        .clear_secret(&key)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn engine_set_kill_switch_hotkey(
    app: AppHandle,
    state: State<'_, AppState>,
    hotkey: String,
) -> Result<(), String> {
    let shortcut = hotkey
        .parse::<tauri_plugin_global_shortcut::Shortcut>()
        .map_err(|e| e.to_string())?;

    // Register new shortcut before removing the old one, to avoid leaving the user without a kill switch.
    let mgr = app.global_shortcut();
    let engine2 = state.engine.clone();
    mgr.on_shortcut(shortcut, move |_app, _shortcut, event| {
        if event.state != tauri_plugin_global_shortcut::ShortcutState::Pressed {
            return;
        }
        let engine3 = engine2.clone();
        tauri::async_runtime::spawn(async move {
            let _ = engine3
                .engage_kill_switch("Global kill-switch hotkey engaged".to_string())
                .await;
        });
    })
    .map_err(|e| e.to_string())?;

    // Best-effort unregister old shortcut.
    let old_hotkey = state.engine.snapshot().await.active_profile.kill_switch_hotkey;
    if let Ok(old_shortcut) = old_hotkey.parse::<tauri_plugin_global_shortcut::Shortcut>() {
        if old_shortcut.id() != shortcut.id() {
            let _ = mgr.unregister(old_shortcut);
        }
    }

    state
        .engine
        .update_kill_switch_hotkey(hotkey)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn engine_start_strategy(
    state: State<'_, AppState>,
    req: StartStrategyRequest,
) -> Result<String, String> {
    state.engine.start_strategy(req).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn engine_stop_strategy(state: State<'_, AppState>, instance_id: String) -> Result<(), String> {
    state
        .engine
        .stop_strategy(&instance_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn engine_engage_kill_switch(state: State<'_, AppState>, reason: String) -> Result<(), String> {
    state.engine.engage_kill_switch(reason).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn engine_update_risk_limits(state: State<'_, AppState>, limits: RiskLimits) -> Result<(), String> {
    state
        .engine
        .update_risk_limits(limits)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn engine_set_active_profile(state: State<'_, AppState>, profile: String) -> Result<(), String> {
    state
        .engine
        .set_active_profile(&profile)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn engine_list_audit_events(
    state: State<'_, AppState>,
    limit: u32,
    offset: u32,
    event_type: Option<String>,
    trace_id: Option<String>,
) -> Result<Vec<trader_core::audit::AuditEventRow>, String> {
    state
        .engine
        .list_audit_events(limit, offset, event_type, trace_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn engine_export_audit_jsonl(state: State<'_, AppState>) -> Result<String, String> {
    state.engine.export_audit_jsonl().await.map_err(|e| e.to_string())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BacktestRunResult {
    pub report: trader_shared::BacktestReport,
    pub report_json_path: String,
    pub report_html_path: String,
}

#[tauri::command]
pub async fn engine_run_backtest(
    state: State<'_, AppState>,
    params: BacktestParams,
) -> Result<BacktestRunResult, String> {
    let (report, report_json_path, report_html_path) = state
        .engine
        .run_backtest(params)
        .await
        .map_err(|e| e.to_string())?;
    Ok(BacktestRunResult {
        report,
        report_json_path,
        report_html_path,
    })
}

#[tauri::command]
pub async fn strategy_catalog(_state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    Ok(trader_strategies::strategy_catalog_json())
}

#[tauri::command]
pub async fn model_catalog(_state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    Ok(trader_models::model_catalog_json())
}

#[tauri::command]
pub async fn engine_list_strategy_defs(
    state: State<'_, AppState>,
) -> Result<Vec<StrategyDefinition>, String> {
    state
        .engine
        .list_strategy_defs()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn engine_upsert_strategy_def(
    state: State<'_, AppState>,
    req: StrategyUpsertRequest,
) -> Result<StrategyDefinition, String> {
    state
        .engine
        .upsert_strategy_def(req)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn engine_delete_strategy_def(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state
        .engine
        .delete_strategy_def(&id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn engine_start_strategy_def(state: State<'_, AppState>, id: String) -> Result<String, String> {
    state
        .engine
        .start_strategy_def(&id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn engine_list_models(state: State<'_, AppState>) -> Result<Vec<RegisteredModel>, String> {
    state.engine.list_models().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn engine_register_model(
    state: State<'_, AppState>,
    req: ModelRegisterRequest,
) -> Result<RegisteredModel, String> {
    state
        .engine
        .register_model(req)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn engine_delete_model(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.engine.delete_model(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn engine_evaluate_model(
    state: State<'_, AppState>,
    params: ModelEvalParams,
) -> Result<ModelEvalRunResult, String> {
    state
        .engine
        .evaluate_model(params)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn enable_live_trading_unlock(
    state: State<'_, AppState>,
    confirmation_phrase: String,
    risk_non_default: bool,
    kill_switch_configured: bool,
) -> Result<(), String> {
    state
        .engine
        .enable_live_trading_unlock(&confirmation_phrase, risk_non_default, kill_switch_configured)
        .await
        .map_err(|e| e.to_string())
}
