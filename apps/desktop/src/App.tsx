import { NavLink, Route, Routes } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { Badge, Button, cx } from "./components/ui";
import { useEngine } from "./lib/engineContext";
import { AuditPage } from "./pages/AuditPage";
import { BacktestPage } from "./pages/BacktestPage";
import { DashboardPage } from "./pages/DashboardPage";
import { DiagnosticsPage } from "./pages/DiagnosticsPage";
import { MarketPage } from "./pages/MarketPage";
import { ModelsPage } from "./pages/ModelsPage";
import { SettingsPage } from "./pages/SettingsPage";
import { StrategiesPage } from "./pages/StrategiesPage";
import { TradingPage } from "./pages/TradingPage";
import { GuidePage } from "./pages/GuidePage";

function NavItem({ to, label }: { to: string; label: string }) {
  return (
    <NavLink to={to} className={({ isActive }) => cx(isActive && "tr-nav__active")}>
      <span>{label}</span>
    </NavLink>
  );
}

function Topbar() {
  const { t } = useTranslation();
  const eng = useEngine();
  const s = eng.snapshot;

  const status = s?.status?.type ?? "unknown";
  const statusTone = status === "running" ? "good" : status === "halted" ? "bad" : "neutral";

  const profileMode = s?.active_profile?.mode ?? "paper";
  const profileTone = profileMode === "paper" ? "good" : profileMode === "research" ? "info" : "warn";

  return (
    <div className="tr-topbar">
      <div className="tr-topbar__left">
        <div className="tr-topbar__title">{t("app.name")}</div>
        <Badge tone={profileTone as any}>{t("topbar.profile", { profile: profileMode })}</Badge>
        <Badge tone={statusTone as any}>{t("topbar.engine", { status })}</Badge>
        {s?.safe_mode ? <Badge tone="warn">{t("topbar.safe_mode")}</Badge> : null}
        {s?.kill_switch_engaged ? <Badge tone="bad">{t("topbar.kill_switch_engaged")}</Badge> : null}
      </div>
      <div className="tr-topbar__right">
        <Button
          variant="danger"
          size="sm"
          onClick={() => eng.engageKillSwitch("User engaged kill switch in UI").catch((e) => alert(String(e)))}
        >
          {t("topbar.kill_switch_button")}
        </Button>
      </div>
    </div>
  );
}

export default function App() {
  const { t } = useTranslation();
  const eng = useEngine();
  return (
    <div className="tr-shell">
      <aside className="tr-sidebar">
        <div className="tr-brand">
          <div className="tr-brand__title">{t("app.name")}</div>
          <div className="tr-brand__sub">{t("app.tagline")}</div>
        </div>
        <nav className="tr-nav">
          <NavItem to="/" label={t("nav.dashboard")} />
          <NavItem to="/market" label={t("nav.market")} />
          <NavItem to="/trading" label={t("nav.trading")} />
          <NavItem to="/strategies" label={t("nav.strategies")} />
          <NavItem to="/models" label={t("nav.models")} />
          <NavItem to="/backtest" label={t("nav.backtest")} />
          <NavItem to="/audit" label={t("nav.audit")} />
          <NavItem to="/settings" label={t("nav.settings")} />
          <NavItem to="/guide" label={t("nav.guide")} />
          <NavItem to="/diagnostics" label={t("nav.diagnostics")} />
        </nav>

        <div style={{ marginTop: 14, padding: "0 10px", color: "var(--muted)", fontSize: 12, lineHeight: 1.4 }}>
          {eng.error ? (
            <div style={{ color: "rgba(255,77,109,0.95)" }}>
              {t("app.backend_error")}
              <div className="tr-mono" style={{ marginTop: 6 }}>
                {eng.error}
              </div>
            </div>
          ) : null}
        </div>
      </aside>

      <main className="tr-main">
        <Topbar />
        <div className="tr-content">
          <Routes>
            <Route path="/" element={<DashboardPage />} />
            <Route path="/market" element={<MarketPage />} />
            <Route path="/trading" element={<TradingPage />} />
            <Route path="/strategies" element={<StrategiesPage />} />
            <Route path="/models" element={<ModelsPage />} />
            <Route path="/backtest" element={<BacktestPage />} />
            <Route path="/audit" element={<AuditPage />} />
            <Route path="/settings" element={<SettingsPage />} />
            <Route path="/guide" element={<GuidePage />} />
            <Route path="/diagnostics" element={<DiagnosticsPage />} />
          </Routes>
        </div>
      </main>
    </div>
  );
}
