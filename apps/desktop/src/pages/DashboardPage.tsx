import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Badge, Button, Card } from "../components/ui";
import { useEngine } from "../lib/engineContext";

function fmtUsd(n: number) {
  const sign = n < 0 ? "-" : "";
  const v = Math.abs(n);
  return `${sign}$${v.toFixed(2)}`;
}

export function DashboardPage() {
  const { t, i18n } = useTranslation();
  const eng = useEngine();
  const s = eng.snapshot;

  const kpis = useMemo(() => {
    if (!s) return null;
    return [
      { label: t("dashboard.equity"), value: fmtUsd(s.equity) },
      { label: t("dashboard.cash"), value: fmtUsd(s.cash) },
      { label: t("dashboard.realized_pnl"), value: fmtUsd(s.realized_pnl) },
      { label: t("dashboard.unrealized_pnl"), value: fmtUsd(s.unrealized_pnl) },
    ];
  }, [s, i18n.language]);

  return (
    <div style={{ display: "grid", gap: 12 }}>
      <div className="tr-grid">
        {kpis
          ? kpis.map((k) => (
              <Card key={k.label} className="tr-kpi" style={{ gridColumn: "span 3" }}>
                <div className="tr-kpi__label">{k.label}</div>
                <div className="tr-kpi__value">{k.value}</div>
              </Card>
            ))
          : null}

        <Card className="tr-kpi" style={{ gridColumn: "span 6" }}>
          <div className="tr-kpi__label">{t("dashboard.status")}</div>
          <div style={{ marginTop: 10, display: "flex", gap: 10, flexWrap: "wrap" }}>
            <Badge tone={s?.status.type === "running" ? "good" : "bad"}>
              {t("dashboard.engine", { status: s?.status.type ?? "unknown" })}
            </Badge>
            <Badge tone="info">{t("dashboard.mode", { mode: s?.active_profile.mode ?? "paper" })}</Badge>
            {s?.safe_mode ? <Badge tone="warn">{t("dashboard.safe_mode")}</Badge> : null}
            {s?.kill_switch_engaged ? <Badge tone="bad">{t("dashboard.kill_switch_engaged")}</Badge> : null}
          </div>
          {s?.status.type === "halted" ? (
            <div style={{ marginTop: 10, color: "rgba(255,77,109,0.92)" }}>
              {t("dashboard.halt_reason", { reason: s.status.reason })}
            </div>
          ) : null}
        </Card>
      </div>

      <div className="tr-split">
        <Card style={{ padding: 14 }}>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
            <div style={{ fontWeight: 700 }}>{t("dashboard.running_strategies")}</div>
            <Badge tone="neutral">{s?.strategies.length ?? 0}</Badge>
          </div>
          <div style={{ marginTop: 12 }}>
            {s?.strategies.length ? (
              <table className="tr-table">
                <thead>
                  <tr>
                    <th>{t("dashboard.instance")}</th>
                    <th>{t("dashboard.strategy")}</th>
                    <th>{t("dashboard.symbol")}</th>
                    <th />
                  </tr>
                </thead>
                <tbody>
                  {s.strategies.map((st) => (
                    <tr key={st.instance_id} className="tr-row-hover">
                      <td className="tr-mono">{st.instance_id.slice(0, 8)}</td>
                      <td>{st.strategy_id}</td>
                      <td className="tr-mono">{st.symbol}</td>
                      <td style={{ textAlign: "right" }}>
                        <Button
                          size="sm"
                          variant="ghost"
                          onClick={() => eng.stopStrategy(st.instance_id).catch((e) => alert(String(e)))}
                        >
                          {t("common.stop")}
                        </Button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            ) : (
              <div style={{ color: "var(--muted)", fontSize: 12 }}>
                {t("dashboard.no_strategies")}
              </div>
            )}
          </div>
        </Card>

        <Card style={{ padding: 14 }}>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
            <div style={{ fontWeight: 700 }}>{t("dashboard.diagnostics_live")}</div>
            <Button size="sm" variant="ghost" onClick={() => eng.refresh().catch(() => {})}>
              {t("common.refresh")}
            </Button>
          </div>
          <div style={{ marginTop: 12, display: "grid", gap: 8, fontSize: 12 }}>
            <div>
              {t("dashboard.quote_updates")}: <span className="tr-mono">{s?.metrics.quote_updates ?? 0}</span>
            </div>
            <div>
              {t("dashboard.candles_built")}: <span className="tr-mono">{s?.metrics.candles_built ?? 0}</span>
            </div>
            <div>
              {t("dashboard.orders_placed")}: <span className="tr-mono">{s?.metrics.orders_placed ?? 0}</span>
            </div>
            <div>
              {t("dashboard.orders_rejected")}: <span className="tr-mono">{s?.metrics.orders_rejected ?? 0}</span>
            </div>
            <div>
              {t("dashboard.fills")}: <span className="tr-mono">{s?.metrics.fills ?? 0}</span>
            </div>
          </div>
        </Card>
      </div>

      <Card style={{ padding: 14 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <div style={{ fontWeight: 700 }}>{t("dashboard.event_stream_tail")}</div>
          <Badge tone="neutral">{eng.events.length}</Badge>
        </div>
        <div style={{ marginTop: 10, fontSize: 12, color: "var(--muted)" }}>
          {t("dashboard.event_stream_help")}
        </div>
        <div style={{ marginTop: 10, display: "grid", gap: 8 }}>
          {eng.events
            .slice(-10)
            .reverse()
            .map((evt, idx) => (
              <pre
                key={idx}
                style={{
                  margin: 0,
                  padding: 10,
                  border: "1px solid rgba(255,255,255,0.10)",
                  borderRadius: 12,
                  background: "rgba(0,0,0,0.22)",
                  overflow: "auto",
                }}
              >
                {JSON.stringify(evt, null, 2)}
              </pre>
            ))}
        </div>
      </Card>
    </div>
  );
}
