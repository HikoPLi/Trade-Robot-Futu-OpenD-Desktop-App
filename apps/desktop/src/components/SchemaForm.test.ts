import { describe, expect, it } from "vitest";
import { initFromSchema } from "./SchemaForm";

describe("SchemaForm helpers", () => {
  it("initializes defaults from a JSON-schema-like object", () => {
    const schema = {
      type: "object",
      properties: {
        symbol: { type: "string", default: "AAPL" },
        fast_period: { type: "integer", default: 10 },
        enabled: { type: "boolean", default: true },
        note: { type: "string" },
      },
      required: ["symbol", "fast_period"],
    };

    expect(initFromSchema(schema)).toEqual({
      symbol: "AAPL",
      fast_period: 10,
      enabled: true,
      note: "",
    });
  });
});

