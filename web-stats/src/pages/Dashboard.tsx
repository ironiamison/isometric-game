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
      <section className="pixel-box relative overflow-hidden rounded-xl bg-[radial-gradient(circle_at_20%_10%,rgba(212,168,68,0.2),transparent_45%),radial-gradient(circle_at_80%_0%,rgba(90,114,71,0.18),transparent_50%),var(--panel)] px-6 py-8 md:px-8 md:py-10">
        <p className="text-xs uppercase tracking-[0.22em] text-[var(--muted)]" style={{ fontFamily: 'var(--font-display)' }}>World Pulse</p>
        <h1 className="mt-2 text-4xl font-bold text-[var(--text)] md:text-5xl">New Aeven Stats</h1>
        <p className="mt-3 max-w-2xl text-sm text-[var(--text-soft)]">
          See who's been grinding, who's climbing the ranks, and who rules the boards.
        </p>
        <div className="mt-5 flex flex-wrap gap-3">
          <Link
            to="/leaderboards"
            className="pixel-btn rounded-md bg-[var(--gold)] px-4 py-2 text-sm font-bold text-[#1a1210] hover:bg-[var(--gold-light)]"
          >
            Leaderboards
          </Link>
          <Link
            to="/players"
            className="pixel-btn rounded-md bg-[var(--panel-soft)] px-4 py-2 text-sm font-bold text-[var(--text-soft)] hover:text-[var(--text)]"
          >
            Online Players
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
          subtitle="Highest combined skill levels"
          metricLabel="Total Level"
          metric={(entry) => entry.total_level.toLocaleString()}
          data={topLevels}
          loading={loadingTopLevels}
        />
        <SpotlightBoard
          title="Top Monster Hunters"
          subtitle="Most monsters slain"
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
    <div className="pixel-box rounded-xl bg-[var(--panel)] p-5 transition-colors hover:border-[var(--gold)]/60">
      <div className="flex items-center gap-2">
        {indicator && <span className="inline-block h-2.5 w-2.5 rounded-full bg-[var(--moss-light)] shadow-[0_0_8px_var(--moss-light)]" />}
        <span className="text-sm text-[var(--text-soft)]">{label}</span>
      </div>
      <p className="mt-3 text-4xl font-bold text-[var(--gold)]">{value.toLocaleString()}</p>
    </div>
  )
}

function SkeletonCard() {
  return (
    <div className="rounded-xl border border-[var(--panel-border)] bg-[var(--panel)] p-5">
      <div className="h-4 w-28 animate-pulse rounded bg-[var(--panel-soft)]" />
      <div className="mt-4 h-10 w-20 animate-pulse rounded bg-[var(--panel-soft)]" />
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
    <section className="pixel-box rounded-xl bg-[var(--panel)] p-5">
      <h2 className="text-xl font-bold text-[var(--text)]">{title}</h2>
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
