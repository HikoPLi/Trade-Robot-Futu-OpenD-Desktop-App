import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Candle, Quote } from "@trade-robot/shared";
import { Button, Card, Field, Input, Select } from "../components/ui";
import { CandleChart } from "../components/CandleChart";
import { useEngine } from "../lib/engineContext";

function fmtTs(ts: string) {
  const d = new Date(ts);
  if (Number.isNaN(d.getTime())) return ts;
  return d.toLocaleString();
}

export function MarketPage() {
  const { t } = useTranslation();
  const eng = useEngine();
  const s = eng.snapshot;
  const [newSymbol, setNewSymbol] = useState("");
  const [selectedSymbol, setSelectedSymbol] = useState<string>("");
  const [intervalSec, setIntervalSec] = useState(60);
  const [candles, setCandles] = useState<Candle[]>([]);
  const [candlesLoading, setCandlesLoading] = useState(false);

  const watchlist = s?.watchlist ?? [];
  const quotes = useMemo(() => {
    const qs = s?.quotes ?? [];
    const bySym = new Map<string, Quote>();
    for (const q of qs) bySym.set(q.symbol, q);
    return watchlist
      .map((sym) => bySym.get(sym))
      .filter(Boolean) as Quote[];
  }, [s, watchlist]);

  useEffect(() => {
    if (!selectedSymbol && watchlist.length) setSelectedSymbol(watchlist[0]);
  }, [selectedSymbol, watchlist]);

  async function refreshCandles(sym: string, interval: number) {
    setCandlesLoading(true);
    try {
      const data = (await eng.getCandles(sym, interval, 200)) as Candle[];
      setCandles(data);
    } catch (e) {
      alert(String(e));
    } finally {
      setCandlesLoading(false);
    }
  }

  useEffect(() => {
    if (!selectedSymbol) return;
    refreshCandles(selectedSymbol, intervalSec).catch(() => {});
  }, [selectedSymbol, intervalSec]);

  useEffect(() => {
    // OpenD KL supports 1m/5m in MVP. Avoid spamming errors in live mode.
    if (s?.active_profile.mode === "live" && intervalSec < 60) setIntervalSec(60);
  }, [s?.active_profile.mode, intervalSec]);

  const quoteForSelected = quotes.find((q) => q.symbol === selectedSymbol) ?? null;

  return (
    <div style={{ display: "grid", gap: 12 }}>
      <Card style={{ padding: 14 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <div style={{ fontWeight: 700 }}>{t("market.watchlist")}</div>
          <div style={{ display: "flex", gap: 8, width: 360, maxWidth: "100%" }}>
            <Input
              value={newSymbol}
              placeholder={t("market.add_symbol_placeholder")}
              onChange={(e) => setNewSymbol(e.currentTarget.value.toUpperCase())}
            />
            <Button
              variant="primary"
              onClick={() => {
                const sym = newSymbol.trim();
                if (!sym) return;
                const next = Array.from(new Set([...watchlist, sym]));
                eng.setWatchlist(next).catch((e) => alert(String(e)));
                setNewSymbol("");
              }}
            >
              {t("common.add")}
            </Button>
          </div>
        </div>

        <div style={{ marginTop: 12 }}>
          {watchlist.length ? (
            <table className="tr-table">
              <thead>
                <tr>
                  <th>{t("market.symbol")}</th>
                  <th>{t("market.bid")}</th>
                  <th>{t("market.ask")}</th>
                  <th>{t("market.last")}</th>
                  <th>{t("market.vol")}</th>
                  <th>{t("market.time")}</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {quotes.map((q) => (
                  <tr
                    key={q.symbol}
                    className="tr-row-hover"
                    style={{
                      cursor: "pointer",
                      background: q.symbol === selectedSymbol ? "rgba(103,164,255,0.10)" : undefined,
                    }}
                    onClick={() => setSelectedSymbol(q.symbol)}
                  >
                    <td className="tr-mono">{q.symbol}</td>
                    <td className="tr-mono">{q.bid.toFixed(2)}</td>
                    <td className="tr-mono">{q.ask.toFixed(2)}</td>
                    <td className="tr-mono">{q.last.toFixed(2)}</td>
                    <td className="tr-mono">{q.volume.toFixed(0)}</td>
                    <td>{fmtTs(q.ts)}</td>
                    <td style={{ textAlign: "right" }}>
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={(e) => {
                          e.stopPropagation();
                          const next = watchlist.filter((x) => x !== q.symbol);
                          eng.setWatchlist(next).catch((err) => alert(String(err)));
                          if (selectedSymbol === q.symbol) setSelectedSymbol(next[0] ?? "");
                        }}
                      >
                        {t("common.remove")}
                      </Button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <div style={{ color: "var(--muted)", fontSize: 12 }}>
              {t("market.empty")}
            </div>
          )}
        </div>
      </Card>

      <Card style={{ padding: 14 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <div style={{ display: "grid", gap: 4 }}>
            <div style={{ fontWeight: 700 }}>{t("market.candles")}</div>
            <div style={{ color: "var(--muted)", fontSize: 12 }}>
              {selectedSymbol ? (
                <>
                  {quoteForSelected
                    ? t("market.last_inline", { symbol: selectedSymbol, last: quoteForSelected.last.toFixed(2) })
                    : selectedSymbol}
                </>
              ) : (
                t("market.select_symbol")
              )}
            </div>
          </div>

          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <div style={{ width: 180 }}>
              <Field label={t("market.interval")}>
                <Select value={intervalSec} onChange={(e) => setIntervalSec(Number(e.currentTarget.value))}>
                  {s?.active_profile.mode === "live" ? null : <option value={5}>5s</option>}
                  <option value={60}>1m</option>
                  <option value={300}>5m</option>
                </Select>
              </Field>
            </div>
            <Button
              size="sm"
              variant="ghost"
              disabled={!selectedSymbol || candlesLoading}
              onClick={() => {
                if (!selectedSymbol) return;
                refreshCandles(selectedSymbol, intervalSec).catch(() => {});
              }}
            >
              {candlesLoading ? t("common.loading") : t("common.refresh")}
            </Button>
          </div>
        </div>

        <div style={{ marginTop: 12 }}>
          {selectedSymbol ? (
            candles.length ? (
              <CandleChart candles={candles} />
            ) : (
              <div style={{ color: "var(--muted)", fontSize: 12 }}>{t("market.no_candles")}</div>
            )
          ) : (
            <div style={{ color: "var(--muted)", fontSize: 12 }}>{t("market.select_symbol_above")}</div>
          )}
        </div>
      </Card>
    </div>
  );
}
