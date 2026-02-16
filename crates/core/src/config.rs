use crate::paths::AppPaths;
use anyhow::Context;
use std::path::Path;
use trader_shared::AppConfig;

pub async fn load_or_init(paths: &AppPaths) -> anyhow::Result<AppConfig> {
    std::fs::create_dir_all(&paths.data_dir)?;

    if Path::new(&paths.config_path).exists() {
        let bytes = tokio::fs::read(&paths.config_path)
            .await
            .with_context(|| format!("read {:?}", paths.config_path))?;
        let cfg: AppConfig = serde_json::from_slice(&bytes).context("parse config.json")?;
        Ok(cfg)
    } else {
        let cfg = AppConfig::default();
        save(paths, &cfg).await?;
        Ok(cfg)
    }
}

pub async fn save(paths: &AppPaths, cfg: &AppConfig) -> anyhow::Result<()> {
    std::fs::create_dir_all(&paths.data_dir)?;
    let tmp = paths.config_path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(cfg).context("serialize config")?;
    tokio::fs::write(&tmp, &bytes)
        .await
        .with_context(|| format!("write {tmp:?}"))?;
    tokio::fs::rename(&tmp, &paths.config_path)
        .await
        .with_context(|| format!("rename {tmp:?} -> {:?}", paths.config_path))?;
    Ok(())
}
