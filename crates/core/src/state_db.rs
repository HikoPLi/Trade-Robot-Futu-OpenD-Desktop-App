use crate::paths::AppPaths;
use anyhow::Context;
use chrono::{DateTime, Utc};
use tokio_rusqlite::{rusqlite, Connection};
use trader_shared::{
    ModelKind, RegisteredModel, StrategyDefinition, StrategyLifecycle,
};

#[derive(Debug, Clone)]
pub struct ModelEvalRow {
    pub id: String,
    pub model_id: String,
    pub report_json_path: String,
    pub report_html_path: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct StateDb {
    conn: Connection,
}

impl StateDb {
    pub async fn open(paths: &AppPaths) -> anyhow::Result<Self> {
        if let Some(parent) = paths.db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&paths.db_path)
            .await
            .with_context(|| format!("open sqlite at {:?}", paths.db_path))?;
        let this = Self { conn };
        this.migrate().await?;
        Ok(this)
    }

    async fn migrate(&self) -> anyhow::Result<()> {
        self.conn
            .call(|c| -> rusqlite::Result<()> {
                c.execute_batch(
                    r#"
PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;

CREATE TABLE IF NOT EXISTS strategy_defs (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  strategy_id TEXT NOT NULL,
  params_json TEXT NOT NULL,
  lifecycle TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS strategy_defs_updated_idx ON strategy_defs(updated_at);
CREATE INDEX IF NOT EXISTS strategy_defs_strategy_id_idx ON strategy_defs(strategy_id);

CREATE TABLE IF NOT EXISTS model_registry (
  id TEXT PRIMARY KEY,
  base_id TEXT NOT NULL,
  kind TEXT NOT NULL,
  name TEXT NOT NULL,
  version TEXT NOT NULL,
  checksum TEXT NOT NULL,
  artifact_path TEXT,
  params_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS model_registry_updated_idx ON model_registry(updated_at);
CREATE INDEX IF NOT EXISTS model_registry_base_idx ON model_registry(base_id);
CREATE INDEX IF NOT EXISTS model_registry_kind_idx ON model_registry(kind);

CREATE TABLE IF NOT EXISTS model_evals (
  id TEXT PRIMARY KEY,
  model_id TEXT NOT NULL,
  report_json_path TEXT NOT NULL,
  report_html_path TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY(model_id) REFERENCES model_registry(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS model_evals_created_idx ON model_evals(created_at);
CREATE INDEX IF NOT EXISTS model_evals_model_idx ON model_evals(model_id);
"#,
                )?;
                Ok(())
            })
            .await
            .context("migrate state db")?;
        Ok(())
    }

    pub async fn list_strategy_defs(&self) -> anyhow::Result<Vec<StrategyDefinition>> {
        self.conn
            .call(|c| -> rusqlite::Result<Vec<StrategyDefinition>> {
                let mut stmt = c.prepare(
                    "SELECT id, name, strategy_id, params_json, lifecycle, created_at, updated_at FROM strategy_defs ORDER BY updated_at DESC",
                )?;
                let rows = stmt.query_map([], |row| {
                    let created_at: String = row.get(5)?;
                    let updated_at: String = row.get(6)?;
                    let created_at = chrono::DateTime::parse_from_rfc3339(&created_at)
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(e)))?
                        .with_timezone(&Utc);
                    let updated_at = chrono::DateTime::parse_from_rfc3339(&updated_at)
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(e)))?
                        .with_timezone(&Utc);
                    let params_str: String = row.get(3)?;
                    let params: serde_json::Value = serde_json::from_str(&params_str).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(e))
                    })?;
                    let lifecycle: String = row.get(4)?;
                    let lifecycle = match lifecycle.as_str() {
                        "draft" => StrategyLifecycle::Draft,
                        "paper" => StrategyLifecycle::Paper,
                        "live" => StrategyLifecycle::Live,
                        _ => StrategyLifecycle::Draft,
                    };
                    Ok(StrategyDefinition {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        strategy_id: row.get(2)?,
                        params,
                        lifecycle,
                        created_at,
                        updated_at,
                    })
                })?;
                let mut out = Vec::new();
                for r in rows {
                    out.push(r?);
                }
                Ok(out)
            })
            .await
            .context("list strategy_defs")
    }

    pub async fn get_strategy_def(&self, id: String) -> anyhow::Result<Option<StrategyDefinition>> {
        self.conn
            .call(move |c| -> rusqlite::Result<Option<StrategyDefinition>> {
                let mut stmt = c.prepare(
                    "SELECT id, name, strategy_id, params_json, lifecycle, created_at, updated_at FROM strategy_defs WHERE id=?1",
                )?;
                let mut rows = stmt.query((id,))?;
                let Some(row) = rows.next()? else { return Ok(None) };

                let created_at: String = row.get(5)?;
                let updated_at: String = row.get(6)?;
                let created_at = chrono::DateTime::parse_from_rfc3339(&created_at)
                    .map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(e))
                    })?
                    .with_timezone(&Utc);
                let updated_at = chrono::DateTime::parse_from_rfc3339(&updated_at)
                    .map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(e))
                    })?
                    .with_timezone(&Utc);
                let params_str: String = row.get(3)?;
                let params: serde_json::Value = serde_json::from_str(&params_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(e))
                })?;
                let lifecycle: String = row.get(4)?;
                let lifecycle = match lifecycle.as_str() {
                    "draft" => StrategyLifecycle::Draft,
                    "paper" => StrategyLifecycle::Paper,
                    "live" => StrategyLifecycle::Live,
                    _ => StrategyLifecycle::Draft,
                };
                Ok(Some(StrategyDefinition {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    strategy_id: row.get(2)?,
                    params,
                    lifecycle,
                    created_at,
                    updated_at,
                }))
            })
            .await
            .context("get strategy_def")
    }

    pub async fn upsert_strategy_def(
        &self,
        def: StrategyDefinition,
    ) -> anyhow::Result<StrategyDefinition> {
        let id = def.id.clone();
        let name = def.name.clone();
        let strategy_id = def.strategy_id.clone();
        let params_json = serde_json::to_string(&def.params).context("serialize strategy params")?;
        let lifecycle = match def.lifecycle {
            StrategyLifecycle::Draft => "draft",
            StrategyLifecycle::Paper => "paper",
            StrategyLifecycle::Live => "live",
        }
        .to_string();
        let created_at = def.created_at.to_rfc3339();
        let updated_at = def.updated_at.to_rfc3339();

        let def2 = def.clone();
        self.conn
            .call(move |c| -> rusqlite::Result<StrategyDefinition> {
                c.execute(
                    r#"
INSERT INTO strategy_defs (id, name, strategy_id, params_json, lifecycle, created_at, updated_at)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
ON CONFLICT(id) DO UPDATE SET
  name=excluded.name,
  strategy_id=excluded.strategy_id,
  params_json=excluded.params_json,
  lifecycle=excluded.lifecycle,
  updated_at=excluded.updated_at
"#,
                    (
                        id,
                        name,
                        strategy_id,
                        params_json,
                        lifecycle,
                        created_at,
                        updated_at,
                    ),
                )?;
                Ok(def2)
            })
            .await
            .context("upsert strategy_def")
    }

    pub async fn delete_strategy_def(&self, id: String) -> anyhow::Result<()> {
        self.conn
            .call(move |c| -> rusqlite::Result<()> {
                c.execute("DELETE FROM strategy_defs WHERE id=?1", (id,))?;
                Ok(())
            })
            .await
            .context("delete strategy_def")?;
        Ok(())
    }

    pub async fn list_models(&self) -> anyhow::Result<Vec<RegisteredModel>> {
        self.conn
            .call(|c| -> rusqlite::Result<Vec<RegisteredModel>> {
                let mut stmt = c.prepare(
                    "SELECT id, base_id, kind, name, version, checksum, artifact_path, params_json, created_at, updated_at FROM model_registry ORDER BY updated_at DESC",
                )?;
                let rows = stmt.query_map([], |row| {
                    let kind: String = row.get(2)?;
                    let kind = match kind.as_str() {
                        "builtin" => ModelKind::Builtin,
                        "onnx" => ModelKind::Onnx,
                        _ => ModelKind::Builtin,
                    };
                    let params_str: String = row.get(7)?;
                    let params: serde_json::Value = serde_json::from_str(&params_str).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(e))
                    })?;
                    let created_at: String = row.get(8)?;
                    let updated_at: String = row.get(9)?;
                    let created_at = chrono::DateTime::parse_from_rfc3339(&created_at)
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::new(e)))?
                        .with_timezone(&Utc);
                    let updated_at = chrono::DateTime::parse_from_rfc3339(&updated_at)
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(9, rusqlite::types::Type::Text, Box::new(e)))?
                        .with_timezone(&Utc);
                    Ok(RegisteredModel {
                        id: row.get(0)?,
                        base_id: row.get(1)?,
                        kind,
                        name: row.get(3)?,
                        version: row.get(4)?,
                        checksum: row.get(5)?,
                        artifact_path: row.get(6)?,
                        params,
                        created_at,
                        updated_at,
                    })
                })?;
                let mut out = Vec::new();
                for r in rows {
                    out.push(r?);
                }
                Ok(out)
            })
            .await
            .context("list models")
    }

    pub async fn get_model(&self, id: String) -> anyhow::Result<Option<RegisteredModel>> {
        self.conn
            .call(move |c| -> rusqlite::Result<Option<RegisteredModel>> {
                let mut stmt = c.prepare(
                    "SELECT id, base_id, kind, name, version, checksum, artifact_path, params_json, created_at, updated_at FROM model_registry WHERE id=?1",
                )?;
                let mut rows = stmt.query((id,))?;
                let Some(row) = rows.next()? else { return Ok(None) };

                let kind: String = row.get(2)?;
                let kind = match kind.as_str() {
                    "builtin" => ModelKind::Builtin,
                    "onnx" => ModelKind::Onnx,
                    _ => ModelKind::Builtin,
                };
                let params_str: String = row.get(7)?;
                let params: serde_json::Value = serde_json::from_str(&params_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(e))
                })?;
                let created_at: String = row.get(8)?;
                let updated_at: String = row.get(9)?;
                let created_at = chrono::DateTime::parse_from_rfc3339(&created_at)
                    .map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::new(e))
                    })?
                    .with_timezone(&Utc);
                let updated_at = chrono::DateTime::parse_from_rfc3339(&updated_at)
                    .map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(9, rusqlite::types::Type::Text, Box::new(e))
                    })?
                    .with_timezone(&Utc);

                Ok(Some(RegisteredModel {
                    id: row.get(0)?,
                    base_id: row.get(1)?,
                    kind,
                    name: row.get(3)?,
                    version: row.get(4)?,
                    checksum: row.get(5)?,
                    artifact_path: row.get(6)?,
                    params,
                    created_at,
                    updated_at,
                }))
            })
            .await
            .context("get model")
    }

    pub async fn upsert_model(&self, m: RegisteredModel) -> anyhow::Result<RegisteredModel> {
        let id = m.id.clone();
        let base_id = m.base_id.clone();
        let kind = match m.kind {
            ModelKind::Builtin => "builtin",
            ModelKind::Onnx => "onnx",
        }
        .to_string();
        let name = m.name.clone();
        let version = m.version.clone();
        let checksum = m.checksum.clone();
        let artifact_path = m.artifact_path.clone();
        let params_json = serde_json::to_string(&m.params).context("serialize model params")?;
        let created_at = m.created_at.to_rfc3339();
        let updated_at = m.updated_at.to_rfc3339();

        let m2 = m.clone();
        self.conn
            .call(move |c| -> rusqlite::Result<RegisteredModel> {
                c.execute(
                    r#"
INSERT INTO model_registry (id, base_id, kind, name, version, checksum, artifact_path, params_json, created_at, updated_at)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
ON CONFLICT(id) DO UPDATE SET
  base_id=excluded.base_id,
  kind=excluded.kind,
  name=excluded.name,
  version=excluded.version,
  checksum=excluded.checksum,
  artifact_path=excluded.artifact_path,
  params_json=excluded.params_json,
  updated_at=excluded.updated_at
"#,
                    (
                        id,
                        base_id,
                        kind,
                        name,
                        version,
                        checksum,
                        artifact_path,
                        params_json,
                        created_at,
                        updated_at,
                    ),
                )?;
                Ok(m2)
            })
            .await
            .context("upsert model")
    }

    pub async fn delete_model(&self, id: String) -> anyhow::Result<()> {
        self.conn
            .call(move |c| -> rusqlite::Result<()> {
                c.execute("DELETE FROM model_registry WHERE id=?1", (id,))?;
                Ok(())
            })
            .await
            .context("delete model")?;
        Ok(())
    }

    pub async fn insert_model_eval_row(&self, row: ModelEvalRow) -> anyhow::Result<()> {
        let id = row.id.clone();
        let model_id = row.model_id.clone();
        let report_json_path = row.report_json_path.clone();
        let report_html_path = row.report_html_path.clone();
        let created_at = row.created_at.to_rfc3339();

        self.conn
            .call(move |c| -> rusqlite::Result<()> {
                c.execute(
                    "INSERT INTO model_evals (id, model_id, report_json_path, report_html_path, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                    (id, model_id, report_json_path, report_html_path, created_at),
                )?;
                Ok(())
            })
            .await
            .context("insert model eval")?;
        Ok(())
    }
}
