import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Badge, Card } from "../components/ui";

function collect(t: (k: string) => string, prefix: string, count: number): string[] {
  return Array.from({ length: count }, (_, idx) => t(`${prefix}.${idx + 1}`));
}

function NumberList({ items }: { items: string[] }) {
  return (
    <ol style={{ margin: 0, paddingLeft: 18, display: "grid", gap: 8, color: "var(--muted)", fontSize: 13, lineHeight: 1.5 }}>
      {items.map((item, idx) => (
        <li key={`${idx}-${item}`}>{item}</li>
      ))}
    </ol>
  );
}

function DotList({ items }: { items: string[] }) {
  return (
    <ul style={{ margin: 0, paddingLeft: 18, display: "grid", gap: 8, color: "var(--muted)", fontSize: 13, lineHeight: 1.5 }}>
      {items.map((item, idx) => (
        <li key={`${idx}-${item}`}>{item}</li>
      ))}
    </ul>
  );
}

function Checklist({ items }: { items: string[] }) {
  return (
    <div style={{ display: "grid", gap: 8 }}>
      {items.map((item, idx) => (
        <label
          key={`${idx}-${item}`}
          style={{
            display: "grid",
            gridTemplateColumns: "18px 1fr",
            alignItems: "start",
            gap: 8,
            color: "var(--muted)",
            fontSize: 13,
            lineHeight: 1.5,
          }}
        >
          <input type="checkbox" style={{ accentColor: "var(--accent2)" }} />
          <span>{item}</span>
        </label>
      ))}
    </div>
  );
}

export function GuidePage() {
  const { t } = useTranslation();
  const safety = useMemo(() => collect(t, "guide.safety.items", 5), [t]);
  const quickstart = useMemo(() => collect(t, "guide.quickstart.steps", 8), [t]);
  const lifecycle = useMemo(() => collect(t, "guide.lifecycle.items", 4), [t]);
  const liveChecklist = useMemo(() => collect(t, "guide.live_checklist.items", 8), [t]);
  const sopPre = useMemo(() => collect(t, "guide.sop.pre", 5), [t]);
  const sopDuring = useMemo(() => collect(t, "guide.sop.during", 4), [t]);
  const sopAfter = useMemo(() => collect(t, "guide.sop.after", 4), [t]);
  const incident = useMemo(() => collect(t, "guide.incident.items", 5), [t]);
  const training = useMemo(() => collect(t, "guide.training.flow", 5), [t]);
  const scoring = useMemo(() => collect(t, "guide.training.scoring", 4), [t]);

  return (
    <div style={{ display: "grid", gap: 12 }}>
      <Card style={{ padding: 16 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 10 }}>
          <div style={{ fontWeight: 700, fontSize: 18 }}>{t("guide.title")}</div>
          <Badge tone="info">{t("guide.badge")}</Badge>
        </div>
        <div style={{ marginTop: 8, color: "var(--muted)", lineHeight: 1.5 }}>{t("guide.subtitle")}</div>
        <div style={{ marginTop: 8, color: "var(--faint)", fontSize: 12 }}>{t("guide.updated_at")}</div>
      </Card>

      <div className="tr-grid">
        <Card className="tr-guide-half" style={{ padding: 14 }}>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8 }}>
            <div style={{ fontWeight: 700 }}>{t("guide.quickstart.title")}</div>
            <Badge tone="good">{t("guide.quickstart.badge")}</Badge>
          </div>
          <div style={{ marginTop: 10 }}>
            <NumberList items={quickstart} />
          </div>
        </Card>

        <Card className="tr-guide-half" style={{ padding: 14 }}>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8 }}>
            <div style={{ fontWeight: 700 }}>{t("guide.safety.title")}</div>
            <Badge tone="warn">{t("guide.safety.badge")}</Badge>
          </div>
          <div style={{ marginTop: 10 }}>
            <DotList items={safety} />
          </div>
        </Card>
      </div>

      <div className="tr-grid">
        <Card className="tr-guide-half" style={{ padding: 14 }}>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8 }}>
            <div style={{ fontWeight: 700 }}>{t("guide.live_checklist.title")}</div>
            <Badge tone="bad">{t("guide.live_checklist.badge")}</Badge>
          </div>
          <div style={{ marginTop: 8, color: "var(--muted)", fontSize: 12 }}>{t("guide.live_checklist.note")}</div>
          <div style={{ marginTop: 10 }}>
            <Checklist items={liveChecklist} />
          </div>
        </Card>

        <Card className="tr-guide-half" style={{ padding: 14 }}>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8 }}>
            <div style={{ fontWeight: 700 }}>{t("guide.lifecycle.title")}</div>
            <Badge tone="neutral">{t("guide.lifecycle.badge")}</Badge>
          </div>
          <div style={{ marginTop: 10 }}>
            <NumberList items={lifecycle} />
          </div>
        </Card>
      </div>

      <Card style={{ padding: 14 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8 }}>
          <div style={{ fontWeight: 700 }}>{t("guide.sop.title")}</div>
          <Badge tone="good">{t("guide.sop.badge")}</Badge>
        </div>
        <div className="tr-grid" style={{ marginTop: 10 }}>
          <div className="tr-guide-third">
            <div style={{ fontWeight: 600, marginBottom: 8 }}>{t("guide.sop.pre_title")}</div>
            <DotList items={sopPre} />
          </div>
          <div className="tr-guide-third">
            <div style={{ fontWeight: 600, marginBottom: 8 }}>{t("guide.sop.during_title")}</div>
            <DotList items={sopDuring} />
          </div>
          <div className="tr-guide-third">
            <div style={{ fontWeight: 600, marginBottom: 8 }}>{t("guide.sop.after_title")}</div>
            <DotList items={sopAfter} />
          </div>
        </div>
      </Card>

      <div className="tr-grid">
        <Card className="tr-guide-half" style={{ padding: 14 }}>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8 }}>
            <div style={{ fontWeight: 700 }}>{t("guide.incident.title")}</div>
            <Badge tone="bad">{t("guide.incident.badge")}</Badge>
          </div>
          <div style={{ marginTop: 10 }}>
            <NumberList items={incident} />
          </div>
        </Card>

        <Card className="tr-guide-half" style={{ padding: 14 }}>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8 }}>
            <div style={{ fontWeight: 700 }}>{t("guide.training.title")}</div>
            <Badge tone="info">{t("guide.training.badge")}</Badge>
          </div>
          <div style={{ marginTop: 10, fontWeight: 600, fontSize: 13 }}>{t("guide.training.flow_title")}</div>
          <div style={{ marginTop: 8 }}>
            <NumberList items={training} />
          </div>
          <div style={{ marginTop: 12, fontWeight: 600, fontSize: 13 }}>{t("guide.training.scoring_title")}</div>
          <div style={{ marginTop: 8 }}>
            <DotList items={scoring} />
          </div>
        </Card>
      </div>
    </div>
  );
}
