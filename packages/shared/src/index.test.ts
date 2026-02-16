import { describe, expect, it } from "vitest";
import {
  AppConfigSchema,
  OpenDConfigSchema,
  ProfileConfigSchema,
  StrategyCatalogSchema,
} from "./index";

describe("@trade-robot/shared schemas", () => {
  it("parses OpenDConfig", () => {
    expect(
      OpenDConfigSchema.parse({ host: "127.0.0.1", port: 11111, use_tls: false }),
    ).toEqual({ host: "127.0.0.1", port: 11111, use_tls: false });
  });

  it("parses ProfileConfig", () => {
    const p = ProfileConfigSchema.parse({
      mode: "paper",
      opend: { host: "127.0.0.1", port: 11111, use_tls: false },
      opend_trd_env: "simulate",
      risk: {
        max_position_qty: 100,
        max_order_qty: 25,
        max_orders_per_minute: 10,
        max_daily_loss_usd: 50,
        symbol_allowlist: [],
        allowed_markets: [],
        price_band_pct: 0.1,
      },
      time_controls: {
        enabled: false,
        sessions_utc: [],
        blackout_utc: [],
        cooldown_sec: 0,
      },
      live_trading_unlocked: false,
      kill_switch_hotkey: "CmdOrCtrl+Alt+K",
      ai_router: {
        primary: "openai",
        fallbacks: ["deepseek"],
      },
      ai_providers: {
        openai: {
          id: "openai",
          kind: "openai",
          enabled: false,
          base_url: "https://api.openai.com/v1",
          model: "gpt-4o-mini",
          api_key_secret: "ai.openai_api_key",
          timeout_ms: 6000,
          max_tokens: 180,
          temperature: 0,
        },
      },
    });
    expect(p.mode).toBe("paper");
    expect(p.opend.port).toBe(11111);
  });

  it("parses AppConfig record profiles (z.record v4)", () => {
    const cfg = AppConfigSchema.parse({
      active_profile: "paper",
      profiles: {
        paper: {
          mode: "paper",
          opend: { host: "127.0.0.1", port: 11111, use_tls: false },
          opend_trd_env: "simulate",
          risk: {
            max_position_qty: 100,
            max_order_qty: 25,
            max_orders_per_minute: 10,
            max_daily_loss_usd: 50,
            symbol_allowlist: [],
            allowed_markets: [],
            price_band_pct: 0.1,
          },
          time_controls: {
            enabled: false,
            sessions_utc: [],
            blackout_utc: [],
            cooldown_sec: 0,
          },
          live_trading_unlocked: false,
          kill_switch_hotkey: "CmdOrCtrl+Alt+K",
          ai_router: {
            primary: "openai",
            fallbacks: ["deepseek"],
          },
          ai_providers: {
            openai: {
              id: "openai",
              kind: "openai",
              enabled: false,
              base_url: "https://api.openai.com/v1",
              model: "gpt-4o-mini",
              api_key_secret: "ai.openai_api_key",
              timeout_ms: 6000,
              max_tokens: 180,
              temperature: 0,
            },
          },
        },
      },
    });
    expect(Object.keys(cfg.profiles)).toEqual(["paper"]);
  });

  it("parses StrategyCatalog record", () => {
    const cat = StrategyCatalogSchema.parse({
      ma_crossover: {
        id: "ma_crossover",
        name: "MA Crossover",
        description: "demo",
        params_schema: { type: "object" },
      },
    });
    expect(cat.ma_crossover.id).toBe("ma_crossover");
  });
});
