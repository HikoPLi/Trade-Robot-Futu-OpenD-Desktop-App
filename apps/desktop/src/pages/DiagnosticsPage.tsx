import { Badge, Button, Card } from "../components/ui";
import { useEngine } from "../lib/engineContext";
import { useTranslation } from "react-i18next";

export function DiagnosticsPage() {
  const { t } = useTranslation();
  const eng = useEngine();
  const s = eng.snapshot;

  return (
    <div style={{ display: "grid", gap: 12 }}>
      <Card style={{ padding: 14 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <div style={{ fontWeight: 700 }}>{t("diagnostics.title")}</div>
          <div style={{ display: "flex", gap: 8 }}>
            <Button size="sm" variant="ghost" onClick={() => eng.refresh().catch(() => {})}>
              {t("common.refresh")}
            </Button>
            <Badge tone="neutral">{t("diagnostics.events_buffered", { count: eng.events.length })}</Badge>
          </div>
        </div>

        <div style={{ marginTop: 12, display: "grid", gap: 8, fontSize: 12, color: "var(--muted)" }}>
          <div>
            {t("diagnostics.profile")}: <span className="tr-mono">{s?.config.active_profile ?? "-"}</span>
          </div>
          <div>
            {t("diagnostics.mode")}: <span className="tr-mono">{s?.active_profile.mode ?? "-"}</span>
          </div>
          <div>
            {t("diagnostics.safe_mode")}: <span className="tr-mono">{s?.safe_mode ? "true" : "false"}</span>
          </div>
          <div>
            {t("diagnostics.kill_switch")}:{" "}
            <span className="tr-mono">{s?.kill_switch_engaged ? "true" : "false"}</span>
          </div>
          <div>
            {t("diagnostics.hotkey")}:{" "}
            <span className="tr-mono">{s?.active_profile.kill_switch_hotkey ?? "-"}</span>
          </div>
        </div>
      </Card>

      <Card style={{ padding: 14 }}>
        <div style={{ fontWeight: 700 }}>{t("diagnostics.raw_snapshot")}</div>
        <div style={{ marginTop: 10 }}>
          <pre
            style={{
              margin: 0,
              padding: 12,
              borderRadius: 12,
              border: "1px solid rgba(255,255,255,0.10)",
              background: "rgba(0,0,0,0.22)",
              overflow: "auto",
              maxHeight: 520,
            }}
          >
            {JSON.stringify(s, null, 2)}
          </pre>
        </div>
      </Card>
    </div>
  );
}
