use anyhow::Context;
use serde::{Deserialize, Serialize};
use trader_shared::Candle;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub params_schema: serde_json::Value,
}

pub trait Model: Send {
    fn metadata(&self) -> ModelMetadata;
    fn set_params(&mut self, params: serde_json::Value) -> anyhow::Result<()>;

    /// Compute a scalar value from candles. Returns None if insufficient data.
    fn compute(&mut self, candles: &[Candle]) -> anyhow::Result<Option<f64>>;
}

#[derive(Debug, Clone)]
pub enum BuiltInModel {
    Sma(SmaModel),
    ZScore(ZScoreModel),
}

impl BuiltInModel {
    pub fn boxed(self) -> Box<dyn Model> {
        match self {
            BuiltInModel::Sma(m) => Box::new(m),
            BuiltInModel::ZScore(m) => Box::new(m),
        }
    }
}

pub fn built_in_models() -> Vec<Box<dyn Model>> {
    vec![BuiltInModel::Sma(SmaModel::default()).boxed(), BuiltInModel::ZScore(ZScoreModel::default()).boxed()]
}

pub fn create_model(id: &str) -> anyhow::Result<Box<dyn Model>> {
    match id {
        "sma" => Ok(Box::new(SmaModel::default())),
        "zscore" => Ok(Box::new(ZScoreModel::default())),
        _ => anyhow::bail!("unknown model id: {id}"),
    }
}

pub fn validate_model_params(id: &str, params: serde_json::Value) -> anyhow::Result<()> {
    let mut m = create_model(id)?;
    m.set_params(params)
}

pub fn model_catalog_json() -> serde_json::Value {
    let models = built_in_models();
    let mut map = BTreeMap::new();
    for m in models {
        let meta = m.metadata();
        map.insert(meta.id.clone(), meta);
    }
    serde_json::to_value(map).expect("serializable")
}

#[derive(Debug, Clone)]
pub struct SmaModel {
    period: usize,
}

impl Default for SmaModel {
    fn default() -> Self {
        Self { period: 20 }
    }
}

impl Model for SmaModel {
    fn metadata(&self) -> ModelMetadata {
        ModelMetadata {
            id: "sma".to_string(),
            name: "Simple Moving Average".to_string(),
            description: "Computes SMA(period) of candle close.".to_string(),
            version: "1.0.0".to_string(),
            params_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "period": {"type": "integer", "minimum": 1, "maximum": 500, "default": 20}
                },
                "required": ["period"],
                "additionalProperties": false
            }),
        }
    }

    fn set_params(&mut self, params: serde_json::Value) -> anyhow::Result<()> {
        #[derive(Deserialize)]
        struct P {
            period: usize,
        }
        let p: P = serde_json::from_value(params).context("invalid params")?;
        if p.period == 0 || p.period > 500 {
            anyhow::bail!("period out of range");
        }
        self.period = p.period;
        Ok(())
    }

    fn compute(&mut self, candles: &[Candle]) -> anyhow::Result<Option<f64>> {
        if candles.len() < self.period {
            return Ok(None);
        }
        let slice = &candles[candles.len() - self.period..];
        let sum: f64 = slice.iter().map(|c| c.close).sum();
        Ok(Some(sum / self.period as f64))
    }
}

#[derive(Debug, Clone)]
pub struct ZScoreModel {
    period: usize,
}

impl Default for ZScoreModel {
    fn default() -> Self {
        Self { period: 50 }
    }
}

impl Model for ZScoreModel {
    fn metadata(&self) -> ModelMetadata {
        ModelMetadata {
            id: "zscore".to_string(),
            name: "Z-Score".to_string(),
            description: "Z-score of close relative to SMA(period) and stddev(period).".to_string(),
            version: "1.0.0".to_string(),
            params_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "period": {"type": "integer", "minimum": 2, "maximum": 500, "default": 50}
                },
                "required": ["period"],
                "additionalProperties": false
            }),
        }
    }

    fn set_params(&mut self, params: serde_json::Value) -> anyhow::Result<()> {
        #[derive(Deserialize)]
        struct P {
            period: usize,
        }
        let p: P = serde_json::from_value(params).context("invalid params")?;
        if p.period < 2 || p.period > 500 {
            anyhow::bail!("period out of range");
        }
        self.period = p.period;
        Ok(())
    }

    fn compute(&mut self, candles: &[Candle]) -> anyhow::Result<Option<f64>> {
        if candles.len() < self.period {
            return Ok(None);
        }
        let slice = &candles[candles.len() - self.period..];
        let mean: f64 = slice.iter().map(|c| c.close).sum::<f64>() / self.period as f64;
        let var: f64 = slice
            .iter()
            .map(|c| {
                let d = c.close - mean;
                d * d
            })
            .sum::<f64>()
            / (self.period as f64);
        let std = var.sqrt();
        if std == 0.0 {
            return Ok(Some(0.0));
        }
        let last = slice.last().expect("non-empty").close;
        Ok(Some((last - mean) / std))
    }
}
