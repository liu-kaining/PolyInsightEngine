"use client";

import { useParams } from "next/navigation";
import useSWR from "swr";
import { useEffect, useRef, useState } from "react";
import { fetchMarketHistory, fetchSmartMoneyTrades } from "@/lib/api";
import type { MarketHistoryPoint, SmartMoneyTrade } from "@/lib/api";
import ReactECharts from "echarts-for-react";

const API_BASE = process.env.NEXT_PUBLIC_API_BASE || "http://localhost:8080";

// Types for lightweight-charts
interface ChartInstance {
  remove: () => void;
  addLineSeries: (options: { color: string; lineWidth: number }) => LineSeries;
  timeScale: () => { fitContent: () => void };
}

interface LineSeries {
  setData: (data: Array<{ time: string; value: number }>) => void;
}

export default function MarketPage() {
  const params = useParams();
  const id = typeof params.id === "string" ? params.id : "";
  const { data: history = [] } = useSWR<MarketHistoryPoint[]>(
    id ? ["market-history", id, API_BASE] : null,
    () => fetchMarketHistory(id, "24h"),
    { refreshInterval: 60000 }
  );

  const { data: smartMoneyTrades = [] } = useSWR<SmartMoneyTrade[]>(
    id ? ["smart-money", id, API_BASE] : null,
    () => fetchSmartMoneyTrades(id),
    { refreshInterval: 60000 }
  );

  const chartRef = useRef<HTMLDivElement>(null);
  const chartInstanceRef = useRef<ChartInstance | null>(null);
  const [chartLoaded, setChartLoaded] = useState(false);

  // Fix: Properly cleanup and recreate chart when id or history changes
  useEffect(() => {
    // Only run on client side
    if (typeof window === "undefined" || !chartRef.current || !id || history.length === 0) {
      return;
    }

    // Cleanup existing chart first
    if (chartInstanceRef.current) {
      chartInstanceRef.current.remove();
      chartInstanceRef.current = null;
    }

    // Dynamic import for SSR safety
    let mounted = true;

    import("lightweight-charts").then((lwc) => {
      if (!mounted || !chartRef.current) return;

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

      if (mounted) {
        chartInstanceRef.current = chart;
        setChartLoaded(true);
      }
    }).catch((err) => {
      console.error("Failed to load lightweight-charts:", err);
    });

    return () => {
      mounted = false;
      if (chartInstanceRef.current) {
        chartInstanceRef.current.remove();
        chartInstanceRef.current = null;
      }
      setChartLoaded(false);
    };
  }, [id, history]);

  // ECharts options for smart money trades - Scatter Chart
  const smartMoneyChartOption = {
    backgroundColor: "transparent",
    title: {
      text: "🐋 Whale Flow (Smart Money)",
      left: "center",
      textStyle: {
        color: "#94a3b8",
        fontSize: 14,
        fontWeight: "normal" as const,
      },
    },
    tooltip: {
      trigger: "item" as const,
      backgroundColor: "rgba(15, 23, 42, 0.95)",
      borderColor: "#334155",
      textStyle: { color: "#e2e8f0" },
      formatter: (params: any) => {
        const trade = smartMoneyTrades[params.dataIndex];
        if (!trade) return "";
        const walletShort = trade.wallet_address.slice(0, 6) + "...";
        const side = trade.side === "YES" ? "🟢 YES" : "🔴 NO";
        return `
          <div style="padding: 4px;">
            <div style="font-weight: bold; margin-bottom: 4px;">Wallet: ${walletShort}</div>
            <div>Side: ${side}</div>
            <div>Size: $${trade.size.toLocaleString(undefined, { maximumFractionDigits: 0 })}</div>
            <div>Price: $${trade.price.toFixed(3)}</div>
          </div>
        `;
      },
    },
    legend: {
      data: ["YES", "NO"],
      bottom: 0,
      textStyle: { color: "#64748b" },
    },
    grid: {
      left: "3%",
      right: "7%",
      bottom: "15%",
      top: "15%",
      containLabel: true,
    },
    xAxis: {
      type: "time" as const,
      name: "Time",
      nameTextStyle: { color: "#64748b" },
      axisLine: { lineStyle: { color: "#334155" } },
      axisLabel: { color: "#64748b", fontSize: 10 },
      splitLine: { show: false },
    },
    yAxis: {
      type: "value" as const,
      name: "Size ($)",
      nameTextStyle: { color: "#64748b" },
      axisLine: { lineStyle: { color: "#334155" } },
      axisLabel: {
        color: "#64748b",
        formatter: (value: number) => {
          if (value >= 1000000) return `$${(value / 1000000).toFixed(1)}M`;
          if (value >= 1000) return `$${(value / 1000).toFixed(0)}K`;
          return `$${value}`;
        },
      },
      splitLine: { lineStyle: { color: "#1e293b" } },
    },
    series: [
      {
        name: "YES",
        type: "scatter" as const,
        data: smartMoneyTrades
          .filter((t) => t.side === "YES")
          .map((t) => ({
            value: [new Date(t.timestamp).getTime(), t.size],
            symbolSize: Math.max(10, Math.min(30, t.size / 10000)),
          })),
        itemStyle: { color: "#34d399" },
      },
      {
        name: "NO",
        type: "scatter" as const,
        data: smartMoneyTrades
          .filter((t) => t.side === "NO")
          .map((t) => ({
            value: [new Date(t.timestamp).getTime(), t.size],
            symbolSize: Math.max(10, Math.min(30, t.size / 10000)),
          })),
        itemStyle: { color: "#f43f5e" },
      },
    ],
  };

  const lastPrice = history.length > 0 ? history[history.length - 1] : null;

  return (
    <main className="min-h-screen bg-[#0a0a0a] p-6 text-slate-200">
      <h1 className="text-2xl font-semibold text-slate-100">Market</h1>
      <p className="font-mono text-slate-400 text-sm mt-1">Condition ID: {id}</p>

      <div className="mt-6 rounded-lg border border-slate-800/50 bg-slate-900/30 p-4">
        <h2 className="text-lg font-mono text-emerald-400 mb-3">Price History (24h)</h2>
        {history.length === 0 && (
          <p className="text-slate-500 py-8">No history data. Chart will appear when data exists.</p>
        )}
        {history.length > 0 && (
          <div ref={chartRef} className="w-full min-h-[360px]" style={{ height: 360 }} />
        )}
      </div>

      <div className="mt-6 grid grid-cols-1 lg:grid-cols-2 gap-4">
        <div className="rounded border border-slate-800/50 p-3 bg-slate-900/30">
          <p className="text-slate-500 text-xs">YES / NO (last)</p>
          <p className="font-mono text-slate-300 mt-1">
            {lastPrice
              ? `${lastPrice.yes_price.toFixed(3)} / ${lastPrice.no_price.toFixed(3)}`
              : "—"}
          </p>
        </div>
        <div className="rounded border border-slate-800/50 p-3 bg-slate-900/30">
          <p className="text-slate-500 text-xs">Liquidity</p>
          <p className="font-mono text-slate-300 mt-1">
            {lastPrice ? `$${lastPrice.liquidity.toLocaleString(undefined, { maximumFractionDigits: 0 })}` : "—"}
          </p>
        </div>
      </div>

      {/* Smart Money Trades Section */}
      <div className="mt-6 rounded-lg border border-slate-800/50 bg-slate-900/30 p-4">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-mono text-rose-400/90">Smart Money Tracker</h2>
          <span className="text-xs text-slate-500">
            {smartMoneyTrades.length} whale trades tracked
          </span>
        </div>

        {smartMoneyTrades.length === 0 ? (
          <p className="text-slate-500 py-8 text-center">
            No whale trades detected for this market yet.
          </p>
        ) : (
          <>
            {/* Bar Chart */}
            <div className="mb-6">
              <ReactECharts
                option={smartMoneyChartOption}
                style={{ height: 300, width: "100%" }}
                opts={{ renderer: "canvas" }}
              />
            </div>

            {/* Trade List */}
            <div className="space-y-2 max-h-[300px] overflow-y-auto">
              {smartMoneyTrades.slice(0, 10).map((trade) => (
                <div
                  key={trade.tx_hash}
                  className="flex items-center justify-between py-2 px-3 rounded bg-slate-800/30 hover:bg-slate-800/50"
                >
                  <div className="flex items-center gap-3">
                    <span
                      className={`text-xs font-mono px-2 py-1 rounded ${
                        trade.side === "YES"
                          ? "bg-emerald-500/20 text-emerald-400"
                          : "bg-rose-500/20 text-rose-400"
                      }`}
                    >
                      {trade.side}
                    </span>
                    <span className="font-mono text-sm text-slate-300">
                      {trade.wallet_address.slice(0, 10)}...
                    </span>
                  </div>
                  <div className="text-right">
                    <p className="font-mono text-emerald-400 font-semibold">
                      ${trade.size.toLocaleString(undefined, { maximumFractionDigits: 0 })}
                    </p>
                    <p className="text-xs text-slate-500">@ ${trade.price.toFixed(3)}</p>
                  </div>
                </div>
              ))}
            </div>
          </>
        )}
      </div>
    </main>
  );
}
