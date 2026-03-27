"use client";

import Link from "next/link";
import useSWR from "swr";
import { fetchLeaderboard, fetchSignalsLatest } from "@/lib/api";
import type { LeaderboardEntry, SignalSummary } from "@/lib/api";
import { useStreamMarkets } from "@/lib/sse";

const fetcherLeaderboard = () => fetchLeaderboard();
const fetcherSignals = () => fetchSignalsLatest();

// Cyberpunk skeleton animation
function SkeletonLine({ width = "w-full", height = "h-4" }: { width?: string; height?: string }) {
  return (
    <div
      className={`${width} ${height} rounded bg-slate-800/50 animate-pulse`}
      style={{
        background: "linear-gradient(90deg, rgba(30,41,59,0.5) 25%, rgba(51,65,85,0.8) 50%, rgba(30,41,59,0.5) 75%)",
        backgroundSize: "200% 100%",
        animation: "shimmer 1.5s infinite",
      }}
    />
  );
}

// Keyframe animation via style tag
const shimmerKeyframes = `
@keyframes shimmer {
  0% { background-position: 200% 0; }
  100% { background-position: -200% 0; }
}
`;

// Loading skeleton for leaderboard
function LeaderboardSkeleton() {
  return (
    <div className="space-y-3 max-h-[400px] overflow-hidden">
      {[...Array(8)].map((_, i) => (
        <div key={i} className="flex justify-between items-center py-2 px-2">
          <div className="flex-1">
            <SkeletonLine width="w-3/4" height="h-4" />
          </div>
          <SkeletonLine width="w-16" height="h-4" />
        </div>
      ))}
    </div>
  );
}

// Loading skeleton for signals
function SignalsSkeleton() {
  return (
    <div className="space-y-3 max-h-[400px] overflow-hidden">
      {[...Array(5)].map((_, i) => (
        <div key={i} className="py-2 border-b border-slate-800/30">
          <SkeletonLine width="w-full" height="h-4" />
          <div className="mt-2 flex gap-2">
            <SkeletonLine width="w-20" height="h-3" />
            <SkeletonLine width="w-16" height="h-3" />
          </div>
        </div>
      ))}
    </div>
  );
}

export default function DashboardPage() {
  const { data: leaderboardRest, isLoading: leaderboardLoading } = useSWR<LeaderboardEntry[]>(
    "leaderboard",
    fetcherLeaderboard,
    { refreshInterval: 60000 }
  );
  const streamLeaderboard = useStreamMarkets();
  const leaderboard = streamLeaderboard.length > 0 ? streamLeaderboard : leaderboardRest ?? [];

  const { data: signals = [], isLoading: signalsLoading } = useSWR<SignalSummary[]>(
    "signals",
    fetcherSignals,
    { refreshInterval: 30000 }
  );

  const hasData = leaderboard.length > 0 || signals.length > 0;
  const isInitialLoad = leaderboardLoading || signalsLoading;

  return (
    <>
      {/* Inject shimmer animation */}
      <style jsx global>{shimmerKeyframes}</style>

      <main className="min-h-screen bg-[#0a0a0a] p-6 text-slate-200">
        <header className="border-b border-slate-800/50 pb-4 mb-6">
          <div className="flex items-center gap-3">
            <h1 className="text-2xl font-semibold text-slate-100">PolyInsight Dashboard</h1>
            {/* Live indicator */}
            <span className="flex items-center gap-1.5">
              <span className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse" />
              <span className="text-xs text-emerald-400 font-mono">LIVE</span>
            </span>
          </div>
          <p className="text-slate-400 text-sm mt-1">
            APY Leaderboard & AI signals
          </p>
        </header>

        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          {/* Leaderboard Section */}
          <section className="rounded-lg border border-slate-800/50 bg-slate-900/30 p-4 shadow-lg">
            <div className="flex items-center justify-between mb-3">
              <h2 className="text-lg font-mono text-emerald-400">做市 APY 排行榜</h2>
              {streamLeaderboard.length > 0 && (
                <span className="text-xs text-slate-500 font-mono">via SSE</span>
              )}
            </div>

            {isInitialLoad && !hasData ? (
              <LeaderboardSkeleton />
            ) : leaderboard.length === 0 ? (
              <div className="flex flex-col items-center justify-center py-12 text-slate-500">
                <div className="text-4xl mb-3 opacity-30">📊</div>
                <p className="font-mono text-sm">No markets available</p>
                <p className="text-xs text-slate-600 mt-1">Data will appear when available</p>
              </div>
            ) : (
              <div className="space-y-2 max-h-[400px] overflow-y-auto">
                {leaderboard.slice(0, 20).map((m, i) => (
                  <Link
                    key={m.condition_id}
                    href={`/market/${encodeURIComponent(m.condition_id)}`}
                    className="flex justify-between items-center py-2 border-b border-slate-800/30 last:border-0 hover:bg-slate-800/30 rounded px-2 -mx-2 transition-colors"
                  >
                    <div className="flex items-center gap-3 flex-1 min-w-0">
                      {/* Rank badge */}
                      <span className={`flex-shrink-0 w-6 h-6 rounded flex items-center justify-center text-xs font-mono ${
                        i === 0 ? "bg-amber-500/20 text-amber-400" :
                        i === 1 ? "bg-slate-400/20 text-slate-300" :
                        i === 2 ? "bg-orange-600/20 text-orange-400" :
                        "bg-slate-800/50 text-slate-500"
                      }`}>
                        {i + 1}
                      </span>
                      <span className="text-slate-300 truncate font-mono text-sm">
                        {m.question || m.condition_id.slice(0, 12)}
                      </span>
                    </div>
                    <span className="font-mono text-emerald-400 flex-shrink-0 ml-2">
                      {(m.apy * 100).toFixed(2)}%
                    </span>
                  </Link>
                ))}
              </div>
            )}
          </section>

          {/* AI Signals Section */}
          <section className="rounded-lg border border-slate-800/50 bg-slate-900/30 p-4 shadow-lg">
            <div className="flex items-center justify-between mb-3">
              <h2 className="text-lg font-mono text-rose-400/90">AI 异动研判</h2>
              <span className="text-xs text-slate-500 font-mono">
                {signals.length > 0 ? `${signals.length} signals` : "auto-gen"}
              </span>
            </div>

            {isInitialLoad && signals.length === 0 ? (
              <SignalsSkeleton />
            ) : signals.length === 0 ? (
              <div className="flex flex-col items-center justify-center py-12 text-slate-500">
                <div className="text-4xl mb-3 opacity-30">🧠</div>
                <p className="font-mono text-sm">No signals generated yet</p>
                <p className="text-xs text-slate-600 mt-1">AI analysis running every 2 min</p>
              </div>
            ) : (
              <div className="space-y-2 max-h-[400px] overflow-y-auto">
                {signals.map((s) => (
                  <div
                    key={s.signal_id}
                    className="py-2 border-b border-slate-800/30 last:border-0 hover:bg-slate-800/20 rounded px-2 -mx-2 transition-colors"
                  >
                    <p className="text-slate-300 text-sm leading-relaxed">{s.reasoning}</p>
                    <div className="flex items-center gap-3 mt-2 flex-wrap">
                      <span className="font-mono text-xs text-slate-500">
                        {s.condition_id.slice(0, 8)}...
                      </span>
                      <span className={`text-xs font-mono px-2 py-0.5 rounded ${
                        s.target_side.includes("YES") || s.target_side === "BUY_YES"
                          ? "bg-emerald-500/20 text-emerald-400"
                          : "bg-rose-500/20 text-rose-400"
                      }`}>
                        {s.target_side}
                      </span>
                      <span className="font-mono text-xs text-amber-400">
                        {(s.confidence * 100).toFixed(0)}% conf
                      </span>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </section>
        </div>

        {/* Footer status bar */}
        <footer className="mt-8 pt-4 border-t border-slate-800/30 flex items-center justify-between text-xs text-slate-600">
          <span className="font-mono">PolyInsight Engine v0.1.0</span>
          <div className="flex items-center gap-4">
            <span className="flex items-center gap-1">
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-500" />
              Leaderboard
            </span>
            <span className="flex items-center gap-1">
              <span className="w-1.5 h-1.5 rounded-full bg-rose-500" />
              AI Signals
            </span>
            <span className="flex items-center gap-1">
              <span className="w-1.5 h-1.5 rounded-full bg-amber-500" />
              Oracle Arb
            </span>
          </div>
        </footer>
      </main>
    </>
  );
}
