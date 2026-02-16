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
  return Math.floor(new Date(ts).getTime() / 1000) as UTCTimestamp;
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
    return candles.map((c) => ({
      time: isoToUtcSeconds(c.ts),
      open: c.open,
      high: c.high,
      low: c.low,
      close: c.close,
    }));
  }, [candles]);

  useEffect(() => {
    if (!ref.current) return;

    const chart = createChart(ref.current, {
      height,
      width: ref.current.clientWidth,
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

    if (data.length) {
      series.setData(data);
      chart.timeScale().fitContent();
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
    if (!series) return;
    if (data.length) series.setData(data);
  }, [data]);

  return <div ref={ref} style={{ width: "100%" }} />;
}
