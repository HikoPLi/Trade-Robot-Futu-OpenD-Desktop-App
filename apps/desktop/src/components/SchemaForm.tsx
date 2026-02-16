import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Field, Input, TextArea } from "./ui";

type JsonSchema = any;

function getDefaultValue(propSchema: any) {
  if (propSchema && typeof propSchema.default !== "undefined") return propSchema.default;
  switch (propSchema?.type) {
    case "string":
      return "";
    case "integer":
    case "number":
      return 0;
    case "boolean":
      return false;
    default:
      return null;
  }
}

export function initFromSchema(schema: JsonSchema): Record<string, any> {
  const out: Record<string, any> = {};
  const props = schema?.properties ?? {};
  for (const key of Object.keys(props)) {
    out[key] = getDefaultValue(props[key]);
  }
  return out;
}

export function SchemaForm({
  schema,
  value,
  onChange,
}: {
  schema: JsonSchema;
  value: Record<string, any>;
  onChange(next: Record<string, any>): void;
}) {
  const { t } = useTranslation();
  const props = schema?.properties ?? {};
  const required = new Set<string>(schema?.required ?? []);

  const fields = useMemo(() => Object.entries(props) as Array<[string, any]>, [schema]);

  return (
    <div className="tr-grid">
      {fields.map(([key, propSchema]) => {
        const type = propSchema?.type;
        const title = propSchema?.title ?? key;
        const hintParts: string[] = [];
        if (required.has(key)) hintParts.push(t("schema.required"));
        if (typeof propSchema?.minimum === "number") hintParts.push(t("schema.min", { n: propSchema.minimum }));
        if (typeof propSchema?.maximum === "number") hintParts.push(t("schema.max", { n: propSchema.maximum }));
        const hint = hintParts.join(" • ") || undefined;

        const val = value[key];
        const isScript = typeof val === "string" && key.toLowerCase().includes("script");

        const col = type === "string" && val?.length > 64 ? 12 : 4;
        const colSpan = isScript ? 12 : col;

        return (
          <div key={key} style={{ gridColumn: `span ${colSpan}` }}>
            <Field label={title} hint={hint}>
              {isScript ? (
                <TextArea
                  value={String(val ?? "")}
                  onChange={(e) => onChange({ ...value, [key]: e.currentTarget.value })}
                />
              ) : type === "integer" || type === "number" ? (
                <Input
                  type="number"
                  value={val ?? 0}
                  min={propSchema?.minimum}
                  max={propSchema?.maximum}
                  step={type === "integer" ? 1 : 0.0001}
                  onChange={(e) =>
                    onChange({
                      ...value,
                      [key]: type === "integer" ? Math.floor(Number(e.currentTarget.value)) : Number(e.currentTarget.value),
                    })
                  }
                />
              ) : (
                <Input
                  value={String(val ?? "")}
                  onChange={(e) => onChange({ ...value, [key]: e.currentTarget.value })}
                />
              )}
            </Field>
          </div>
        );
      })}
    </div>
  );
}
