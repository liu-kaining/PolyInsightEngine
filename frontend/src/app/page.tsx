"use client";

import Link from "next/link";
import useSWR from "swr";
import { fetchLeaderboard, fetchSignalsLatest } from "@/lib/api";
import { useStreamMarkets } from "@/lib/sse";
import type { LeaderboardEntry, SignalSummary } from "@/lib/api";

const fetcherLeaderboard = () => fetchLeaderboard();
const fetcherSignals = () => fetchSignalsLatest();

export default function DashboardPage() {
  const { data: leaderboardRest } = useSWR<LeaderboardEntry[]>(
    "leaderboard",
    fetcherLeaderboard,
    { refreshInterval: 60000 }
  );
  const streamLeaderboard = useStreamMarkets();
  const leaderboard = streamLeaderboard.length > 0 ? streamLeaderboard : leaderboardRest ?? [];

  const { data: signals = [] } = useSWR<SignalSummary[]>("signals", fetcherSignals, {
    refreshInterval: 30000,
  });

  return (
    <main className="min-h-screen bg-[#0a0a0a] p-6 text-slate-200">
      <header className="border-b border-slate-800/50 pb-4 mb-6">
        <h1 className="text-2xl font-semibold text-slate-100">PolyInsight Dashboard</h1>
        <p className="text-slate-400 text-sm mt-1">
          APY Leaderboard & AI signals
        </p>
      </header>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <section className="rounded-lg border border-slate-800/50 bg-slate-900/30 p-4 shadow-lg">
          <h2 className="text-lg font-mono text-emerald-400 mb-3">做市 APY 排行榜</h2>
          <div className="space-y-2 max-h-[400px] overflow-y-auto">
            {leaderboard.length === 0 && (
              <p className="text-slate-500 text-sm">Loading or no data...</p>
            )}
            {leaderboard.slice(0, 20).map((m) => (
              <Link
                key={m.condition_id}
                href={`/market/${encodeURIComponent(m.condition_id)}`}
                className="flex justify-between items-center py-2 border-b border-slate-800/30 last:border-0 hover:bg-slate-800/30 rounded px-2 -mx-2"
              >
                <span className="text-slate-300 truncate max-w-[60%] font-mono text-sm">
                  {m.question || m.condition_id.slice(0, 12)}
                </span>
                <span className="font-mono text-emerald-400">{(m.apy * 100).toFixed(2)}%</span>
              </Link>
            ))}
          </div>
        </section>

        <section className="rounded-lg border border-slate-800/50 bg-slate-900/30 p-4 shadow-lg">
          <h2 className="text-lg font-mono text-rose-400/90 mb-3">AI 异动研判</h2>
          <div className="space-y-2 max-h-[400px] overflow-y-auto">
            {signals.length === 0 && (
              <p className="text-slate-500 text-sm">No signals in the last hour.</p>
            )}
            {signals.map((s) => (
              <div
                key={s.signal_id}
                className="py-2 border-b border-slate-800/30 last:border-0"
              >
                <p className="text-slate-300 text-sm">{s.reasoning}</p>
                <p className="font-mono text-xs text-slate-500 mt-1">
                  {s.condition_id.slice(0, 10)}... · {s.target_side} · {(s.confidence * 100).toFixed(0)}%
                </p>
              </div>
            ))}
          </div>
        </section>
      </div>
    </main>
  );
}
