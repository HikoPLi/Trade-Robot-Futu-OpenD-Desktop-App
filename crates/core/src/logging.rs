use crate::paths::AppPaths;
use once_cell::sync::OnceCell;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

static LOG_GUARD: OnceCell<tracing_appender::non_blocking::WorkerGuard> = OnceCell::new();

pub fn init_logging(paths: &AppPaths) -> anyhow::Result<()> {
    std::fs::create_dir_all(&paths.log_dir)?;

    let file_appender = tracing_appender::rolling::daily(&paths.log_dir, "trade_robot.jsonl");
    let (nb, guard) = tracing_appender::non_blocking(file_appender);
    let _ = LOG_GUARD.set(guard);

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,trader_core=debug"));

    let fmt = tracing_subscriber::fmt::layer()
        .json()
        .with_writer(nb)
        .with_current_span(true)
        .with_span_list(true);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt)
        .init();
    Ok(())
}
