import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type {
  EngineSnapshot,
  ModelCatalog,
  ModelEvalParams,
  ModelEvalRunResult,
  ModelRegisterRequest,
  OpenDTradeEnv,
  OrderRequest,
  RegisteredModel,
  RiskLimits,
  StartStrategyRequest,
  StrategyDefinition,
  StrategyUpsertRequest,
  TimeControls,
} from "@trade-robot/shared";
import { CandleSchema, EnableLiveTradingPhrase, type BacktestParams, type OpenDConfig } from "@trade-robot/shared";
import {
  engineCancelOrder,
  engineEngageKillSwitch,
  engineEnableLiveTradingUnlock,
  engineClearSecret,
  engineExportAuditJsonl,
  engineGenerateSampleCandlesCsv,
  engineGetCandles,
  engineListAuditEvents,
  engineListModels,
  engineListStrategyDefs,
  engineModelCatalog,
  engineRegisterModel,
  engineDeleteModel,
  engineEvaluateModel,
  engineUpsertStrategyDef,
  engineDeleteStrategyDef,
  engineStartStrategyDef,
  enginePlaceOrder,
  engineRunBacktest,
  engineSecretStatus,
  engineSetActiveProfile,
  engineSetKillSwitchHotkey,
  engineSetSecret,
  engineSetWatchlist,
  engineSnapshot,
  engineStartStrategy,
  engineStopStrategy,
  engineStrategyCatalog,
  engineTestOpenDConnection,
  engineUpdateOpenDConfig,
  engineUpdateOpenDTradeEnv,
  engineUpdateTimeControls,
  engineUpdateRiskLimits,
  listenEngineEvents,
  type EngineEvent,
} from "./engineApi";

type EngineContextValue = {
  snapshot: EngineSnapshot | null;
  loading: boolean;
  error: string | null;
  events: EngineEvent[];

  refresh(): Promise<void>;
  setWatchlist(symbols: string[]): Promise<void>;
  getCandles(symbol: string, interval_sec: number, limit: number): Promise<Array<unknown>>;

  placeOrder(req: OrderRequest): Promise<void>;
  cancelOrder(order_id: string): Promise<void>;

  strategyCatalog(): Promise<unknown>;
  startStrategy(req: StartStrategyRequest): Promise<void>;
  stopStrategy(instance_id: string): Promise<void>;

  listStrategyDefs(): Promise<StrategyDefinition[]>;
  upsertStrategyDef(req: StrategyUpsertRequest): Promise<StrategyDefinition>;
  deleteStrategyDef(id: string): Promise<void>;
  startStrategyDef(id: string): Promise<string>;

  updateRiskLimits(limits: RiskLimits): Promise<void>;
  setActiveProfile(profile: string): Promise<void>;

  updateOpenDConfig(opend: OpenDConfig): Promise<void>;
  updateOpenDTradeEnv(env: OpenDTradeEnv): Promise<void>;
  updateTimeControls(time_controls: TimeControls): Promise<void>;
  testOpenDConnection(): Promise<void>;

  generateSampleCandlesCsv(symbol: string, interval_sec: number, limit: number): Promise<string>;
  runBacktest(params: BacktestParams): Promise<unknown>;

  listAuditEvents(args: {
    limit: number;
    offset: number;
    event_type?: string;
    trace_id?: string;
  }): Promise<unknown>;
  exportAuditJsonl(): Promise<string>;

  engageKillSwitch(reason: string): Promise<void>;
  setKillSwitchHotkey(hotkey: string): Promise<void>;

  enableLiveTradingUnlock(args: {
    confirmation_phrase: string;
    risk_non_default: boolean;
    kill_switch_configured: boolean;
  }): Promise<void>;

  secretStatus(key: string): Promise<boolean>;
  setSecret(key: string, value: string): Promise<void>;
  clearSecret(key: string): Promise<void>;

  defaults: {
    enable_live_trading_phrase: string;
  };

  models: {
    catalog(): Promise<ModelCatalog>;
    list(): Promise<RegisteredModel[]>;
    register(req: ModelRegisterRequest): Promise<RegisteredModel>;
    delete(id: string): Promise<void>;
    evaluate(params: ModelEvalParams): Promise<ModelEvalRunResult>;
  };
};

const EngineContext = createContext<EngineContextValue | null>(null);

export function EngineProvider({ children }: { children: ReactNode }) {
  const [snapshot, setSnapshot] = useState<EngineSnapshot | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [events, setEvents] = useState<EngineEvent[]>([]);

  const unlistenRef = useRef<null | (() => void)>(null);

  const refresh = useCallback(async () => {
    try {
      const s = await engineSnapshot();
      setSnapshot(s);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    let alive = true;
    (async () => {
      try {
        const s = await engineSnapshot();
        if (!alive) return;
        setSnapshot(s);
        setError(null);

        // Starter watchlist for out-of-box usability.
        if (s.watchlist.length === 0) {
          await engineSetWatchlist(["US.AAPL", "US.TSLA", "US.MSFT", "US.NVDA"]);
        }
      } catch (e) {
        if (!alive) return;
        setError(String(e));
      } finally {
        if (!alive) return;
        setLoading(false);
      }
    })();

    return () => {
      alive = false;
    };
  }, []);

  useEffect(() => {
    let disposed = false;
    (async () => {
      const unlisten = await listenEngineEvents((evt) => {
        if (disposed) return;
        setEvents((prev) => {
          const next = [...prev, evt].slice(-500);
          return next;
        });
      });
      unlistenRef.current = () => {
        unlisten();
      };
    })();

    return () => {
      disposed = true;
      unlistenRef.current?.();
      unlistenRef.current = null;
    };
  }, []);

  useEffect(() => {
    const t = window.setInterval(() => {
      refresh().catch(() => {});
    }, 1500);
    return () => window.clearInterval(t);
  }, [refresh]);

  const api = useMemo<EngineContextValue>(() => {
    return {
      snapshot,
      loading,
      error,
      events,

      refresh,
      setWatchlist: async (symbols) => {
        await engineSetWatchlist(symbols);
        await refresh();
      },
      getCandles: async (symbol, interval_sec, limit) => {
        const raw = (await engineGetCandles(symbol, interval_sec, limit)) as unknown[];
        return raw.map((x) => CandleSchema.parse(x));
      },

      placeOrder: async (req) => {
        await enginePlaceOrder(req);
        await refresh();
      },
      cancelOrder: async (order_id) => {
        await engineCancelOrder(order_id);
        await refresh();
      },

      strategyCatalog: async () => {
        return engineStrategyCatalog();
      },
      startStrategy: async (req) => {
        await engineStartStrategy(req);
        await refresh();
      },
      stopStrategy: async (instance_id) => {
        await engineStopStrategy(instance_id);
        await refresh();
      },

      listStrategyDefs: async () => {
        return engineListStrategyDefs();
      },
      upsertStrategyDef: async (req) => {
        const saved = await engineUpsertStrategyDef(req);
        await refresh();
        return saved;
      },
      deleteStrategyDef: async (id) => {
        await engineDeleteStrategyDef(id);
        await refresh();
      },
      startStrategyDef: async (id) => {
        const instance = await engineStartStrategyDef(id);
        await refresh();
        return instance;
      },

      updateRiskLimits: async (limits) => {
        await engineUpdateRiskLimits(limits);
        await refresh();
      },
      setActiveProfile: async (profile) => {
        await engineSetActiveProfile(profile);
        await refresh();
      },

      updateOpenDConfig: async (opend) => {
        await engineUpdateOpenDConfig(opend);
        await refresh();
      },
      updateOpenDTradeEnv: async (env) => {
        await engineUpdateOpenDTradeEnv(env);
        await refresh();
      },
      updateTimeControls: async (time_controls) => {
        await engineUpdateTimeControls(time_controls);
        await refresh();
      },
      testOpenDConnection: async () => {
        await engineTestOpenDConnection();
      },

      generateSampleCandlesCsv: async (symbol, interval_sec, limit) => {
        return engineGenerateSampleCandlesCsv(symbol, interval_sec, limit);
      },
      runBacktest: async (params) => {
        return engineRunBacktest(params);
      },

      listAuditEvents: async (args) => {
        return engineListAuditEvents(args);
      },
      exportAuditJsonl: async () => {
        return engineExportAuditJsonl();
      },

      engageKillSwitch: async (reason) => {
        await engineEngageKillSwitch(reason);
        await refresh();
      },
      setKillSwitchHotkey: async (hotkey) => {
        await engineSetKillSwitchHotkey(hotkey);
        await refresh();
      },

      enableLiveTradingUnlock: async (args) => {
        await engineEnableLiveTradingUnlock(args);
        await refresh();
      },

      secretStatus: async (key) => {
        return engineSecretStatus(key);
      },
      setSecret: async (key, value) => {
        await engineSetSecret(key, value);
      },
      clearSecret: async (key) => {
        await engineClearSecret(key);
      },

      defaults: {
        enable_live_trading_phrase: EnableLiveTradingPhrase,
      },

      models: {
        catalog: async () => {
          return engineModelCatalog();
        },
        list: async () => {
          return engineListModels();
        },
        register: async (req) => {
          const m = await engineRegisterModel(req);
          await refresh();
          return m;
        },
        delete: async (id) => {
          await engineDeleteModel(id);
          await refresh();
        },
        evaluate: async (params) => {
          return engineEvaluateModel(params);
        },
      },
    };
  }, [snapshot, loading, error, events, refresh]);

  return <EngineContext.Provider value={api}>{children}</EngineContext.Provider>;
}

export function useEngine() {
  const ctx = useContext(EngineContext);
  if (!ctx) throw new Error("useEngine must be used within EngineProvider");
  return ctx;
}
