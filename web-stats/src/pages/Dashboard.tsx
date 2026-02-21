import { useQuery } from '@tanstack/react-query'
import { Link } from 'react-router-dom'
import { api, type LeaderboardEntry } from '../api'

export function Dashboard() {
  const { data: overview, isLoading: loadingOverview } = useQuery({
    queryKey: ['overview'],
    queryFn: api.overview,
    refetchInterval: 15000,
  })

  const { data: topLevels, isLoading: loadingTopLevels } = useQuery({
    queryKey: ['dashboard', 'top-levels'],
    queryFn: () => api.leaderboard('total_level', 5),
  })

  const { data: topHunters, isLoading: loadingTopHunters } = useQuery({
    queryKey: ['dashboard', 'top-hunters'],
    queryFn: () => api.leaderboard('monster_kills', 5),
  })

  return (
    <div className="space-y-6">
      <section className="relative overflow-hidden rounded-2xl border border-[var(--panel-border)] bg-[radial-gradient(circle_at_20%_10%,rgba(76,135,206,0.2),transparent_45%),radial-gradient(circle_at_80%_0%,rgba(217,178,95,0.3),transparent_50%),var(--panel)] px-6 py-8 md:px-8 md:py-10">
        <p className="text-xs uppercase tracking-[0.22em] text-[var(--muted)]">World Pulse</p>
        <h1 className="mt-2 text-4xl font-semibold text-[var(--text)] md:text-5xl">New Aeven Stats Hub</h1>
        <p className="mt-3 max-w-2xl text-sm text-[var(--text-soft)]">
          Track rising players, compare skill progress, and claim spots on the Hall of Legends.
        </p>
        <div className="mt-5 flex flex-wrap gap-3">
          <Link
            to="/leaderboards"
            className="rounded-full border border-[var(--gold)] bg-[var(--gold)]/20 px-4 py-2 text-sm font-medium text-[var(--text)] transition-colors hover:bg-[var(--gold)]/30"
          >
            View Leaderboards
          </Link>
          <Link
            to="/players"
            className="rounded-full border border-[var(--panel-border)] bg-[var(--panel-soft)] px-4 py-2 text-sm font-medium text-[var(--text-soft)] transition-colors hover:text-[var(--text)]"
          >
            Watch Online Players
          </Link>
        </div>
      </section>

      <div className="grid grid-cols-1 gap-4 md:grid-cols-3">
        {loadingOverview ? (
          <>
            <SkeletonCard />
            <SkeletonCard />
            <SkeletonCard />
          </>
        ) : (
          <>
            <StatCard label="Online Players" value={overview?.online_players ?? 0} indicator />
            <StatCard label="Total Characters" value={overview?.total_characters ?? 0} />
            <StatCard label="Total Accounts" value={overview?.total_accounts ?? 0} />
          </>
        )}
      </div>

      <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
        <SpotlightBoard
          title="Top Total Level"
          subtitle="Most progressed characters overall"
          metricLabel="Total Level"
          metric={(entry) => entry.total_level.toLocaleString()}
          data={topLevels}
          loading={loadingTopLevels}
        />
        <SpotlightBoard
          title="Top Monster Hunters"
          subtitle="Players with the most monster defeats"
          metricLabel="Kills"
          metric={(entry) => entry.monster_kills.toLocaleString()}
          data={topHunters}
          loading={loadingTopHunters}
        />
      </div>
    </div>
  )
}

function StatCard({ label, value, indicator }: { label: string; value: number; indicator?: boolean }) {
  return (
    <div className="rounded-xl border border-[var(--panel-border)] bg-[var(--panel)] p-5 transition-colors hover:border-[var(--gold)]/40">
      <div className="flex items-center gap-2">
        {indicator && <span className="inline-block h-2.5 w-2.5 rounded-full bg-[#4ade80] shadow-[0_0_8px_#4ade80]" />}
        <span className="text-sm text-[var(--text-soft)]">{label}</span>
      </div>
      <p className="mt-3 text-4xl font-semibold text-[var(--gold)]">{value.toLocaleString()}</p>
    </div>
  )
}

function SkeletonCard() {
  return (
    <div className="rounded-xl border border-[var(--panel-border)] bg-[var(--panel)] p-5">
      <div className="h-4 w-28 animate-pulse rounded bg-[var(--panel-border)]" />
      <div className="mt-4 h-10 w-20 animate-pulse rounded bg-[var(--panel-border)]" />
    </div>
  )
}

function SpotlightBoard({
  title,
  subtitle,
  metricLabel,
  metric,
  data,
  loading,
}: {
  title: string
  subtitle: string
  metricLabel: string
  metric: (entry: LeaderboardEntry) => string
  data: LeaderboardEntry[] | undefined
  loading: boolean
}) {
  return (
    <section className="rounded-xl border border-[var(--panel-border)] bg-[var(--panel)] p-5">
      <h2 className="text-xl font-semibold text-[var(--text)]">{title}</h2>
      <p className="mt-1 text-sm text-[var(--text-soft)]">{subtitle}</p>
      <div className="mt-4 space-y-2">
        {loading &&
          Array.from({ length: 5 }).map((_, i) => (
            <div key={i} className="h-10 animate-pulse rounded-lg bg-[var(--panel-soft)]" />
          ))}
        {!loading && (data ?? []).length === 0 && (
          <p className="rounded-lg border border-[var(--panel-border)] bg-[var(--panel-soft)] px-3 py-4 text-sm text-[var(--text-soft)]">
            No data yet.
          </p>
        )}
        {!loading &&
          (data ?? []).map((entry, index) => (
            <div key={entry.name} className="flex items-center justify-between rounded-lg border border-[var(--panel-border)] bg-[var(--panel-soft)] px-3 py-2.5">
              <div className="flex items-center gap-3">
                <span className="w-5 text-xs font-mono text-[var(--muted)]">{index + 1}</span>
                <Link
                  to={`/player/${encodeURIComponent(entry.name)}`}
                  className="font-medium text-[var(--text)] hover:text-[var(--gold)]"
                >
                  {entry.name}
                </Link>
              </div>
              <div className="text-right">
                <p className="font-mono text-sm text-[var(--text)]">{metric(entry)}</p>
                <p className="text-[11px] uppercase tracking-[0.12em] text-[var(--muted)]">{metricLabel}</p>
              </div>
            </div>
          ))}
      </div>
    </section>
  )
}
