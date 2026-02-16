import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { OrderRequest, OrderSide, OrderType, Quote } from "@trade-robot/shared";
import { Badge, Button, Card, Field, Input, Select } from "../components/ui";
import { useEngine } from "../lib/engineContext";

function fmtUsd(n: number) {
  const sign = n < 0 ? "-" : "";
  const v = Math.abs(n);
  return `${sign}$${v.toFixed(2)}`;
}

function quoteBySymbol(quotes: Quote[]) {
  const m = new Map<string, Quote>();
  for (const q of quotes) m.set(q.symbol, q);
  return m;
}

export function TradingPage() {
  const { t } = useTranslation();
  const eng = useEngine();
  const s = eng.snapshot;

  const watchlist = s?.watchlist ?? [];
  const quotes = s?.quotes ?? [];
  const quoteMap = useMemo(() => quoteBySymbol(quotes), [quotes]);

  const [symbol, setSymbol] = useState<string>(watchlist[0] ?? "US.AAPL");
  const [side, setSide] = useState<OrderSide>("buy");
  const [qty, setQty] = useState<number>(10);
  const [orderType, setOrderType] = useState<OrderType>("market");
  const [limitPrice, setLimitPrice] = useState<number | "">("");

  const posBySym = useMemo(() => {
    const m = new Map<string, number>();
    for (const p of s?.positions ?? []) m.set(p.symbol, p.qty);
    return m;
  }, [s?.positions]);

  async function submit() {
    const req: OrderRequest = {
      symbol: symbol.trim(),
      side,
      qty: Math.max(1, Math.floor(Number(qty))),
      order_type: orderType,
      limit_price: orderType === "limit" ? (limitPrice === "" ? null : Number(limitPrice)) : null,
      client_order_id: crypto.randomUUID(),
    };

    try {
      await eng.placeOrder(req);
    } catch (e) {
      alert(String(e));
    }
  }

  return (
    <div style={{ display: "grid", gap: 12 }}>
      <Card style={{ padding: 14 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <div style={{ fontWeight: 700 }}>
            {s?.active_profile.mode === "live" ? t("trading.order_ticket_live") : t("trading.order_ticket_paper")}
          </div>
          <Badge tone="neutral">{t("trading.risk_note")}</Badge>
        </div>

        <div style={{ marginTop: 12 }} className="tr-grid">
          <div style={{ gridColumn: "span 3" }}>
            <Field label={t("trading.symbol")} hint={t("trading.position_hint", { qty: posBySym.get(symbol) ?? 0 })}>
              <Select value={symbol} onChange={(e) => setSymbol(e.currentTarget.value)}>
                {watchlist.map((sym) => (
                  <option key={sym} value={sym}>
                    {sym}
                  </option>
                ))}
              </Select>
            </Field>
          </div>
          <div style={{ gridColumn: "span 2" }}>
            <Field label={t("trading.side")}>
              <Select value={side} onChange={(e) => setSide(e.currentTarget.value as any)}>
                <option value="buy">{t("enum.side.buy")}</option>
                <option value="sell">{t("enum.side.sell")}</option>
              </Select>
            </Field>
          </div>
          <div style={{ gridColumn: "span 2" }}>
            <Field label={t("trading.qty")}>
              <Input value={qty} type="number" min={1} step={1} onChange={(e) => setQty(Number(e.currentTarget.value))} />
            </Field>
          </div>
          <div style={{ gridColumn: "span 2" }}>
            <Field label={t("trading.type")}>
              <Select value={orderType} onChange={(e) => setOrderType(e.currentTarget.value as any)}>
                <option value="market">{t("enum.order_type.market")}</option>
                <option value="limit">{t("enum.order_type.limit")}</option>
              </Select>
            </Field>
          </div>
          <div style={{ gridColumn: "span 3" }}>
            <Field
              label={t("trading.limit_price")}
              hint={
                orderType === "limit"
                  ? t("trading.limit_hint_band")
                  : t("trading.last_hint", { last: (quoteMap.get(symbol)?.last ?? 0).toFixed(2) })
              }
            >
              <Input
                value={limitPrice}
                disabled={orderType !== "limit"}
                type="number"
                min={0}
                step={0.01}
                placeholder={orderType === "limit" ? t("trading.limit_placeholder") : ""}
                onChange={(e) => setLimitPrice(e.currentTarget.value === "" ? "" : Number(e.currentTarget.value))}
              />
            </Field>
          </div>

          <div style={{ gridColumn: "span 12", display: "flex", justifyContent: "flex-end", gap: 10 }}>
            <Button variant="primary" onClick={() => submit()}>
              {t("trading.place_order")}
            </Button>
          </div>
        </div>
      </Card>

      <div className="tr-split">
        <Card style={{ padding: 14 }}>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
            <div style={{ fontWeight: 700 }}>{t("trading.positions")}</div>
            <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
              <Badge tone={s && s.unrealized_pnl >= 0 ? "good" : "bad"}>
                {t("trading.unreal")}: {fmtUsd(s?.unrealized_pnl ?? 0)}
              </Badge>
              <Badge tone={s && s.realized_pnl >= 0 ? "good" : "bad"}>
                {t("trading.realized")}: {fmtUsd(s?.realized_pnl ?? 0)}
              </Badge>
            </div>
          </div>
          <div style={{ marginTop: 12 }}>
            {s?.positions.length ? (
              <table className="tr-table">
                <thead>
                  <tr>
                    <th>{t("trading.symbol")}</th>
                    <th>{t("trading.qty")}</th>
                    <th>{t("trading.avg_cost")}</th>
                    <th>{t("market.last")}</th>
                    <th>{t("trading.unreal")}</th>
                    <th>{t("trading.realized_pnl")}</th>
                  </tr>
                </thead>
                <tbody>
                  {s.positions.map((p) => {
                    const last = quoteMap.get(p.symbol)?.last ?? p.avg_cost;
                    const unreal = (last - p.avg_cost) * Math.max(0, p.qty);
                    return (
                      <tr key={p.symbol} className="tr-row-hover">
                        <td className="tr-mono">{p.symbol}</td>
                        <td className="tr-mono">{p.qty}</td>
                        <td className="tr-mono">{p.avg_cost.toFixed(2)}</td>
                        <td className="tr-mono">{last.toFixed(2)}</td>
                        <td className="tr-mono" style={{ color: unreal >= 0 ? "rgba(68,229,167,0.95)" : "rgba(255,77,109,0.92)" }}>
                          {fmtUsd(unreal)}
                        </td>
                        <td className="tr-mono" style={{ color: p.realized_pnl >= 0 ? "rgba(68,229,167,0.95)" : "rgba(255,77,109,0.92)" }}>
                          {fmtUsd(p.realized_pnl)}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            ) : (
              <div style={{ color: "var(--muted)", fontSize: 12 }}>{t("trading.no_positions")}</div>
            )}
          </div>
        </Card>

        <Card style={{ padding: 14 }}>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
            <div style={{ fontWeight: 700 }}>{t("trading.orders")}</div>
            <Badge tone="neutral">{s?.orders.length ?? 0}</Badge>
          </div>
          <div style={{ marginTop: 12 }}>
            {s?.orders.length ? (
              <table className="tr-table">
                <thead>
                  <tr>
                    <th>{t("trading.id")}</th>
                    <th>{t("trading.symbol")}</th>
                    <th>{t("trading.side")}</th>
                    <th>{t("trading.qty")}</th>
                    <th>{t("trading.type")}</th>
                    <th>{t("trading.limit_price")}</th>
                    <th>{t("trading.status")}</th>
                    <th />
                  </tr>
                </thead>
                <tbody>
                  {s.orders
                    .slice()
                    .reverse()
                    .slice(0, 50)
                    .map((o) => {
                      const canCancel = o.status === "submitted" || o.status === "pending_submit";
                      return (
                        <tr key={o.id} className="tr-row-hover">
                          <td className="tr-mono">{o.id.slice(0, 8)}</td>
                          <td className="tr-mono">{o.symbol}</td>
                          <td>{t(`enum.side.${o.side}`)}</td>
                          <td className="tr-mono">{o.qty}</td>
                          <td>{t(`enum.order_type.${o.order_type}`)}</td>
                          <td className="tr-mono">{o.limit_price ? Number(o.limit_price).toFixed(2) : "-"}</td>
                          <td>{t(`enum.order_status.${o.status}`)}</td>
                          <td style={{ textAlign: "right" }}>
                            {canCancel ? (
                              <Button
                                size="sm"
                                variant="ghost"
                                onClick={() => eng.cancelOrder(o.id).catch((e) => alert(String(e)))}
                              >
                                {t("common.cancel")}
                              </Button>
                            ) : null}
                          </td>
                        </tr>
                      );
                    })}
                </tbody>
              </table>
            ) : (
              <div style={{ color: "var(--muted)", fontSize: 12 }}>{t("trading.no_orders")}</div>
            )}
          </div>
        </Card>
      </div>
    </div>
  );
}
