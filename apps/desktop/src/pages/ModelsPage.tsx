import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { openPath } from "@tauri-apps/plugin-opener";
import {
  ModelCatalogSchema,
  type AiSignalResponse,
  type ModelCatalog,
  type ModelKind,
  type ModelEvalRunResult,
  type OrderSide,
  type RegisteredModel,
} from "@trade-robot/shared";
import { Badge, Button, Card, Field, Input, Select } from "../components/ui";
import { SchemaForm, initFromSchema } from "../components/SchemaForm";
import { useEngine } from "../lib/engineContext";

export function ModelsPage() {
  const { t } = useTranslation();
  const eng = useEngine();
  const s = eng.snapshot;

  const [catalog, setCatalog] = useState<ModelCatalog | null>(null);
  const [kind, setKind] = useState<ModelKind>("builtin");
  const [baseId, setBaseId] = useState<string>("sma");
  const [params, setParams] = useState<Record<string, any>>({});
  const [name, setName] = useState<string>(() => t("models.default_name"));
  const [version, setVersion] = useState<string>("1.0.0");
  const [artifactPath, setArtifactPath] = useState<string>("");

  const [models, setModels] = useState<RegisteredModel[] | null>(null);
  const [busy, setBusy] = useState(false);

  const [evalModelId, setEvalModelId] = useState<string>("");
  const [evalSymbol, setEvalSymbol] = useState<string>(s?.watchlist[0] ?? "US.AAPL");
  const [evalCsvPath, setEvalCsvPath] = useState<string>("");
  const [evalSeed, setEvalSeed] = useState<number>(42);
  const [evalResult, setEvalResult] = useState<ModelEvalRunResult | null>(null);
  const [aiSymbol, setAiSymbol] = useState<string>(s?.watchlist[0] ?? "US.AAPL");
  const [aiSide, setAiSide] = useState<OrderSide>("buy");
  const [aiReason, setAiReason] = useState<string>("short-term momentum breakout setup");
  const [aiHorizonSec, setAiHorizonSec] = useState<number>(60);
  const [aiSignal, setAiSignal] = useState<AiSignalResponse | null>(null);

  useEffect(() => {
    let alive = true;
    (async () => {
      try {
        const raw = await eng.models.catalog();
        const cat = ModelCatalogSchema.parse(raw);
        if (!alive) return;
        setCatalog(cat);
        const first = Object.keys(cat)[0] ?? "sma";
        setBaseId(first);
        setParams(initFromSchema(cat[first]?.params_schema));
      } catch (e) {
        alert(String(e));
      }
    })();
    return () => {
      alive = false;
    };
  }, []);

  const selected = useMemo(() => (catalog ? catalog[baseId] : null), [catalog, baseId]);

  function displayModelName(id: string, fallback: string) {
    return t(`catalog.model.${id}.name`, { defaultValue: fallback });
  }

  function displayModelDescription(id: string, fallback: string) {
    return t(`catalog.model.${id}.description`, { defaultValue: fallback });
  }

  useEffect(() => {
    if (!selected) return;
    setParams(initFromSchema(selected.params_schema));
    setVersion(selected.version);
    if (!name || name === t("models.default_name")) {
      setName(`${displayModelName(selected.id, selected.name)} (${selected.version})`);
    }
  }, [baseId]);

  useEffect(() => {
    if (name) return;
    setName(t("models.default_name"));
  }, [name, t]);

  async function refreshModels() {
    try {
      const list = await eng.models.list();
      setModels(list);
      if (!evalModelId && list.length) setEvalModelId(list[0].id);
    } catch (e) {
      alert(String(e));
    }
  }

  useEffect(() => {
    refreshModels().catch(() => {});
  }, [s?.config.active_profile]);

  async function onRegister() {
    setBusy(true);
    try {
      const base = kind === "builtin" ? baseId : "onnx";
      const m = await eng.models.register({
        base_id: base,
        kind,
        name,
        version,
        artifact_source_path: kind === "onnx" ? artifactPath : null,
        params: kind === "builtin" ? params : {},
      });
      await refreshModels();
      setEvalModelId(m.id);
    } finally {
      setBusy(false);
    }
  }

  async function onEvaluate() {
    if (!evalModelId) return;
    setBusy(true);
    try {
      const res = await eng.models.evaluate({
        model_id: evalModelId,
        candles_csv_path: evalCsvPath,
        symbol: evalSymbol,
        seed: evalSeed,
      });
      setEvalResult(res);
    } finally {
      setBusy(false);
    }
  }

  async function onGenerateAiSignal() {
    const q = s?.quotes.find((x) => x.symbol === aiSymbol);
    const last = q?.last ?? 0;
    const spread_bps = q && q.last > 0 ? ((q.ask - q.bid) / q.last) * 10000 : 0;
    const res = await eng.generateAiSignal({
      symbol: aiSymbol,
      strategy_id: "manual_ai_workbench",
      proposed_side: aiSide,
      reason: aiReason,
      last_price: last,
      spread_bps,
      horizon_sec: aiHorizonSec,
    });
    setAiSignal(res);
  }

  return (
    <div style={{ display: "grid", gap: 12 }}>
      <Card style={{ padding: 14 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <div style={{ fontWeight: 700 }}>{t("models.title")}</div>
          <Badge tone="neutral">{t("models.badge")}</Badge>
        </div>

        <div style={{ marginTop: 12 }} className="tr-grid">
          <div style={{ gridColumn: "span 3" }}>
            <Field label={t("models.kind")}>
              <Select value={kind} onChange={(e) => setKind(e.currentTarget.value as ModelKind)}>
                <option value="builtin">{t("models.kind_builtin")}</option>
                <option value="onnx">{t("models.kind_onnx")}</option>
              </Select>
            </Field>
          </div>

          <div style={{ gridColumn: "span 3" }}>
            <Field label={t("models.name")}>
              <Input value={name} onChange={(e) => setName(e.currentTarget.value)} />
            </Field>
          </div>

          <div style={{ gridColumn: "span 3" }}>
            <Field label={t("models.version")}>
              <Input value={version} onChange={(e) => setVersion(e.currentTarget.value)} />
            </Field>
          </div>

          <div style={{ gridColumn: "span 3", display: "flex", justifyContent: "flex-end", alignItems: "flex-end" }}>
            <Button variant="primary" disabled={busy} onClick={() => onRegister().catch((e) => alert(String(e)))}>
              {t("models.register")}
            </Button>
          </div>

          {kind === "builtin" ? (
            <>
              <div style={{ gridColumn: "span 4" }}>
                <Field label={t("models.base_model")}>
                  <Select
                    value={baseId}
                    onChange={(e) => {
                      const id = e.currentTarget.value;
                      setBaseId(id);
                      const meta = catalog?.[id];
                      if (meta) setParams(initFromSchema(meta.params_schema));
                    }}
                  >
                    {catalog
                      ? Object.values(catalog).map((m) => (
                          <option key={m.id} value={m.id}>
                            {displayModelName(m.id, m.name)}
                          </option>
                        ))
                      : null}
                  </Select>
                </Field>
              </div>
              <div style={{ gridColumn: "span 8" }}>
                {selected ? (
                  <div style={{ padding: "10px 12px", border: "1px solid rgba(255,255,255,0.10)", borderRadius: 12, background: "rgba(0,0,0,0.18)" }}>
                    <div style={{ fontWeight: 700 }}>{displayModelName(selected.id, selected.name)}</div>
                    <div style={{ marginTop: 6, color: "var(--muted)", fontSize: 12 }}>
                      {displayModelDescription(selected.id, selected.description)}
                    </div>
                    <div style={{ marginTop: 8, color: "var(--faint)", fontSize: 12 }}>
                      {t("common.id")}: <span className="tr-mono">{selected.id}</span> · {t("common.version")}:{" "}
                      <span className="tr-mono">{selected.version}</span>
                    </div>
                  </div>
                ) : (
                  <div style={{ color: "var(--muted)", fontSize: 12 }}>{t("models.loading_catalog")}</div>
                )}
              </div>

              <div style={{ gridColumn: "span 12" }}>
                {selected ? <SchemaForm schema={selected.params_schema} value={params} onChange={setParams} /> : null}
              </div>
            </>
          ) : (
            <>
              <div style={{ gridColumn: "span 12" }}>
                <Field label={t("models.onnx_path")} hint={t("models.onnx_hint")}>
                  <Input
                    value={artifactPath}
                    onChange={(e) => setArtifactPath(e.currentTarget.value)}
                    placeholder={t("models.onnx_placeholder")}
                  />
                </Field>
              </div>
            </>
          )}
        </div>
      </Card>

      <Card style={{ padding: 14 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <div style={{ fontWeight: 700 }}>{t("models.registered")}</div>
          <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <Button size="sm" variant="ghost" onClick={() => refreshModels().catch((e) => alert(String(e)))}>
              {t("common.refresh")}
            </Button>
            <Badge tone="neutral">{models?.length ?? 0}</Badge>
          </div>
        </div>

        <div style={{ marginTop: 12 }}>
          {models?.length ? (
            <table className="tr-table">
              <thead>
                <tr>
                  <th>{t("models.table_name")}</th>
                  <th>{t("models.table_kind")}</th>
                  <th>{t("models.table_base")}</th>
                  <th>{t("models.table_version")}</th>
                  <th>{t("models.table_checksum")}</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {models.map((m) => (
                  <tr key={m.id} className="tr-row-hover">
                    <td>{m.name}</td>
                    <td className="tr-mono">{m.kind}</td>
                    <td className="tr-mono">{m.base_id}</td>
                    <td className="tr-mono">{m.version}</td>
                    <td className="tr-mono">{m.checksum.slice(0, 10)}</td>
                    <td style={{ textAlign: "right", display: "flex", gap: 8, justifyContent: "flex-end" }}>
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => {
                          setEvalModelId(m.id);
                          setEvalResult(null);
                        }}
                      >
                        {t("models.evaluate")}
                      </Button>
                      <Button
                        size="sm"
                        variant="danger"
                        onClick={() => {
                          if (!confirm(t("models.delete_confirm"))) return;
                          eng.models.delete(m.id).catch((e) => alert(String(e)));
                        }}
                      >
                        {t("common.delete")}
                      </Button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <div style={{ color: "var(--muted)", fontSize: 12 }}>{t("models.no_registered")}</div>
          )}
        </div>
      </Card>

      <Card style={{ padding: 14 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <div style={{ fontWeight: 700 }}>{t("models.eval_title")}</div>
          <Badge tone="neutral">{evalModelId ? evalModelId.slice(0, 8) : "-"}</Badge>
        </div>

        <div style={{ marginTop: 12 }} className="tr-grid">
          <div style={{ gridColumn: "span 3" }}>
            <Field label={t("models.eval_symbol")}>
              <Input value={evalSymbol} onChange={(e) => setEvalSymbol(e.currentTarget.value)} />
            </Field>
          </div>
          <div style={{ gridColumn: "span 3" }}>
            <Field label={t("models.eval_seed")}>
              <Input type="number" value={evalSeed} onChange={(e) => setEvalSeed(Number(e.currentTarget.value))} />
            </Field>
          </div>
          <div style={{ gridColumn: "span 6" }}>
            <Field label={t("models.eval_csv")} hint={t("models.eval_csv_hint")}>
              <Input
                value={evalCsvPath}
                onChange={(e) => setEvalCsvPath(e.currentTarget.value)}
                placeholder={t("models.eval_csv_placeholder")}
              />
            </Field>
          </div>
          <div style={{ gridColumn: "span 12", display: "flex", gap: 8, justifyContent: "flex-end" }}>
            <Button
              variant="ghost"
              disabled={!evalSymbol || busy}
              onClick={() =>
                eng
                  .generateSampleCandlesCsv(evalSymbol, 60, 800)
                  .then((p) => setEvalCsvPath(p))
                  .catch((e) => alert(String(e)))
              }
            >
              {t("models.generate_sample_csv")}
            </Button>
            <Button variant="primary" disabled={!evalCsvPath || !evalModelId || busy} onClick={() => onEvaluate().catch((e) => alert(String(e)))}>
              {t("models.run_eval")}
            </Button>
          </div>
        </div>

        {evalResult ? (
          <div style={{ marginTop: 12 }}>
            <div style={{ display: "flex", gap: 8, flexWrap: "wrap", alignItems: "center" }}>
              <Badge tone="neutral">
                {t("models.metrics_samples")}: {evalResult.report.metrics.samples}
              </Badge>
              <Badge tone="neutral">
                {t("models.metrics_ic")}: {evalResult.report.metrics.ic.toFixed(4)}
              </Badge>
              <Badge tone="neutral">
                {t("models.metrics_acc")}: {evalResult.report.metrics.accuracy.toFixed(4)}
              </Badge>
              <Button size="sm" variant="ghost" onClick={() => openPath(evalResult.report_json_path).catch(() => {})}>
                {t("models.open_json")}
              </Button>
              <Button size="sm" variant="ghost" onClick={() => openPath(evalResult.report_html_path).catch(() => {})}>
                {t("models.open_html")}
              </Button>
            </div>
          </div>
        ) : null}
      </Card>

      <Card style={{ padding: 14 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <div style={{ fontWeight: 700 }}>{t("models.ai_signal_title")}</div>
          <Badge tone="neutral">{aiSignal ? `${aiSignal.provider_id}/${aiSignal.model}` : "-"}</Badge>
        </div>
        <div style={{ marginTop: 10, color: "var(--muted)", fontSize: 12, lineHeight: 1.5 }}>
          {t("models.ai_signal_help")}
        </div>

        <div style={{ marginTop: 12 }} className="tr-grid">
          <div style={{ gridColumn: "span 3" }}>
            <Field label={t("models.eval_symbol")}>
              <Input value={aiSymbol} onChange={(e) => setAiSymbol(e.currentTarget.value)} />
            </Field>
          </div>
          <div style={{ gridColumn: "span 2" }}>
            <Field label={t("trading.side")}>
              <Select value={aiSide} onChange={(e) => setAiSide(e.currentTarget.value as OrderSide)}>
                <option value="buy">{t("enum.side.buy")}</option>
                <option value="sell">{t("enum.side.sell")}</option>
              </Select>
            </Field>
          </div>
          <div style={{ gridColumn: "span 2" }}>
            <Field label={t("models.ai_horizon")}>
              <Input type="number" min={1} value={aiHorizonSec} onChange={(e) => setAiHorizonSec(Number(e.currentTarget.value))} />
            </Field>
          </div>
          <div style={{ gridColumn: "span 5" }}>
            <Field label={t("models.ai_reason")}>
              <Input value={aiReason} onChange={(e) => setAiReason(e.currentTarget.value)} />
            </Field>
          </div>
          <div style={{ gridColumn: "span 12", display: "flex", justifyContent: "flex-end" }}>
            <Button variant="primary" disabled={!aiSymbol || busy} onClick={() => onGenerateAiSignal().catch((e) => alert(String(e)))}>
              {t("models.ai_run")}
            </Button>
          </div>
        </div>

        {aiSignal ? (
          <div style={{ marginTop: 12, display: "flex", gap: 8, flexWrap: "wrap", alignItems: "center" }}>
            <Badge tone="neutral">
              {t("models.ai_action")}: {aiSignal.action.toUpperCase()}
            </Badge>
            <Badge tone="neutral">
              {t("models.ai_confidence")}: {(aiSignal.confidence * 100).toFixed(1)}%
            </Badge>
            <Badge tone="neutral">
              {t("models.ai_reason")}: {aiSignal.reason}
            </Badge>
          </div>
        ) : null}
      </Card>
    </div>
  );
}
