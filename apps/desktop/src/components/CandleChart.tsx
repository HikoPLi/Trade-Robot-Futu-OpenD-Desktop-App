import { useEffect, useMemo, useRef } from "react";
import {
  CandlestickSeries,
  createChart,
  type IChartApi,
  type ISeriesApi,
  type UTCTimestamp,
} from "lightweight-charts";
import type { Candle } from "@trade-robot/shared";

function isoToUtcSeconds(ts: string): UTCTimestamp {
  const ms = new Date(ts).getTime();
  if (!Number.isFinite(ms) || Number.isNaN(ms) || ms <= 0) return 0 as UTCTimestamp;
  return Math.floor(ms / 1000) as UTCTimestamp;
}

export function CandleChart({
  candles,
  height = 360,
}: {
  candles: Candle[];
  height?: number;
}) {
  const ref = useRef<HTMLDivElement | null>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const seriesRef = useRef<ISeriesApi<"Candlestick"> | null>(null);

  const data = useMemo(() => {
    const map = new Map<number, { time: UTCTimestamp; open: number; high: number; low: number; close: number }>();
    for (const c of candles) {
      const time = isoToUtcSeconds(c.ts);
      if (!time) continue;
      if (![c.open, c.high, c.low, c.close].every((v) => Number.isFinite(v))) continue;
      map.set(Number(time), {
        time,
        open: c.open,
        high: c.high,
        low: c.low,
        close: c.close,
      });
    }
    const out = Array.from(map.values()).sort((a, b) => Number(a.time) - Number(b.time));
    return out;
  }, [candles]);

  useEffect(() => {
    if (!ref.current) return;

    const chart = createChart(ref.current, {
      height,
      width: Math.max(320, ref.current.clientWidth || 0),
      layout: {
        background: { color: "transparent" },
        textColor: "rgba(255,255,255,0.75)",
      },
      grid: {
        vertLines: { color: "rgba(255,255,255,0.06)" },
        horzLines: { color: "rgba(255,255,255,0.06)" },
      },
      rightPriceScale: {
        borderColor: "rgba(255,255,255,0.10)",
      },
      timeScale: {
        borderColor: "rgba(255,255,255,0.10)",
      },
      crosshair: {
        vertLine: { color: "rgba(103,164,255,0.35)" },
        horzLine: { color: "rgba(103,164,255,0.35)" },
      },
    });

    const series = chart.addSeries(CandlestickSeries, {
      upColor: "rgba(68,229,167,0.95)",
      downColor: "rgba(255,77,109,0.92)",
      borderUpColor: "rgba(68,229,167,0.65)",
      borderDownColor: "rgba(255,77,109,0.65)",
      wickUpColor: "rgba(68,229,167,0.65)",
      wickDownColor: "rgba(255,77,109,0.65)",
    });

    try {
      series.setData(data);
      if (data.length) chart.timeScale().fitContent();
    } catch (err) {
      console.error("failed to set candle data", err);
    }

    chartRef.current = chart;
    seriesRef.current = series;

    const ro = new ResizeObserver(() => {
      if (!ref.current || !chartRef.current) return;
      chartRef.current.resize(ref.current.clientWidth, height);
    });
    ro.observe(ref.current);

    return () => {
      ro.disconnect();
      chart.remove();
      chartRef.current = null;
      seriesRef.current = null;
    };
  }, [height]);

  useEffect(() => {
    const series = seriesRef.current;
    const chart = chartRef.current;
    if (!series || !chart) return;
    try {
      series.setData(data);
      if (data.length) chart.timeScale().fitContent();
    } catch (err) {
      console.error("failed to update candle data", err);
    }
  }, [data]);

  return <div ref={ref} style={{ width: "100%", height, minHeight: height }} />;
}
