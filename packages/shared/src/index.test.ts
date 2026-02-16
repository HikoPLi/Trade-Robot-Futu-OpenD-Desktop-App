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
