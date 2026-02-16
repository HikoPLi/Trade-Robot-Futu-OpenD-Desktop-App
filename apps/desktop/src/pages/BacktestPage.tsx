import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { openPath } from "@tauri-apps/plugin-opener";
import {
  BacktestRunResultSchema,
  StrategyCatalogSchema,
  type BacktestParams,
  type BacktestRunResult,
  type StrategyCatalog,
} from "@trade-robot/shared";
import { Badge, Button, Card, Field, Input, Select } from "../components/ui";
import { SchemaForm, initFromSchema } from "../components/SchemaForm";
import { useEngine } from "../lib/engineContext";

function pct(n: number) {
  return `${n.toFixed(2)}%`;
}

export function BacktestPage() {
  const { t } = useTranslation();
  const eng = useEngine();
  const s = eng.snapshot;

  const [catalog, setCatalog] = useState<StrategyCatalog | null>(null);
  const [strategyId, setStrategyId] = useState<string>("ma_crossover");
  const [strategyParams, setStrategyParams] = useState<Record<string, any>>({});

  const [symbol, setSymbol] = useState<string>(s?.watchlist[0] ?? "US.AAPL");
  const [startingCash, setStartingCash] = useState<number>(10_000);
  const [feePerTrade, setFeePerTrade] = useState<number>(0.1);
  const [slippageBps, setSlippageBps] = useState<number>(1);
  const [candlesCsvPath, setCandlesCsvPath] = useState<string>("");

  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<BacktestRunResult | null>(null);

  useEffect(() => {
    let alive = true;
    (async () => {
      try {
        const raw = await eng.strategyCatalog();
        const cat = StrategyCatalogSchema.parse(raw);
        if (!alive) return;
        setCatalog(cat);
        const first = Object.keys(cat)[0] ?? "ma_crossover";
        setStrategyId(first);
        setStrategyParams(initFromSchema(cat[first].params_schema));
      } catch (e) {
        alert(String(e));
      }
    })();
    return () => {
      alive = false;
    };
  }, []);

  const selected = useMemo(() => (catalog ? catalog[strategyId] : null), [catalog, strategyId]);

  function displayStrategyName(id: string, fallback: string) {
    return t(`catalog.strategy.${id}.name`, { defaultValue: fallback });
  }

  function displayStrategyDescription(id: string, fallback: string) {
    return t(`catalog.strategy.${id}.description`, { defaultValue: fallback });
  }

  useEffect(() => {
    if (!selected) return;
    setStrategyParams(initFromSchema(selected.params_schema));
  }, [strategyId]);

  async function generateSample() {
    try {
      const path = await eng.generateSampleCandlesCsv(symbol, 60, 800);
      setCandlesCsvPath(path);
      alert(t("backtest.generated_alert", { path }));
    } catch (e) {
      alert(String(e));
    }
  }

  async function run() {
    const params: BacktestParams = {
      symbol,
      strategy_id: strategyId,
      strategy_params: strategyParams,
      starting_cash: startingCash,
      fee_per_trade: feePerTrade,
      slippage_bps: slippageBps,
      candles_csv_path: candlesCsvPath,
    };

    setRunning(true);
    try {
      const raw = await eng.runBacktest(params);
      const parsed = BacktestRunResultSchema.parse(raw);
      setResult(parsed);
    } catch (e) {
      alert(String(e));
    } finally {
      setRunning(false);
    }
  }

  return (
    <div style={{ display: "grid", gap: 12 }}>
      <Card style={{ padding: 14 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <div style={{ fontWeight: 700 }}>{t("backtest.runner")}</div>
          <Badge tone="neutral">{t("backtest.badge")}</Badge>
        </div>

        <div style={{ marginTop: 12 }} className="tr-grid">
          <div style={{ gridColumn: "span 3" }}>
            <Field label={t("backtest.symbol")}>
              <Select value={symbol} onChange={(e) => setSymbol(e.currentTarget.value)}>
                {(s?.watchlist.length ? s.watchlist : ["US.AAPL", "US.TSLA"]).map((sym) => (
                  <option key={sym} value={sym}>
                    {sym}
                  </option>
                ))}
              </Select>
            </Field>
          </div>
          <div style={{ gridColumn: "span 5" }}>
            <Field label={t("backtest.strategy")}>
              <Select
                value={strategyId}
                onChange={(e) => {
                  const id = e.currentTarget.value;
                  setStrategyId(id);
                  const meta = catalog?.[id];
                  if (meta) setStrategyParams(initFromSchema(meta.params_schema));
                }}
              >
                {catalog
                  ? Object.values(catalog).map((m) => (
                      <option key={m.id} value={m.id}>
                        {displayStrategyName(m.id, m.name)}
                      </option>
                    ))
                  : null}
              </Select>
            </Field>
          </div>
          <div style={{ gridColumn: "span 2" }}>
            <Field label={t("backtest.start_cash")}>
              <Input type="number" value={startingCash} onChange={(e) => setStartingCash(Number(e.currentTarget.value))} />
            </Field>
          </div>
          <div style={{ gridColumn: "span 1" }}>
            <Field label={t("backtest.fee")}>
              <Input type="number" value={feePerTrade} step={0.01} onChange={(e) => setFeePerTrade(Number(e.currentTarget.value))} />
            </Field>
          </div>
          <div style={{ gridColumn: "span 1" }}>
            <Field label={t("backtest.slippage_bps")}>
              <Input type="number" value={slippageBps} step={0.1} onChange={(e) => setSlippageBps(Number(e.currentTarget.value))} />
            </Field>
          </div>

          <div style={{ gridColumn: "span 9" }}>
            <Field label={t("backtest.candles_csv_path")} hint={t("backtest.candles_csv_hint")}>
              <Input value={candlesCsvPath} placeholder={t("backtest.candles_csv_hint")} onChange={(e) => setCandlesCsvPath(e.currentTarget.value)} />
            </Field>
          </div>
          <div style={{ gridColumn: "span 3", display: "flex", gap: 8, alignItems: "flex-end", justifyContent: "flex-end" }}>
            <Button variant="ghost" onClick={() => generateSample()}>
              {t("backtest.generate_sample")}
            </Button>
            <Button variant="primary" disabled={!candlesCsvPath || running} onClick={() => run()}>
              {running ? t("common.running") : t("backtest.run_backtest")}
            </Button>
          </div>
        </div>

        {selected ? (
          <div style={{ marginTop: 12 }}>
            <div style={{ fontWeight: 700, marginBottom: 8 }}>{t("backtest.strategy_params")}</div>
            <SchemaForm schema={selected.params_schema} value={strategyParams} onChange={setStrategyParams} />
            <div style={{ marginTop: 10, color: "var(--muted)", fontSize: 12 }}>
              {displayStrategyDescription(selected.id, selected.description)}
            </div>
          </div>
        ) : null}
      </Card>

      {result ? (
        <Card style={{ padding: 14 }}>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
            <div style={{ fontWeight: 700 }}>{t("backtest.report")}</div>
            <div style={{ display: "flex", gap: 8 }}>
              <Button variant="ghost" onClick={() => openPath(result.report_html_path).catch((e) => alert(String(e)))}>
                {t("common.open_html")}
              </Button>
              <Button variant="ghost" onClick={() => openPath(result.report_json_path).catch((e) => alert(String(e)))}>
                {t("common.open_json")}
              </Button>
            </div>
          </div>

          <div className="tr-grid" style={{ marginTop: 12 }}>
            <Card className="tr-kpi" style={{ gridColumn: "span 2", boxShadow: "none" }}>
              <div className="tr-kpi__label">{t("backtest.total_return")}</div>
              <div className="tr-kpi__value">{pct(result.report.total_return_pct)}</div>
            </Card>
            <Card className="tr-kpi" style={{ gridColumn: "span 2", boxShadow: "none" }}>
              <div className="tr-kpi__label">{t("backtest.max_drawdown")}</div>
              <div className="tr-kpi__value">{pct(result.report.max_drawdown_pct)}</div>
            </Card>
            <Card className="tr-kpi" style={{ gridColumn: "span 2", boxShadow: "none" }}>
              <div className="tr-kpi__label">{t("backtest.sharpe_ratio")}</div>
              <div className="tr-kpi__value">{result.report.sharpe_ratio.toFixed(3)}</div>
            </Card>
            <Card className="tr-kpi" style={{ gridColumn: "span 2", boxShadow: "none" }}>
              <div className="tr-kpi__label">{t("backtest.turnover")}</div>
              <div className="tr-kpi__value">{result.report.turnover.toFixed(2)}x</div>
            </Card>
            <Card className="tr-kpi" style={{ gridColumn: "span 2", boxShadow: "none" }}>
              <div className="tr-kpi__label">{t("backtest.hit_rate")}</div>
              <div className="tr-kpi__value">{(result.report.hit_rate * 100).toFixed(1)}%</div>
            </Card>
            <Card className="tr-kpi" style={{ gridColumn: "span 2", boxShadow: "none" }}>
              <div className="tr-kpi__label">{t("backtest.trades")}</div>
              <div className="tr-kpi__value">{result.report.trade_count}</div>
            </Card>
          </div>

          <div style={{ marginTop: 10, color: "var(--muted)", fontSize: 12 }}>
            {t("backtest.export_paths")}
            <div className="tr-mono" style={{ marginTop: 6 }}>
              {t("backtest.html")}: {result.report_html_path}
              <br />
              {t("backtest.json")}: {result.report_json_path}
            </div>
          </div>
        </Card>
      ) : null}
    </div>
  );
}
