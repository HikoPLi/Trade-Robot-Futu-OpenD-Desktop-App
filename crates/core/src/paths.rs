use anyhow::Context;
use directories::ProjectDirs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub config_path: PathBuf,
    pub db_path: PathBuf,
    pub log_dir: PathBuf,
    pub crash_marker_path: PathBuf,
    pub backtests_dir: PathBuf,
}

pub fn app_paths() -> anyhow::Result<AppPaths> {
    let proj = ProjectDirs::from("com", "lihiko", "TradeRobot")
        .context("failed to determine project directories")?;

    let data_dir = proj.data_dir().to_path_buf();
    let config_path = data_dir.join("config.json");
    let db_path = data_dir.join("db").join("trade_robot.sqlite");
    let log_dir = data_dir.join("logs");
    let crash_marker_path = data_dir.join("crash_marker.json");
    let backtests_dir = data_dir.join("backtests");

    Ok(AppPaths {
        data_dir,
        config_path,
        db_path,
        log_dir,
        crash_marker_path,
        backtests_dir,
    })
}
