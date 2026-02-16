import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { StrategyCatalogSchema, type StrategyCatalog, type StrategyDefinition, type StrategyLifecycle } from "@trade-robot/shared";
import { Badge, Button, Card, Field, Input, Select } from "../components/ui";
import { SchemaForm, initFromSchema } from "../components/SchemaForm";
import { useEngine } from "../lib/engineContext";

export function StrategiesPage() {
  const { t } = useTranslation();
  const eng = useEngine();
  const s = eng.snapshot;

  const [catalog, setCatalog] = useState<StrategyCatalog | null>(null);
  const [selectedId, setSelectedId] = useState<string>("");
  const [params, setParams] = useState<Record<string, any>>({});

  const [defs, setDefs] = useState<StrategyDefinition[] | null>(null);
  const [defId, setDefId] = useState<string | null>(null);
  const [defName, setDefName] = useState<string>(() => t("strategies.default_def_name"));
  const [defLifecycle, setDefLifecycle] = useState<StrategyLifecycle>("draft");

  async function refreshDefs() {
    const d = await eng.listStrategyDefs();
    setDefs(d);
  }

  useEffect(() => {
    let alive = true;
    (async () => {
      try {
        const raw = await eng.strategyCatalog();
        const cat = StrategyCatalogSchema.parse(raw);
        if (!alive) return;
        setCatalog(cat);
        const first = Object.keys(cat)[0] ?? "";
        setSelectedId((prev) => prev || first);
        if (first) setParams(initFromSchema(cat[first].params_schema));

        await refreshDefs();
      } catch (e) {
        alert(String(e));
      }
    })();
    return () => {
      alive = false;
    };
  }, []);

  const selected = useMemo(() => (catalog && selectedId ? catalog[selectedId] : null), [catalog, selectedId]);

  function displayStrategyName(id: string, fallback: string) {
    return t(`catalog.strategy.${id}.name`, { defaultValue: fallback });
  }

  function displayStrategyDescription(id: string, fallback: string) {
    return t(`catalog.strategy.${id}.description`, { defaultValue: fallback });
  }

  function strategyLabel(id: string) {
    const meta = catalog?.[id];
    return meta ? displayStrategyName(id, meta.name) : id;
  }

  const recentSignals = useMemo(() => {
    const out: Array<any> = [];
    for (const evt of eng.events as any[]) {
      if (evt && typeof evt === "object" && (evt as any).signal_fired) out.push((evt as any).signal_fired);
    }
    return out.slice(-20).reverse();
  }, [eng.events]);

  useEffect(() => {
    if (!selected) return;
    setParams(initFromSchema(selected.params_schema));
  }, [selectedId]);

  useEffect(() => {
    if (defId) return;
    setDefName(t("strategies.default_def_name"));
  }, [t, defId]);

  return (
    <div style={{ display: "grid", gap: 12 }}>
      <Card style={{ padding: 14 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <div style={{ fontWeight: 700 }}>{t("strategies.saved")}</div>
          <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
            <Button size="sm" variant="ghost" onClick={() => refreshDefs().catch((e) => alert(String(e)))}>
              {t("common.refresh")}
            </Button>
            <Badge tone="neutral">{defs?.length ?? 0}</Badge>
          </div>
        </div>

        <div style={{ marginTop: 12 }}>
          {defs?.length ? (
            <table className="tr-table">
              <thead>
                <tr>
                  <th>{t("strategies.table_name")}</th>
                  <th>{t("strategies.table_type")}</th>
                  <th>{t("strategies.table_lifecycle")}</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {defs.map((d) => (
                  <tr key={d.id} className="tr-row-hover">
                    <td>{d.name}</td>
                    <td className="tr-mono">{d.strategy_id}</td>
                    <td className="tr-mono">{t(`strategies.lifecycle_${d.lifecycle}`)}</td>
                    <td style={{ textAlign: "right", display: "flex", gap: 8, justifyContent: "flex-end" }}>
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => {
                          setDefId(d.id);
                          setDefName(d.name);
                          setDefLifecycle(d.lifecycle);
                          setSelectedId(d.strategy_id);
                          setParams(d.params as any);
                        }}
                      >
                        {t("common.edit")}
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => eng.startStrategyDef(d.id).catch((e) => alert(String(e)))}
                      >
                        {t("strategies.run_saved")}
                      </Button>
                      <Button
                        size="sm"
                        variant="danger"
                        onClick={() => {
                          if (!confirm(t("strategies.delete_confirm"))) return;
                          eng
                            .deleteStrategyDef(d.id)
                            .then(() => refreshDefs())
                            .catch((e) => alert(String(e)));
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
            <div style={{ color: "var(--muted)", fontSize: 12 }}>{t("strategies.no_saved")}</div>
          )}
        </div>
      </Card>

      <Card style={{ padding: 14 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <div style={{ fontWeight: 700 }}>{t("strategies.studio")}</div>
          <Badge tone="neutral">{t("strategies.badge")}</Badge>
        </div>

        <div style={{ marginTop: 12 }} className="tr-grid">
          <div style={{ gridColumn: "span 4" }}>
            <Field label={t("strategies.strategy")}>
              <Select
                value={selectedId}
                onChange={(e) => {
                  const id = e.currentTarget.value;
                  setSelectedId(id);
                  const meta = catalog?.[id];
                  if (meta) setParams(initFromSchema(meta.params_schema));
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
          <div style={{ gridColumn: "span 8" }}>
            {selected ? (
              <div style={{ padding: "10px 12px", border: "1px solid rgba(255,255,255,0.10)", borderRadius: 12, background: "rgba(0,0,0,0.18)" }}>
                <div style={{ fontWeight: 700 }}>{displayStrategyName(selected.id, selected.name)}</div>
                <div style={{ marginTop: 6, color: "var(--muted)", fontSize: 12 }}>
                  {displayStrategyDescription(selected.id, selected.description)}
                </div>
                <div style={{ marginTop: 8, color: "var(--faint)", fontSize: 12 }}>
                  {t("common.id")}: <span className="tr-mono">{selected.id}</span>
                </div>
              </div>
            ) : (
              <div style={{ color: "var(--muted)", fontSize: 12 }}>{t("strategies.loading_catalog")}</div>
            )}
          </div>
        </div>
      </Card>

      <Card style={{ padding: 14 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <div style={{ fontWeight: 700 }}>{t("strategies.parameters")}</div>
          <div style={{ display: "flex", gap: 8 }}>
            <Button
              variant="ghost"
              disabled={!selectedId}
              onClick={() =>
                eng
                  .startStrategy({ strategy_id: selectedId, params })
                  .catch((e) => alert(String(e)))
              }
            >
              {t("strategies.start")}
            </Button>
            <Button
              variant="primary"
              disabled={!selectedId}
              onClick={() =>
                eng
                  .upsertStrategyDef({
                    id: defId,
                    name: defName,
                    strategy_id: selectedId,
                    params,
                    lifecycle: defLifecycle,
                  })
                  .then((saved) => {
                    setDefId(saved.id);
                    return refreshDefs();
                  })
                  .catch((e) => alert(String(e)))
              }
            >
              {t("strategies.save")}
            </Button>
          </div>
        </div>
        <div style={{ marginTop: 12 }}>
          <div className="tr-grid" style={{ marginBottom: 12 }}>
            <div style={{ gridColumn: "span 6" }}>
              <Field label={t("strategies.def_name")}>
                <Input value={defName} onChange={(e) => setDefName(e.currentTarget.value)} />
              </Field>
            </div>
            <div style={{ gridColumn: "span 6" }}>
              <Field label={t("strategies.lifecycle")}>
                <Select value={defLifecycle} onChange={(e) => setDefLifecycle(e.currentTarget.value as any)}>
                  <option value="draft">{t("strategies.lifecycle_draft")}</option>
                  <option value="paper">{t("strategies.lifecycle_paper")}</option>
                  <option value="live">{t("strategies.lifecycle_live")}</option>
                </Select>
              </Field>
            </div>
          </div>
          {selected ? (
            <SchemaForm schema={selected.params_schema} value={params} onChange={setParams} />
          ) : (
            <div style={{ color: "var(--muted)", fontSize: 12 }}>{t("strategies.select_strategy")}</div>
          )}
        </div>
      </Card>

      <Card style={{ padding: 14 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <div style={{ fontWeight: 700 }}>{t("strategies.running")}</div>
          <Badge tone="neutral">{s?.strategies.length ?? 0}</Badge>
        </div>
        <div style={{ marginTop: 12 }}>
          {s?.strategies.length ? (
            <table className="tr-table">
              <thead>
                <tr>
                  <th>{t("strategies.instance")}</th>
                  <th>{t("strategies.strategy")}</th>
                  <th>{t("strategies.symbol")}</th>
                  <th>{t("strategies.started")}</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {s.strategies.map((st) => (
                  <tr key={st.instance_id} className="tr-row-hover">
                    <td className="tr-mono">{st.instance_id.slice(0, 8)}</td>
                    <td>{strategyLabel(st.strategy_id)}</td>
                    <td className="tr-mono">{st.symbol}</td>
                    <td>{new Date(st.started_at).toLocaleString()}</td>
                    <td style={{ textAlign: "right" }}>
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => eng.stopStrategy(st.instance_id).catch((e) => alert(String(e)))}
                      >
                        {t("strategies.stop")}
                      </Button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <div style={{ color: "var(--muted)", fontSize: 12 }}>{t("strategies.no_running")}</div>
          )}
        </div>
      </Card>

      <Card style={{ padding: 14 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <div style={{ fontWeight: 700 }}>{t("strategies.explainability")}</div>
          <Badge tone="neutral">{recentSignals.length}</Badge>
        </div>
        <div style={{ marginTop: 12 }}>
          {recentSignals.length ? (
            <table className="tr-table">
              <thead>
                <tr>
                  <th>{t("strategies.signal_time")}</th>
                  <th>{t("strategies.strategy")}</th>
                  <th>{t("strategies.symbol")}</th>
                  <th>{t("strategies.signal_reason")}</th>
                  <th>{t("strategies.signal_order")}</th>
                </tr>
              </thead>
              <tbody>
                {recentSignals.map((sig, idx) => (
                  <tr key={idx} className="tr-row-hover">
                    <td>{new Date(sig.ts).toLocaleString()}</td>
                    <td>{strategyLabel(sig.strategy_id)}</td>
                    <td className="tr-mono">{sig.symbol}</td>
                    <td>{sig.reason}</td>
                    <td className="tr-mono">
                      {sig.order?.side ? t(`enum.side.${sig.order.side}`) : "-"} {sig.order?.qty ?? "-"}{" "}
                      {sig.order?.symbol ?? "-"}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <div style={{ color: "var(--muted)", fontSize: 12 }}>{t("strategies.no_signals")}</div>
          )}
        </div>
      </Card>
    </div>
  );
}
