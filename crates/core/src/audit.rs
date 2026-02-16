use crate::paths::AppPaths;
use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio_rusqlite::rusqlite;
use tokio_rusqlite::Connection;
use trader_shared::TraceId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEventRow {
    pub id: i64,
    pub ts: DateTime<Utc>,
    pub event_type: String,
    pub payload_json: serde_json::Value,
    pub trace_id: Option<TraceId>,
    pub prev_hash: Option<String>,
    pub hash: String,
}

#[derive(Debug, Clone)]
pub struct AuditAppendResult {
    pub id: i64,
    pub hash: String,
}

#[derive(Debug, Clone)]
pub struct AuditLog {
    conn: Connection,
}

impl AuditLog {
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
CREATE TABLE IF NOT EXISTS audit_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  ts TEXT NOT NULL,
  event_type TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  trace_id TEXT,
  prev_hash TEXT,
  hash TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS audit_events_ts_idx ON audit_events(ts);
CREATE INDEX IF NOT EXISTS audit_events_type_idx ON audit_events(event_type);
CREATE INDEX IF NOT EXISTS audit_events_trace_idx ON audit_events(trace_id);
"#,
                )?;
                Ok(())
            })
            .await
            .context("migrate audit_events")?;
        Ok(())
    }

    pub async fn append(
        &self,
        ts: DateTime<Utc>,
        event_type: &str,
        payload_json: &serde_json::Value,
        trace_id: Option<TraceId>,
    ) -> anyhow::Result<AuditAppendResult> {
        let event_type = event_type.to_string();
        let payload_str = serde_json::to_string(payload_json).context("serialize audit payload")?;
        let trace_str = trace_id.map(|t| t.to_string());
        let ts_str = ts.to_rfc3339();

        self.conn
            .call(move |c| -> rusqlite::Result<AuditAppendResult> {
                let prev_hash: Option<String> = c
                    .query_row(
                        "SELECT hash FROM audit_events ORDER BY id DESC LIMIT 1",
                        [],
                        |row| row.get(0),
                    )
                    .ok();

                let hash = compute_hash(prev_hash.as_deref(), &ts_str, &event_type, trace_str.as_deref(), &payload_str);

                c.execute(
                    "INSERT INTO audit_events (ts, event_type, payload_json, trace_id, prev_hash, hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    (
                        ts_str,
                        event_type,
                        payload_str,
                        trace_str,
                        prev_hash,
                        hash.clone(),
                    ),
                )?;

                let id = c.last_insert_rowid();
                Ok(AuditAppendResult { id, hash })
            })
            .await
            .context("append audit event")
    }

    pub async fn list(
        &self,
        limit: u32,
        offset: u32,
        event_type: Option<String>,
        trace_id: Option<String>,
    ) -> anyhow::Result<Vec<AuditEventRow>> {
        let limit = limit.min(500) as i64;
        let offset = offset as i64;
        self.conn
            .call(move |c| -> rusqlite::Result<Vec<AuditEventRow>> {
                let mut sql = String::from(
                    "SELECT id, ts, event_type, payload_json, trace_id, prev_hash, hash FROM audit_events",
                );
                let mut params: Vec<rusqlite::types::Value> = vec![];
                let mut where_added = false;
                if let Some(et) = &event_type {
                    sql.push_str(" WHERE event_type = ?");
                    params.push(et.clone().into());
                    where_added = true;
                }
                if let Some(t) = &trace_id {
                    sql.push_str(if where_added { " AND trace_id = ?" } else { " WHERE trace_id = ?" });
                    params.push(t.clone().into());
                }
                sql.push_str(" ORDER BY id DESC LIMIT ? OFFSET ?");
                params.push(limit.into());
                params.push(offset.into());

                let mut stmt = c.prepare(&sql)?;
                let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), |row| {
                    let ts: String = row.get(1)?;
                    let ts = chrono::DateTime::parse_from_rfc3339(&ts)
                        .map_err(|e| {
                            rusqlite::Error::FromSqlConversionFailure(
                                1,
                                rusqlite::types::Type::Text,
                                Box::new(e),
                            )
                        })?
                        .with_timezone(&Utc);
                    let payload_str: String = row.get(3)?;
                    let payload_json: serde_json::Value = serde_json::from_str(&payload_str)
                        .map_err(|e| {
                            rusqlite::Error::FromSqlConversionFailure(
                                3,
                                rusqlite::types::Type::Text,
                                Box::new(e),
                            )
                        })?;
                    let trace_id: Option<String> = row.get(4)?;
                    let trace_id = trace_id.and_then(|s| uuid::Uuid::parse_str(&s).ok());
                    Ok(AuditEventRow {
                        id: row.get(0)?,
                        ts,
                        event_type: row.get(2)?,
                        payload_json,
                        trace_id,
                        prev_hash: row.get(5)?,
                        hash: row.get(6)?,
                    })
                })?;

                let mut out = Vec::new();
                for r in rows {
                    out.push(r?);
                }
                Ok(out)
            })
            .await
            .context("list audit events")
    }

    pub async fn export_jsonl(&self, out_path: std::path::PathBuf) -> anyhow::Result<()> {
        // Export all events in ascending id order for stable replays.
        let mut offset: u32 = 0;
        let mut all: Vec<AuditEventRow> = Vec::new();
        loop {
            let batch = self.list(500, offset, None, None).await?;
            if batch.is_empty() {
                break;
            }
            let n = batch.len() as u32;
            all.extend(batch);
            if n < 500 {
                break;
            }
            offset = offset.saturating_add(n);
        }

        all.sort_by_key(|e| e.id);

        let mut lines = String::new();
        for e in all {
            lines.push_str(&serde_json::to_string(&e).unwrap_or_else(|_| "{}".to_string()));
            lines.push('\n');
        }
        if let Some(parent) = out_path.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        tokio::fs::write(&out_path, lines).await?;
        Ok(())
    }
}

fn compute_hash(
    prev_hash: Option<&str>,
    ts: &str,
    event_type: &str,
    trace_id: Option<&str>,
    payload: &str,
) -> String {
    let mut hasher = Sha256::new();
    if let Some(prev) = prev_hash {
        hasher.update(prev.as_bytes());
    }
    hasher.update(ts.as_bytes());
    hasher.update(event_type.as_bytes());
    if let Some(t) = trace_id {
        hasher.update(t.as_bytes());
    }
    hasher.update(payload.as_bytes());
    let digest = hasher.finalize();
    hex::encode(digest)
}
