import { useEffect, useMemo, useState } from "react";
import {
  EnableLiveTradingPhrase,
  type AiProviderConfig,
  type AiRouterConfig,
  type OpenDConfig,
  type OpenDTradeEnv,
  type RiskLimits,
  type SessionWindowUtc,
  type TimeControls,
} from "@trade-robot/shared";
import { useTranslation } from "react-i18next";
import { Badge, Button, Card, Field, Input, Select } from "../components/ui";
import { useEngine } from "../lib/engineContext";
import { getPreferredLanguage, setPreferredLanguage, type SupportedLanguage } from "../i18n";

const DEFAULT_RISK: RiskLimits = {
  max_position_qty: 100,
  max_order_qty: 25,
  max_orders_per_minute: 10,
  max_daily_loss_usd: 50.0,
  symbol_allowlist: [],
  allowed_markets: [],
  price_band_pct: 0.1,
};

function riskNonDefault(r: RiskLimits) {
  return JSON.stringify(r) !== JSON.stringify(DEFAULT_RISK);
}

function defaultAiProvider(id: string): AiProviderConfig {
  const lower = id.toLowerCase();
  if (lower === "deepseek") {
    return {
      id: "deepseek",
      kind: "deepseek",
      enabled: false,
      base_url: "https://api.deepseek.com/v1",
      model: "deepseek-chat",
      api_key_secret: "ai.deepseek_api_key",
      timeout_ms: 6000,
      max_tokens: 180,
      temperature: 0,
    };
  }
  if (lower === "qwen") {
    return {
      id: "qwen",
      kind: "qwen",
      enabled: false,
      base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
      model: "qwen-turbo",
      api_key_secret: "ai.qwen_api_key",
      timeout_ms: 6000,
      max_tokens: 180,
      temperature: 0,
    };
  }
  if (lower === "grok") {
    return {
      id: "grok",
      kind: "grok",
      enabled: false,
      base_url: "https://api.x.ai/v1",
      model: "grok-2-latest",
      api_key_secret: "ai.grok_api_key",
      timeout_ms: 6000,
      max_tokens: 180,
      temperature: 0,
    };
  }
  if (lower === "ollama") {
    return {
      id: "ollama",
      kind: "ollama",
      enabled: false,
      base_url: "http://127.0.0.1:11434",
      model: "llama3.1:8b",
      api_key_secret: "ai.ollama_api_key",
      timeout_ms: 4000,
      max_tokens: 120,
      temperature: 0,
    };
  }
  return {
    id: "openai",
    kind: "openai",
    enabled: false,
    base_url: "https://api.openai.com/v1",
    model: "gpt-4o-mini",
    api_key_secret: "ai.openai_api_key",
    timeout_ms: 6000,
    max_tokens: 180,
    temperature: 0,
  };
}

export function SettingsPage() {
  const { t, i18n } = useTranslation();
  const eng = useEngine();
  const s = eng.snapshot;
  const ap = s?.active_profile;

  const [profile, setProfile] = useState<string>(s?.config.active_profile ?? "paper");
  const [language, setLanguage] = useState<SupportedLanguage>(getPreferredLanguage());

  const [risk, setRisk] = useState<RiskLimits>(ap?.risk ?? DEFAULT_RISK);
  const [allowlistText, setAllowlistText] = useState<string>((ap?.risk.symbol_allowlist ?? []).join(", "));
  const [allowedMarketsText, setAllowedMarketsText] = useState<string>((ap?.risk.allowed_markets ?? []).join(", "));

  const [hotkey, setHotkey] = useState<string>(ap?.kill_switch_hotkey ?? "CmdOrCtrl+Alt+K");
  const [opend, setOpend] = useState<OpenDConfig>(
    ap?.opend ?? { host: "127.0.0.1", port: 11111, use_tls: false },
  );
  const [trdEnv, setTrdEnv] = useState<OpenDTradeEnv>(ap?.opend_trd_env ?? "simulate");
  const [timeControls, setTimeControls] = useState<TimeControls>(
    ap?.time_controls ?? { enabled: false, sessions_utc: [], blackout_utc: [], cooldown_sec: 0 },
  );
  const [aiProviders, setAiProviders] = useState<Record<string, AiProviderConfig>>(
    ap?.ai_providers ?? {
      openai: defaultAiProvider("openai"),
      deepseek: defaultAiProvider("deepseek"),
      qwen: defaultAiProvider("qwen"),
      grok: defaultAiProvider("grok"),
      ollama: defaultAiProvider("ollama"),
    },
  );
  const [aiRouter, setAiRouter] = useState<AiRouterConfig>(
    ap?.ai_router ?? { primary: "openai", fallbacks: ["deepseek", "qwen", "grok", "ollama"] },
  );
  const [aiFallbackText, setAiFallbackText] = useState<string>((ap?.ai_router?.fallbacks ?? []).join(", "));
  const [selectedAiProvider, setSelectedAiProvider] = useState<string>("openai");
  const [aiTestResult, setAiTestResult] = useState<string>("");

  const [unlockPhrase, setUnlockPhrase] = useState("");
  const [disclaimerRead, setDisclaimerRead] = useState(false);

  const [secretKey, setSecretKey] = useState<string>("futu.trade_password");
  const [secretValue, setSecretValue] = useState<string>("");
  const [secretStored, setSecretStored] = useState<boolean | null>(null);

  useEffect(() => {
    if (!s) return;
    setProfile(s.config.active_profile);
    setRisk(s.active_profile.risk);
    setAllowlistText(s.active_profile.risk.symbol_allowlist.join(", "));
    setAllowedMarketsText(s.active_profile.risk.allowed_markets.join(", "));
    setHotkey(s.active_profile.kill_switch_hotkey);
    // OpenD protocol doesn't support TLS directly; remote access should use SSH/VPN.
    setOpend({ ...s.active_profile.opend, use_tls: false });
    setTrdEnv(s.active_profile.opend_trd_env);
    setTimeControls(s.active_profile.time_controls);
    setAiProviders(s.active_profile.ai_providers);
    setAiRouter(s.active_profile.ai_router);
    setAiFallbackText((s.active_profile.ai_router?.fallbacks ?? []).join(", "));
    const providerIds = Object.keys(s.active_profile.ai_providers ?? {});
    setSelectedAiProvider((prev) =>
      providerIds.includes(prev) ? prev : providerIds[0] ?? "openai",
    );
    setAiTestResult("");
    setUnlockPhrase("");
    setDisclaimerRead(false);
    setSecretValue("");
  }, [s?.config.active_profile]);

  useEffect(() => {
    // Keep selector in sync when language is changed elsewhere.
    setLanguage(getPreferredLanguage());
  }, [i18n.language]);

  useEffect(() => {
    let alive = true;
    (async () => {
      try {
        const ok = await eng.secretStatus(secretKey);
        if (!alive) return;
        setSecretStored(ok);
      } catch {
        if (!alive) return;
        setSecretStored(null);
      }
    })();
    return () => {
      alive = false;
    };
  }, [s?.config.active_profile, secretKey]);

  const allowlist = useMemo(() => {
    return allowlistText
      .split(",")
      .map((x) => x.trim().toUpperCase())
      .filter(Boolean);
  }, [allowlistText]);

  const allowedMarkets = useMemo(() => {
    return allowedMarketsText
      .split(",")
      .map((x) => x.trim().toUpperCase())
      .filter(Boolean);
  }, [allowedMarketsText]);

  const aiProviderIds = useMemo(() => {
    return Object.keys(aiProviders).sort();
  }, [aiProviders]);

  const aiProvider = useMemo<AiProviderConfig>(() => {
    const id = selectedAiProvider || "openai";
    return aiProviders[id] ?? { ...defaultAiProvider(id), id };
  }, [aiProviders, selectedAiProvider]);

  function patchAiProvider(patch: Partial<AiProviderConfig>) {
    const id = selectedAiProvider || "openai";
    setAiProviders((prev) => {
      const base = prev[id] ?? { ...defaultAiProvider(id), id };
      return {
        ...prev,
        [id]: {
          ...base,
          ...patch,
          id,
        },
      };
    });
  }

  const killSwitchConfigured = hotkey.trim().length > 0;
  const riskConfigured = riskNonDefault({ ...risk, symbol_allowlist: allowlist, allowed_markets: allowedMarkets });

  function parseWeekdays(text: string): number[] {
    const t = (text || "").trim();
    if (!t) return [];
    // Accept "1-5" shorthand.
    const m = t.match(/^\\s*(\\d)\\s*-\\s*(\\d)\\s*$/);
    if (m) {
      const a = Number(m[1]);
      const b = Number(m[2]);
      const out: number[] = [];
      const lo = Math.min(a, b);
      const hi = Math.max(a, b);
      for (let i = lo; i <= hi; i++) out.push(i);
      return out;
    }
    return t
      .split(",")
      .map((x) => Number(x.trim()))
      .filter((n) => Number.isFinite(n) && n >= 1 && n <= 7);
  }

  function fmtWeekdays(days: number[]): string {
    if (!days?.length) return "";
    return days.join(",");
  }

  function updateWindow(listKey: "sessions_utc" | "blackout_utc", idx: number, patch: Partial<SessionWindowUtc>) {
    const nextList = [...timeControls[listKey]];
    nextList[idx] = { ...nextList[idx], ...patch };
    setTimeControls({ ...timeControls, [listKey]: nextList });
  }

  function addWindow(listKey: "sessions_utc" | "blackout_utc") {
    const w: SessionWindowUtc = { weekdays: [1, 2, 3, 4, 5], start_hhmm: "09:30", end_hhmm: "16:00" };
    setTimeControls({ ...timeControls, [listKey]: [...timeControls[listKey], w] });
  }

  function removeWindow(listKey: "sessions_utc" | "blackout_utc", idx: number) {
    const nextList = [...timeControls[listKey]];
    nextList.splice(idx, 1);
    setTimeControls({ ...timeControls, [listKey]: nextList });
  }

  return (
    <div style={{ display: "grid", gap: 12 }}>
      <Card style={{ padding: 14 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <div style={{ fontWeight: 700 }}>{t("settings.language")}</div>
          <Badge tone="neutral" className="tr-mono">
            {language}
          </Badge>
        </div>

        <div style={{ marginTop: 12 }} className="tr-grid">
          <div style={{ gridColumn: "span 6" }}>
            <Field label={t("settings.language")} hint={t("settings.language_hint")}>
              <Select
                value={language}
                onChange={(e) => {
                  const lang = e.currentTarget.value as SupportedLanguage;
                  setLanguage(lang);
                  setPreferredLanguage(lang).catch((err) => alert(String(err)));
                }}
              >
                <option value="en">{t("language.en")}</option>
                <option value="zh-CN">{t("language.zh_cn")}</option>
                <option value="zh-TW">{t("language.zh_tw")}</option>
              </Select>
            </Field>
          </div>
        </div>
      </Card>

      <Card style={{ padding: 14 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <div style={{ fontWeight: 700 }}>{t("settings.profiles")}</div>
          <Badge tone="neutral">{t("settings.profiles_badge")}</Badge>
        </div>

        <div style={{ marginTop: 12 }} className="tr-grid">
          <div style={{ gridColumn: "span 4" }}>
            <Field label={t("settings.active_profile")}>
              <Select value={profile} onChange={(e) => setProfile(e.currentTarget.value)}>
                <option value="paper">{t("profile.paper")}</option>
                <option value="research">{t("profile.research")}</option>
                <option value="live">{t("profile.live")}</option>
              </Select>
            </Field>
          </div>
          <div style={{ gridColumn: "span 8", display: "flex", alignItems: "flex-end", justifyContent: "flex-end" }}>
            <Button
              variant="primary"
              onClick={() => eng.setActiveProfile(profile).catch((e) => alert(String(e)))}
            >
              {t("settings.switch_profile")}
            </Button>
          </div>
        </div>

        {ap?.mode === "live" ? (
          <div style={{ marginTop: 12, color: "rgba(255,207,90,0.92)", fontSize: 12, lineHeight: 1.4 }}>
            {t("settings.live_gated_note")}
          </div>
        ) : null}
      </Card>

      <Card style={{ padding: 14 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <div style={{ fontWeight: 700 }}>{t("settings.risk_limits")}</div>
          <Badge tone={riskConfigured ? "good" : "warn"}>
            {riskConfigured ? t("settings.configured") : t("settings.default")}
          </Badge>
        </div>

        <div style={{ marginTop: 12 }} className="tr-grid">
          <div style={{ gridColumn: "span 3" }}>
            <Field label={t("settings.max_position_qty")}>
              <Input
                type="number"
                min={0}
                step={1}
                value={risk.max_position_qty}
                onChange={(e) => setRisk({ ...risk, max_position_qty: Number(e.currentTarget.value) })}
              />
            </Field>
          </div>
          <div style={{ gridColumn: "span 3" }}>
            <Field label={t("settings.max_order_qty")}>
              <Input
                type="number"
                min={0}
                step={1}
                value={risk.max_order_qty}
                onChange={(e) => setRisk({ ...risk, max_order_qty: Number(e.currentTarget.value) })}
              />
            </Field>
          </div>
          <div style={{ gridColumn: "span 3" }}>
            <Field label={t("settings.max_orders_min")}>
              <Input
                type="number"
                min={1}
                step={1}
                value={risk.max_orders_per_minute}
                onChange={(e) => setRisk({ ...risk, max_orders_per_minute: Number(e.currentTarget.value) })}
              />
            </Field>
          </div>
          <div style={{ gridColumn: "span 3" }}>
            <Field label={t("settings.max_daily_loss")}>
              <Input
                type="number"
                min={0}
                step={1}
                value={risk.max_daily_loss_usd}
                onChange={(e) => setRisk({ ...risk, max_daily_loss_usd: Number(e.currentTarget.value) })}
              />
            </Field>
          </div>

          <div style={{ gridColumn: "span 3" }}>
            <Field label={t("settings.price_band_pct")} hint={t("settings.price_band_hint")}>
              <Input
                type="number"
                min={0}
                max={1}
                step={0.01}
                value={risk.price_band_pct}
                onChange={(e) => setRisk({ ...risk, price_band_pct: Number(e.currentTarget.value) })}
              />
            </Field>
          </div>

          <div style={{ gridColumn: "span 3" }}>
            <Field label={t("settings.allowed_markets")} hint={t("settings.allowed_markets_hint")}>
              <Input value={allowedMarketsText} onChange={(e) => setAllowedMarketsText(e.currentTarget.value)} />
            </Field>
          </div>

          <div style={{ gridColumn: "span 6" }}>
            <Field label={t("settings.symbol_allowlist")} hint={t("settings.allowlist_hint")}>
              <Input value={allowlistText} onChange={(e) => setAllowlistText(e.currentTarget.value)} />
            </Field>
          </div>

          <div style={{ gridColumn: "span 12", display: "flex", justifyContent: "flex-end" }}>
            <Button
              variant="primary"
              onClick={() =>
                eng
                  .updateRiskLimits({ ...risk, symbol_allowlist: allowlist, allowed_markets: allowedMarkets })
                  .catch((e) => alert(String(e)))
              }
            >
              {t("settings.save_risk")}
            </Button>
          </div>
        </div>
      </Card>

      <Card style={{ padding: 14 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <div style={{ fontWeight: 700 }}>{t("settings.kill_switch_hotkey")}</div>
          <Badge tone={killSwitchConfigured ? "good" : "bad"}>
            {killSwitchConfigured ? t("settings.configured_badge") : t("settings.missing_badge")}
          </Badge>
        </div>
        <div style={{ marginTop: 12 }} className="tr-grid">
          <div style={{ gridColumn: "span 6" }}>
            <Field label={t("settings.hotkey")} hint={t("settings.hotkey_hint")}>
              <Input value={hotkey} onChange={(e) => setHotkey(e.currentTarget.value)} />
            </Field>
          </div>
          <div style={{ gridColumn: "span 6", display: "flex", alignItems: "flex-end", justifyContent: "flex-end" }}>
            <Button variant="primary" onClick={() => eng.setKillSwitchHotkey(hotkey).catch((e) => alert(String(e)))}>
              {t("settings.apply_hotkey")}
            </Button>
          </div>
        </div>
        <div style={{ marginTop: 10, color: "var(--muted)", fontSize: 12, lineHeight: 1.4 }}>
          {t("settings.kill_switch_help")}
        </div>
      </Card>

      <Card style={{ padding: 14 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <div style={{ fontWeight: 700 }}>{t("settings.opend_connection")}</div>
          <Badge tone="neutral">{t("settings.opend_badge")}</Badge>
        </div>
        <div style={{ marginTop: 12 }} className="tr-grid">
          <div style={{ gridColumn: "span 4" }}>
            <Field label={t("settings.host")}>
              <Input value={opend.host} onChange={(e) => setOpend({ ...opend, host: e.currentTarget.value })} />
            </Field>
          </div>
          <div style={{ gridColumn: "span 2" }}>
            <Field label={t("settings.port")}>
              <Input
                type="number"
                value={opend.port}
                onChange={(e) => setOpend({ ...opend, port: Number(e.currentTarget.value) })}
              />
            </Field>
          </div>
          <div style={{ gridColumn: "span 2" }}>
            <Field label={t("settings.trade_env")} hint={t("settings.trade_env_hint")}>
              <Select value={trdEnv} onChange={(e) => setTrdEnv(e.currentTarget.value as OpenDTradeEnv)}>
                <option value="simulate">{t("settings.trade_env_simulate")}</option>
                <option value="real">{t("settings.trade_env_real")}</option>
              </Select>
            </Field>
          </div>
          <div style={{ gridColumn: "span 2" }}>
            <Field label={t("settings.tls")} hint={t("settings.tls_hint")}>
              <Select value={"false"} disabled>
                <option value="false">{t("settings.tls_off")}</option>
                <option value="true">{t("settings.tls_on")}</option>
              </Select>
            </Field>
          </div>
          <div style={{ gridColumn: "span 2", display: "flex", alignItems: "flex-end", justifyContent: "flex-end", gap: 8 }}>
            <Button
              variant="ghost"
              onClick={() => eng.testOpenDConnection().then(() => alert(t("common.ok"))).catch((e) => alert(String(e)))}
            >
              {t("common.test")}
            </Button>
            <Button variant="primary" onClick={() => eng.updateOpenDConfig(opend).catch((e) => alert(String(e)))}>
              {t("common.save")}
            </Button>
          </div>
        </div>
        <div style={{ marginTop: 10, fontSize: 12, color: "var(--muted)", lineHeight: 1.5 }}>
          {t("settings.opend_note")}
        </div>
        <div style={{ marginTop: 10, display: "flex", justifyContent: "flex-end" }}>
          <Button
            variant="primary"
            onClick={() =>
              eng
                .updateOpenDTradeEnv(trdEnv)
                .then(() => alert(t("settings.trade_env_saved_alert")))
                .catch((e) => alert(String(e)))
            }
          >
            {t("settings.apply_trade_env")}
          </Button>
        </div>
      </Card>

      <Card style={{ padding: 14 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <div style={{ fontWeight: 700 }}>{t("settings.ai_models")}</div>
          <Badge tone="neutral">{t("settings.ai_badge")}</Badge>
        </div>
        <div style={{ marginTop: 10, fontSize: 12, color: "var(--muted)", lineHeight: 1.5 }}>
          {t("settings.ai_help")}
        </div>

        <div style={{ marginTop: 12 }} className="tr-grid">
          <div style={{ gridColumn: "span 3" }}>
            <Field label={t("settings.ai_provider")}>
              <Select value={selectedAiProvider} onChange={(e) => setSelectedAiProvider(e.currentTarget.value)}>
                {aiProviderIds.map((id) => (
                  <option key={id} value={id}>
                    {id}
                  </option>
                ))}
              </Select>
            </Field>
          </div>
          <div style={{ gridColumn: "span 3" }}>
            <Field label={t("settings.enabled")}>
              <Select
                value={aiProvider.enabled ? "true" : "false"}
                onChange={(e) => patchAiProvider({ enabled: e.currentTarget.value === "true" })}
              >
                <option value="false">{t("common.off")}</option>
                <option value="true">{t("common.on")}</option>
              </Select>
            </Field>
          </div>
          <div style={{ gridColumn: "span 3" }}>
            <Field label={t("settings.ai_kind")}>
              <Select
                value={aiProvider.kind}
                onChange={(e) => patchAiProvider({ kind: e.currentTarget.value as AiProviderConfig["kind"] })}
              >
                <option value="openai">openai</option>
                <option value="deepseek">deepseek</option>
                <option value="qwen">qwen</option>
                <option value="grok">grok</option>
                <option value="ollama">ollama</option>
                <option value="openai_compatible">openai_compatible</option>
              </Select>
            </Field>
          </div>
          <div style={{ gridColumn: "span 3" }}>
            <Field label={t("settings.ai_secret_key")}>
              <Input
                value={aiProvider.api_key_secret}
                onChange={(e) => patchAiProvider({ api_key_secret: e.currentTarget.value })}
              />
            </Field>
          </div>

          <div style={{ gridColumn: "span 4" }}>
            <Field label={t("settings.base_url")}>
              <Input value={aiProvider.base_url} onChange={(e) => patchAiProvider({ base_url: e.currentTarget.value })} />
            </Field>
          </div>
          <div style={{ gridColumn: "span 3" }}>
            <Field label={t("settings.ai_model")}>
              <Input value={aiProvider.model} onChange={(e) => patchAiProvider({ model: e.currentTarget.value })} />
            </Field>
          </div>
          <div style={{ gridColumn: "span 2" }}>
            <Field label={t("settings.ai_timeout_ms")}>
              <Input
                type="number"
                min={500}
                max={120000}
                value={aiProvider.timeout_ms}
                onChange={(e) => patchAiProvider({ timeout_ms: Number(e.currentTarget.value) })}
              />
            </Field>
          </div>
          <div style={{ gridColumn: "span 2" }}>
            <Field label={t("settings.ai_max_tokens")}>
              <Input
                type="number"
                min={1}
                max={32768}
                value={aiProvider.max_tokens}
                onChange={(e) => patchAiProvider({ max_tokens: Number(e.currentTarget.value) })}
              />
            </Field>
          </div>
          <div style={{ gridColumn: "span 1" }}>
            <Field label={t("settings.ai_temp")}>
              <Input
                type="number"
                min={0}
                max={2}
                step={0.1}
                value={aiProvider.temperature}
                onChange={(e) => patchAiProvider({ temperature: Number(e.currentTarget.value) })}
              />
            </Field>
          </div>

          <div style={{ gridColumn: "span 3" }}>
            <Field label={t("settings.ai_primary")}>
              <Select
                value={aiRouter.primary}
                onChange={(e) => setAiRouter({ ...aiRouter, primary: e.currentTarget.value })}
              >
                {aiProviderIds.map((id) => (
                  <option key={id} value={id}>
                    {id}
                  </option>
                ))}
              </Select>
            </Field>
          </div>
          <div style={{ gridColumn: "span 9" }}>
            <Field label={t("settings.ai_fallbacks")} hint={t("settings.ai_fallbacks_hint")}>
              <Input value={aiFallbackText} onChange={(e) => setAiFallbackText(e.currentTarget.value)} />
            </Field>
          </div>

          <div style={{ gridColumn: "span 12", display: "flex", justifyContent: "flex-end", gap: 8 }}>
            <Button
              variant="ghost"
              onClick={() =>
                eng
                  .testAiProvider(selectedAiProvider)
                  .then((res) => {
                    setAiTestResult(`${res.provider_id} · ${res.action} · ${(res.confidence * 100).toFixed(1)}% · ${res.reason}`);
                  })
                  .catch((e) => {
                    setAiTestResult(String(e));
                  })
              }
            >
              {t("settings.ai_test_provider")}
            </Button>
            <Button
              variant="primary"
              onClick={() =>
                eng
                  .updateAiProvider(aiProvider)
                  .then(() => alert(t("common.saved")))
                  .catch((e) => alert(String(e)))
              }
            >
              {t("settings.ai_save_provider")}
            </Button>
            <Button
              variant="primary"
              onClick={() => {
                const fallbacks = aiFallbackText
                  .split(",")
                  .map((x) => x.trim().toLowerCase())
                  .filter(Boolean);
                eng
                  .updateAiRouter({ ...aiRouter, primary: aiRouter.primary.trim().toLowerCase(), fallbacks })
                  .then(() => alert(t("common.saved")))
                  .catch((e) => alert(String(e)));
              }}
            >
              {t("settings.ai_save_router")}
            </Button>
          </div>
        </div>

        {aiTestResult ? (
          <div style={{ marginTop: 10, fontSize: 12, color: "var(--muted)" }}>
            {t("settings.ai_test_result")}: <span className="tr-mono">{aiTestResult}</span>
          </div>
        ) : null}
      </Card>

      <Card style={{ padding: 14 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <div style={{ fontWeight: 700 }}>{t("settings.time_controls")}</div>
          <Badge tone={timeControls.enabled ? "warn" : "neutral"}>
            {timeControls.enabled ? t("common.enabled") : t("common.disabled")}
          </Badge>
        </div>
        <div style={{ marginTop: 12 }} className="tr-grid">
          <div style={{ gridColumn: "span 3" }}>
            <Field label={t("settings.enabled")}>
              <Select
                value={timeControls.enabled ? "true" : "false"}
                onChange={(e) => setTimeControls({ ...timeControls, enabled: e.currentTarget.value === "true" })}
              >
                <option value="false">{t("common.off")}</option>
                <option value="true">{t("common.on")}</option>
              </Select>
            </Field>
          </div>
          <div style={{ gridColumn: "span 3" }}>
            <Field label={t("settings.cooldown_sec")} hint={t("settings.cooldown_hint")}>
              <Input
                type="number"
                min={0}
                step={1}
                value={timeControls.cooldown_sec}
                onChange={(e) => setTimeControls({ ...timeControls, cooldown_sec: Number(e.currentTarget.value) })}
              />
            </Field>
          </div>
          <div style={{ gridColumn: "span 6", display: "flex", alignItems: "flex-end", justifyContent: "flex-end" }}>
            <Button
              variant="primary"
              onClick={() => eng.updateTimeControls(timeControls).then(() => alert(t("common.saved"))).catch((e) => alert(String(e)))}
            >
              {t("settings.save_time_controls")}
            </Button>
          </div>

          <div style={{ gridColumn: "span 12", marginTop: 4, color: "var(--muted)", fontSize: 12 }}>
            {t("settings.time_controls_help")}
          </div>

          <div style={{ gridColumn: "span 12" }}>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <div style={{ fontWeight: 700, fontSize: 12 }}>{t("settings.sessions")}</div>
              <Button size="sm" variant="ghost" onClick={() => addWindow("sessions_utc")}>
                {t("common.add")}
              </Button>
            </div>
            <div style={{ marginTop: 8, display: "grid", gap: 8 }}>
              {timeControls.sessions_utc.length ? (
                timeControls.sessions_utc.map((w, idx) => (
                  <div key={idx} className="tr-grid">
                    <div style={{ gridColumn: "span 4" }}>
                      <Field label={t("settings.weekdays")} hint={t("settings.weekdays_hint")}>
                        <Input
                          value={fmtWeekdays(w.weekdays as any)}
                          onChange={(e) => updateWindow("sessions_utc", idx, { weekdays: parseWeekdays(e.currentTarget.value) as any })}
                        />
                      </Field>
                    </div>
                    <div style={{ gridColumn: "span 3" }}>
                      <Field label={t("settings.start_utc")}>
                        <Input value={w.start_hhmm} onChange={(e) => updateWindow("sessions_utc", idx, { start_hhmm: e.currentTarget.value })} />
                      </Field>
                    </div>
                    <div style={{ gridColumn: "span 3" }}>
                      <Field label={t("settings.end_utc")}>
                        <Input value={w.end_hhmm} onChange={(e) => updateWindow("sessions_utc", idx, { end_hhmm: e.currentTarget.value })} />
                      </Field>
                    </div>
                    <div style={{ gridColumn: "span 2", display: "flex", alignItems: "flex-end", justifyContent: "flex-end" }}>
                      <Button size="sm" variant="danger" onClick={() => removeWindow("sessions_utc", idx)}>
                        {t("common.remove")}
                      </Button>
                    </div>
                  </div>
                ))
              ) : (
                <div style={{ color: "var(--muted)", fontSize: 12 }}>{t("settings.no_sessions")}</div>
              )}
            </div>
          </div>

          <div style={{ gridColumn: "span 12", marginTop: 8 }}>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <div style={{ fontWeight: 700, fontSize: 12 }}>{t("settings.blackout")}</div>
              <Button size="sm" variant="ghost" onClick={() => addWindow("blackout_utc")}>
                {t("common.add")}
              </Button>
            </div>
            <div style={{ marginTop: 8, display: "grid", gap: 8 }}>
              {timeControls.blackout_utc.length ? (
                timeControls.blackout_utc.map((w, idx) => (
                  <div key={idx} className="tr-grid">
                    <div style={{ gridColumn: "span 4" }}>
                      <Field label={t("settings.weekdays")}>
                        <Input
                          value={fmtWeekdays(w.weekdays as any)}
                          onChange={(e) => updateWindow("blackout_utc", idx, { weekdays: parseWeekdays(e.currentTarget.value) as any })}
                        />
                      </Field>
                    </div>
                    <div style={{ gridColumn: "span 3" }}>
                      <Field label={t("settings.start_utc")}>
                        <Input value={w.start_hhmm} onChange={(e) => updateWindow("blackout_utc", idx, { start_hhmm: e.currentTarget.value })} />
                      </Field>
                    </div>
                    <div style={{ gridColumn: "span 3" }}>
                      <Field label={t("settings.end_utc")}>
                        <Input value={w.end_hhmm} onChange={(e) => updateWindow("blackout_utc", idx, { end_hhmm: e.currentTarget.value })} />
                      </Field>
                    </div>
                    <div style={{ gridColumn: "span 2", display: "flex", alignItems: "flex-end", justifyContent: "flex-end" }}>
                      <Button size="sm" variant="danger" onClick={() => removeWindow("blackout_utc", idx)}>
                        {t("common.remove")}
                      </Button>
                    </div>
                  </div>
                ))
              ) : (
                <div style={{ color: "var(--muted)", fontSize: 12 }}>{t("settings.no_blackout")}</div>
              )}
            </div>
          </div>
        </div>
      </Card>

      <Card style={{ padding: 14 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <div style={{ fontWeight: 700 }}>{t("settings.credentials")}</div>
          <Badge tone={secretStored ? "good" : secretStored === false ? "neutral" : "warn"}>
            {secretStored
              ? t("settings.secret_status_stored")
              : secretStored === false
                ? t("settings.secret_status_not_set")
                : t("settings.secret_status_unknown")}
          </Badge>
        </div>
        <div style={{ marginTop: 10, fontSize: 12, color: "var(--muted)", lineHeight: 1.5 }}>
          {t("settings.secrets_help")}
        </div>
        <div style={{ marginTop: 12 }} className="tr-grid">
          <div style={{ gridColumn: "span 4" }}>
            <Field label={t("settings.secret_type")}>
              <Select value={secretKey} onChange={(e) => setSecretKey(e.currentTarget.value)}>
                <option value="futu.trade_password">futu.trade_password</option>
                <option value="futu.api_token">futu.api_token</option>
                <option value="opend.tls_client_key_passphrase">opend.tls_client_key_passphrase</option>
                <option value="ai.openai_api_key">ai.openai_api_key</option>
                <option value="ai.deepseek_api_key">ai.deepseek_api_key</option>
                <option value="ai.qwen_api_key">ai.qwen_api_key</option>
                <option value="ai.grok_api_key">ai.grok_api_key</option>
                <option value="ai.ollama_api_key">ai.ollama_api_key</option>
                <option value="ai.custom_api_key">ai.custom_api_key</option>
              </Select>
            </Field>
          </div>
          <div style={{ gridColumn: "span 6" }}>
            <Field label={t("settings.value")}>
              <Input
                type="password"
                value={secretValue}
                placeholder={t("settings.secret_placeholder")}
                onChange={(e) => setSecretValue(e.currentTarget.value)}
              />
            </Field>
          </div>
          <div style={{ gridColumn: "span 2", display: "flex", alignItems: "flex-end", justifyContent: "flex-end", gap: 8 }}>
            <Button
              variant="ghost"
              onClick={() =>
                eng
                  .clearSecret(secretKey)
                  .then(async () => setSecretStored(await eng.secretStatus(secretKey)))
                  .catch((e) => alert(String(e)))
              }
            >
              {t("common.clear")}
            </Button>
            <Button
              variant="primary"
              disabled={!secretValue.trim()}
              onClick={() =>
                eng
                  .setSecret(secretKey, secretValue)
                  .then(async () => {
                    setSecretValue("");
                    setSecretStored(await eng.secretStatus(secretKey));
                    alert(t("settings.saved_to_keychain"));
                  })
                  .catch((e) => alert(String(e)))
              }
            >
              {t("common.save")}
            </Button>
          </div>
        </div>
      </Card>

      {ap?.mode === "live" ? (
        <Card style={{ padding: 14 }}>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
            <div style={{ fontWeight: 700 }}>{t("settings.enable_live_trading")}</div>
            <Badge tone={ap.live_trading_unlocked ? "warn" : "neutral"}>
              {ap.live_trading_unlocked ? t("settings.unlocked") : t("settings.locked")}
            </Badge>
          </div>

          <div style={{ marginTop: 10, fontSize: 12, color: "var(--muted)", lineHeight: 1.5 }}>
            {t("settings.live_workflow_intro")}
            <div style={{ marginTop: 8 }}>
              <div>{t("settings.live_step_1")}</div>
              <div>{t("settings.live_step_2")}</div>
              <div>{t("settings.live_step_3")}</div>
              <div>{t("settings.live_step_4")}</div>
              <div>{t("settings.live_step_5")}</div>
              <div>{t("settings.live_step_6")}</div>
            </div>
          </div>

          <div style={{ marginTop: 12, padding: 12, borderRadius: 12, border: "1px solid rgba(255,255,255,0.10)", background: "rgba(0,0,0,0.18)" }}>
            <div style={{ fontWeight: 700, fontSize: 12 }}>{t("settings.disclaimer_title")}</div>
            <div style={{ marginTop: 8, fontSize: 12, color: "var(--muted)", lineHeight: 1.5, whiteSpace: "pre-wrap" }}>
              {t("settings.disclaimer_text")}
            </div>
            <label style={{ marginTop: 10, display: "flex", gap: 10, alignItems: "center", fontSize: 12, color: "var(--muted)" }}>
              <input type="checkbox" checked={disclaimerRead} onChange={(e) => setDisclaimerRead(e.currentTarget.checked)} />
              <span>{t("settings.disclaimer_checkbox")}</span>
            </label>
          </div>

          <div style={{ marginTop: 12 }} className="tr-grid">
            <div style={{ gridColumn: "span 6" }}>
              <Field label={t("settings.confirmation_phrase")} hint={t("settings.confirmation_hint", { phrase: EnableLiveTradingPhrase })}>
                <Input value={unlockPhrase} onChange={(e) => setUnlockPhrase(e.currentTarget.value)} />
              </Field>
            </div>
            <div style={{ gridColumn: "span 6", display: "flex", alignItems: "flex-end", justifyContent: "flex-end" }}>
              <Button
                variant="danger"
                disabled={!disclaimerRead || !riskConfigured || !killSwitchConfigured || ap.live_trading_unlocked}
                onClick={() =>
                  eng
                    .enableLiveTradingUnlock({
                      confirmation_phrase: unlockPhrase,
                      risk_non_default: riskConfigured,
                      kill_switch_configured: killSwitchConfigured,
                    })
                    .then(() => alert(t("settings.live_unlocked_alert")))
                    .catch((e) => alert(String(e)))
                }
              >
                {t("settings.unlock")}
              </Button>
            </div>
          </div>

          <div style={{ marginTop: 10, fontSize: 12, color: "rgba(255,207,90,0.92)", lineHeight: 1.5 }}>
            {t("settings.live_warning")}
          </div>
        </Card>
      ) : null}
    </div>
  );
}
