import { Fragment, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { openPath } from "@tauri-apps/plugin-opener";
import { AuditEventRowSchema, type AuditEventRow } from "@trade-robot/shared";
import { Badge, Button, Card, Field, Input } from "../components/ui";
import { useEngine } from "../lib/engineContext";

function fmtTs(ts: string) {
  const d = new Date(ts);
  if (Number.isNaN(d.getTime())) return ts;
  return d.toLocaleString();
}

export function AuditPage() {
  const { t, i18n } = useTranslation();
  const eng = useEngine();

  const [eventType, setEventType] = useState<string>("");
  const [traceId, setTraceId] = useState<string>("");
  const [offset, setOffset] = useState<number>(0);
  const [rows, setRows] = useState<AuditEventRow[]>([]);
  const [loading, setLoading] = useState(false);
  const [expanded, setExpanded] = useState<Set<number>>(new Set());

  const limit = 100;

  async function load() {
    setLoading(true);
    try {
      const raw = await eng.listAuditEvents({
        limit,
        offset,
        event_type: eventType.trim() || undefined,
        trace_id: traceId.trim() || undefined,
      });
      const arr = raw as unknown[];
      setRows(arr.map((x) => AuditEventRowSchema.parse(x)));
    } catch (e) {
      alert(String(e));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    load().catch(() => {});
  }, [offset]);

  const title = useMemo(() => {
    const bits: string[] = [];
    if (eventType.trim()) bits.push(`${t("audit.badge_type")}=${eventType.trim()}`);
    if (traceId.trim()) bits.push(`${t("audit.badge_trace")}=${traceId.trim().slice(0, 8)}…`);
    return bits.length ? bits.join(" ") : t("audit.all_events");
  }, [eventType, traceId, i18n.language]);

  return (
    <div style={{ display: "grid", gap: 12 }}>
      <Card style={{ padding: 14 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
            <div style={{ fontWeight: 700 }}>{t("audit.title")}</div>
            <Badge tone="neutral">{title}</Badge>
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            <Button
              variant="ghost"
              onClick={async () => {
                try {
                  const path = await eng.exportAuditJsonl();
                  alert(t("audit.exported_alert", { path }));
                  await openPath(path);
                } catch (e) {
                  alert(String(e));
                }
              }}
            >
              {t("audit.export_jsonl")}
            </Button>
            <Button variant="ghost" disabled={loading} onClick={() => load()}>
              {loading ? t("common.loading") : t("common.refresh")}
            </Button>
          </div>
        </div>

        <div style={{ marginTop: 12 }} className="tr-grid">
          <div style={{ gridColumn: "span 4" }}>
            <Field label={t("audit.event_type")}>
              <Input value={eventType} placeholder={t("audit.event_type_placeholder")} onChange={(e) => setEventType(e.currentTarget.value)} />
            </Field>
          </div>
          <div style={{ gridColumn: "span 4" }}>
            <Field label={t("audit.trace_id")}>
              <Input value={traceId} placeholder={t("audit.trace_id_placeholder")} onChange={(e) => setTraceId(e.currentTarget.value)} />
            </Field>
          </div>
          <div style={{ gridColumn: "span 4", display: "flex", gap: 8, alignItems: "flex-end", justifyContent: "flex-end" }}>
            <Button
              variant="primary"
              onClick={() => {
                setOffset(0);
                load().catch(() => {});
              }}
            >
              {t("common.apply")}
            </Button>
          </div>
        </div>
      </Card>

      <Card style={{ padding: 0, overflow: "hidden" }}>
        <table className="tr-table">
          <thead>
            <tr>
              <th style={{ width: 70 }}>{t("audit.id")}</th>
              <th style={{ width: 200 }}>{t("audit.time")}</th>
              <th style={{ width: 180 }}>{t("audit.type")}</th>
              <th style={{ width: 220 }}>{t("audit.trace")}</th>
              <th>{t("audit.hash")}</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => {
              const isOpen = expanded.has(r.id);
              return (
                <Fragment key={r.id}>
                  <tr
                    className="tr-row-hover"
                    style={{ cursor: "pointer" }}
                    onClick={() => {
                      setExpanded((prev) => {
                        const next = new Set(prev);
                        if (next.has(r.id)) next.delete(r.id);
                        else next.add(r.id);
                        return next;
                      });
                    }}
                  >
                    <td className="tr-mono">{r.id}</td>
                    <td>{fmtTs(r.ts)}</td>
                    <td className="tr-mono">{r.event_type}</td>
                    <td className="tr-mono">{r.trace_id ? r.trace_id.slice(0, 12) : "-"}</td>
                    <td className="tr-mono">{r.hash.slice(0, 18)}…</td>
                  </tr>
                  {isOpen ? (
                    <tr>
                      <td colSpan={5} style={{ padding: 12 }}>
                        <pre
                          style={{
                            margin: 0,
                            padding: 12,
                            borderRadius: 12,
                            border: "1px solid rgba(255,255,255,0.10)",
                            background: "rgba(0,0,0,0.22)",
                            overflow: "auto",
                          }}
                        >
                          {JSON.stringify(r.payload_json, null, 2)}
                        </pre>
                      </td>
                    </tr>
                  ) : null}
                </Fragment>
              );
            })}
          </tbody>
        </table>

        <div style={{ padding: 12, display: "flex", alignItems: "center", justifyContent: "space-between" }}>
          <div style={{ color: "var(--muted)", fontSize: 12 }}>
            {t("audit.showing", { count: rows.length, limit, offset })}
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            <Button size="sm" variant="ghost" disabled={offset === 0} onClick={() => setOffset(Math.max(0, offset - limit))}>
              {t("common.prev")}
            </Button>
            <Button size="sm" variant="ghost" disabled={rows.length < limit} onClick={() => setOffset(offset + limit)}>
              {t("common.next")}
            </Button>
          </div>
        </div>
      </Card>
    </div>
  );
}
