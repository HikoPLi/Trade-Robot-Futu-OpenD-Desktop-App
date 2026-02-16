use crate::secrets;
use anyhow::Context;
use std::time::Duration;
use trader_shared::{
    AiProviderConfig, AiProviderKind, AiSignalRequest, AiSignalResponse, AiTradeAction, OrderSide,
    ProfileConfig,
};

const SYSTEM_PROMPT: &str = r#"You are a strict risk gate for a short-term trading robot.
Return JSON only with this schema:
{
  "action": "buy" | "sell" | "hold",
  "confidence": 0.0..1.0,
  "reason": "short explanation",
  "safeguards": ["list", "of", "checks"]
}
Rules:
- Never claim guaranteed profit.
- If uncertain, choose "hold".
- Prefer conservative action when data quality is limited.
- Keep reason concise and factual."#;

#[derive(Clone)]
pub struct ModelApiRuntime {
    client: reqwest::Client,
}

impl ModelApiRuntime {
    pub fn new() -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent("trade-robot/0.1")
            .build()
            .context("build reqwest client")?;
        Ok(Self { client })
    }

    pub async fn infer_signal(
        &self,
        profile_name: &str,
        profile: &ProfileConfig,
        req: &AiSignalRequest,
    ) -> anyhow::Result<AiSignalResponse> {
        let route = provider_route(profile);
        if route.is_empty() {
            anyhow::bail!("ai provider route is empty");
        }

        let mut errors = Vec::new();
        for provider_id in route {
            let Some(cfg) = profile.ai_providers.get(&provider_id).cloned() else {
                continue;
            };
            if !cfg.enabled {
                continue;
            }

            match self
                .call_provider(profile_name, cfg.clone(), req)
                .await
                .with_context(|| format!("provider {}", cfg.id))
            {
                Ok(resp) => return Ok(resp),
                Err(err) => errors.push(err.to_string()),
            }
        }

        if errors.is_empty() {
            anyhow::bail!("no enabled ai provider in route");
        }
        anyhow::bail!("all ai providers failed: {}", errors.join(" | "))
    }

    async fn call_provider(
        &self,
        profile_name: &str,
        cfg: AiProviderConfig,
        req: &AiSignalRequest,
    ) -> anyhow::Result<AiSignalResponse> {
        let key_opt = if cfg.api_key_secret.trim().is_empty() {
            None
        } else {
            secrets::get_secret(profile_name.to_string(), cfg.api_key_secret.clone()).await?
        };

        if !matches!(cfg.kind, AiProviderKind::Ollama) && key_opt.is_none() {
            anyhow::bail!("missing secret {}", cfg.api_key_secret);
        }

        let prompt = build_user_prompt(req);
        let timeout = Duration::from_millis(cfg.timeout_ms.max(500));
        let base = cfg.base_url.trim_end_matches('/').to_string();

        let content = if matches!(cfg.kind, AiProviderKind::Ollama) {
            let url = format!("{base}/api/chat");
            let body = serde_json::json!({
                "model": cfg.model,
                "stream": false,
                "format": "json",
                "messages": [
                    {"role": "system", "content": SYSTEM_PROMPT},
                    {"role": "user", "content": prompt},
                ]
            });
            let mut rb = self.client.post(url).timeout(timeout).json(&body);
            if let Some(k) = key_opt.clone() {
                rb = rb.bearer_auth(k);
            }
            let resp = rb.send().await.context("request failed")?;
            let status = resp.status();
            let raw: serde_json::Value = resp.json().await.context("json decode failed")?;
            if !status.is_success() {
                anyhow::bail!("http {status} body={raw}");
            }
            raw.pointer("/message/content")
                .and_then(|v| v.as_str())
                .map(ToString::to_string)
                .context("missing ollama message.content")?
        } else {
            let url = format!("{base}/chat/completions");
            let body = serde_json::json!({
                "model": cfg.model,
                "temperature": cfg.temperature,
                "max_tokens": cfg.max_tokens,
                "response_format": {"type": "json_object"},
                "messages": [
                    {"role": "system", "content": SYSTEM_PROMPT},
                    {"role": "user", "content": prompt},
                ]
            });
            let mut rb = self.client.post(url).timeout(timeout).json(&body);
            if let Some(k) = key_opt {
                rb = rb.bearer_auth(k);
            }
            let resp = rb.send().await.context("request failed")?;
            let status = resp.status();
            let raw: serde_json::Value = resp.json().await.context("json decode failed")?;
            if !status.is_success() {
                anyhow::bail!("http {status} body={raw}");
            }
            extract_openai_content(&raw).context("missing openai choices[0].message.content")?
        };

        let mut parsed = parse_model_output(&content).context("invalid model output")?;
        parsed.provider_id = cfg.id;
        parsed.model = cfg.model;
        Ok(parsed)
    }
}

fn provider_route(profile: &ProfileConfig) -> Vec<String> {
    let mut out = Vec::new();
    let primary = profile.ai_router.primary.trim();
    if !primary.is_empty() {
        out.push(primary.to_string());
    }
    for id in profile.ai_router.fallbacks.iter() {
        if id.trim().is_empty() {
            continue;
        }
        if !out.iter().any(|x| x == id) {
            out.push(id.clone());
        }
    }
    out
}

fn build_user_prompt(req: &AiSignalRequest) -> String {
    format!(
        "symbol={symbol}\nstrategy_id={strategy_id}\nproposed_side={side}\nlast_price={last_price:.6}\nspread_bps={spread_bps:.3}\nhorizon_sec={horizon}\nreason={reason}",
        symbol = req.symbol,
        strategy_id = req.strategy_id,
        side = side_str(req.proposed_side),
        last_price = req.last_price,
        spread_bps = req.spread_bps,
        horizon = req.horizon_sec,
        reason = req.reason,
    )
}

fn side_str(side: OrderSide) -> &'static str {
    match side {
        OrderSide::Buy => "buy",
        OrderSide::Sell => "sell",
    }
}

fn extract_openai_content(raw: &serde_json::Value) -> Option<String> {
    let c = raw.pointer("/choices/0/message/content")?;
    if let Some(s) = c.as_str() {
        return Some(s.to_string());
    }
    if let Some(arr) = c.as_array() {
        for item in arr {
            if let Some(s) = item.get("text").and_then(|v| v.as_str()) {
                return Some(s.to_string());
            }
        }
    }
    None
}

fn parse_model_output(content: &str) -> anyhow::Result<AiSignalResponse> {
    let raw = parse_json_object(content)?;
    let action = match raw
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("hold")
        .to_ascii_lowercase()
        .as_str()
    {
        "buy" => AiTradeAction::Buy,
        "sell" => AiTradeAction::Sell,
        _ => AiTradeAction::Hold,
    };
    let confidence = raw
        .get("confidence")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let reason = raw
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("no reason")
        .trim()
        .to_string();
    let safeguards = raw
        .get("safeguards")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(|s| s.trim().to_string()))
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(AiSignalResponse {
        provider_id: String::new(),
        model: String::new(),
        action,
        confidence,
        reason,
        safeguards,
        raw,
    })
}

fn parse_json_object(content: &str) -> anyhow::Result<serde_json::Value> {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(content) {
        if v.is_object() {
            return Ok(v);
        }
    }

    let start = content.find('{').context("json start not found")?;
    let end = content.rfind('}').context("json end not found")?;
    if end <= start {
        anyhow::bail!("invalid json wrapper");
    }
    let slice = &content[start..=end];
    let v: serde_json::Value = serde_json::from_str(slice).context("parse json slice")?;
    if !v.is_object() {
        anyhow::bail!("parsed json is not object");
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_model_output_json() {
        let text = r#"{"action":"buy","confidence":0.77,"reason":"trend continuation","safeguards":["spread_ok"]}"#;
        let v = parse_model_output(text).unwrap();
        assert!(matches!(v.action, AiTradeAction::Buy));
        assert!((v.confidence - 0.77).abs() < 1e-9);
        assert_eq!(v.reason, "trend continuation");
        assert_eq!(v.safeguards.len(), 1);
    }

    #[test]
    fn parse_model_output_wrapped_text() {
        let text = "answer:\n```json\n{\"action\":\"hold\",\"confidence\":0.3,\"reason\":\"uncertain\"}\n```";
        let v = parse_model_output(text).unwrap();
        assert!(matches!(v.action, AiTradeAction::Hold));
        assert!((v.confidence - 0.3).abs() < 1e-9);
    }
}
