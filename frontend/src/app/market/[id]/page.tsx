"use client";

import { useParams } from "next/navigation";
import useSWR from "swr";
import { useEffect, useRef } from "react";
import { fetchMarketHistory } from "@/lib/api";
import type { MarketHistoryPoint } from "@/lib/api";

export default function MarketPage() {
  const params = useParams();
  const id = typeof params.id === "string" ? params.id : "";
  const { data: history = [] } = useSWR<MarketHistoryPoint[]>(
    id ? ["market-history", id] : null,
    () => fetchMarketHistory(id, "24h"),
    { refreshInterval: 60000 }
  );
  const chartRef = useRef<HTMLDivElement>(null);
  const chartInstance = useRef<ReturnType<typeof import("lightweight-charts").createChart> | null>(null);

  useEffect(() => {
    if (!chartRef.current || !id || history.length === 0) return;
    const lwc = require("lightweight-charts");
    if (chartInstance.current) return;
    const chart = lwc.createChart(chartRef.current, {
      layout: { background: { color: "#0a0a0a" }, textColor: "#94a3b8" },
      grid: { vertLines: { color: "#1e293b" }, horzLines: { color: "#1e293b" } },
      width: chartRef.current.clientWidth,
      height: 360,
    });
    const yesSeries = chart.addLineSeries({ color: "#34d399", lineWidth: 2 });
    const noSeries = chart.addLineSeries({ color: "#f43f5e", lineWidth: 2 });
    yesSeries.setData(
      history.map((p) => ({ time: p.timestamp.slice(0, 10) as any, value: p.yes_price }))
    );
    noSeries.setData(
      history.map((p) => ({ time: p.timestamp.slice(0, 10) as any, value: p.no_price }))
    );
    chart.timeScale().fitContent();
    chartInstance.current = chart;
    return () => {
      chart.remove();
      chartInstance.current = null;
    };
  }, [id, history]);

  return (
    <main className="min-h-screen bg-[#0a0a0a] p-6 text-slate-200">
      <h1 className="text-2xl font-semibold text-slate-100">Market</h1>
      <p className="font-mono text-slate-400 text-sm mt-1">Condition ID: {id}</p>

      <div className="mt-6 rounded-lg border border-slate-800/50 bg-slate-900/30 p-4">
        <h2 className="text-lg font-mono text-emerald-400 mb-3">Price History (24h)</h2>
        {history.length === 0 && (
          <p className="text-slate-500 py-8">No history data. Chart will appear when data exists.</p>
        )}
        <div ref={chartRef} className="w-full min-h-[360px]" style={{ height: 360 }} />
      </div>

      <div className="mt-6 grid grid-cols-2 gap-4">
        <div className="rounded border border-slate-800/50 p-3">
          <p className="text-slate-500 text-xs">YES / NO (last)</p>
          <p className="font-mono text-slate-300 mt-1">
            {history.length
              ? `${history[history.length - 1].yes_price.toFixed(3)} / ${history[history.length - 1].no_price.toFixed(3)}`
              : "—"}
          </p>
        </div>
        <div className="rounded border border-slate-800/50 p-3">
          <p className="text-slate-500 text-xs">Liquidity</p>
          <p className="font-mono text-slate-300 mt-1">
            {history.length ? history[history.length - 1].liquidity.toFixed(0) : "—"}
          </p>
        </div>
      </div>
    </main>
  );
}
