import { useEffect } from 'react'
import { useQuery } from '@tanstack/react-query'
import { Link } from 'react-router-dom'
import { api, type LeaderboardEntry } from '../api'
import { Users, Trophy, Gem, Skull, Crown, Crosshair, Signal, UserRound, UserCheck } from 'lucide-react'

export function Dashboard() {
  useEffect(() => { document.title = 'World Pulse — New Aeven World Statistics' }, [])
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
    <div className="space-y-5">
      {/* Title */}
      <h1 className="text-3xl font-bold text-[var(--text)] md:text-4xl">New Aeven Stats</h1>

      {/* Bento grid */}
      <div className="grid grid-cols-3 gap-2 md:grid-cols-6 md:gap-3">
        {/* Stat: Online */}
        <div className="pixel-box col-span-1 rounded-lg bg-[var(--panel)] px-3 py-2.5">
          <span className="flex items-center gap-1.5 text-xs text-[var(--text-soft)]">
            <Signal size={10} className="text-[var(--moss-light)]" />
            Online
          </span>
          {loadingOverview ? (
            <div className="mt-1.5 h-6 w-10 animate-pulse rounded bg-[var(--panel-soft)]" />
          ) : (
            <p className="mt-1.5 text-xl font-bold text-[var(--gold)]">{(overview?.online_players ?? 0).toLocaleString()}</p>
          )}
        </div>

        {/* Stat: Characters */}
        <div className="pixel-box col-span-1 rounded-lg bg-[var(--panel)] px-3 py-2.5">
          <span className="flex items-center gap-1.5 text-xs text-[var(--text-soft)]">
            <UserRound size={10} />
            Characters
          </span>
          {loadingOverview ? (
            <div className="mt-1.5 h-6 w-12 animate-pulse rounded bg-[var(--panel-soft)]" />
          ) : (
            <p className="mt-1.5 text-xl font-bold text-[var(--text)]">{(overview?.total_characters ?? 0).toLocaleString()}</p>
          )}
        </div>

        {/* Stat: Accounts */}
        <div className="pixel-box col-span-1 rounded-lg bg-[var(--panel)] px-3 py-2.5">
          <span className="flex items-center gap-1.5 text-xs text-[var(--text-soft)]">
            <UserCheck size={10} />
            Accounts
          </span>
          {loadingOverview ? (
            <div className="mt-1.5 h-6 w-12 animate-pulse rounded bg-[var(--panel-soft)]" />
          ) : (
            <p className="mt-1.5 text-xl font-bold text-[var(--text)]">{(overview?.total_accounts ?? 0).toLocaleString()}</p>
          )}
        </div>

        {/* Nav card: Live Players */}
        <Link
          to="/players"
          className="pixel-box group col-span-1 rounded-lg bg-[var(--panel)] px-3 py-2.5 transition-colors hover:border-[var(--gold)]/50"
        >
          <span className="flex items-center gap-1.5 text-xs text-[var(--text-soft)]">
            <Users size={10} />
            Live Players
          </span>
          <p className="mt-1.5 text-xs font-semibold text-[var(--gold)] opacity-60 transition-opacity group-hover:opacity-100">
            View &rarr;
          </p>
        </Link>

        {/* Nav card: Leaderboards */}
        <Link
          to="/leaderboards"
          className="pixel-box group col-span-1 rounded-lg bg-[var(--panel)] px-3 py-2.5 transition-colors hover:border-[var(--gold)]/50"
        >
          <span className="flex items-center gap-1.5 text-xs text-[var(--text-soft)]">
            <Trophy size={10} />
            Leaderboards
          </span>
          <p className="mt-1.5 text-xs font-semibold text-[var(--gold)] opacity-60 transition-opacity group-hover:opacity-100">
            View &rarr;
          </p>
        </Link>

        {/* Nav card: Items + Bestiary stacked */}
        <div className="col-span-1 flex flex-col gap-2 md:gap-3">
          <Link
            to="/items"
            className="pixel-box group flex-1 rounded-lg bg-[var(--panel)] px-3 py-2 transition-colors hover:border-[var(--gold)]/50"
          >
            <span className="flex items-center gap-1.5 text-xs text-[var(--text-soft)]">
              <Gem size={10} />
              Items
            </span>
          </Link>
          <Link
            to="/bestiary"
            className="pixel-box group flex-1 rounded-lg bg-[var(--panel)] px-3 py-2 transition-colors hover:border-[var(--gold)]/50"
          >
            <span className="flex items-center gap-1.5 text-xs text-[var(--text-soft)]">
              <Skull size={10} />
              Bestiary
            </span>
          </Link>
        </div>

        {/* Spotlight: Top Total Level — spans wider */}
        <div className="col-span-2 md:col-span-3">
          <SpotlightBoard
            title="Top Total Level"
            icon={<Crown size={14} className="text-[var(--gold)]" />}
            metricLabel="Total Lv"
            metric={(entry) => entry.total_level.toLocaleString()}
            data={topLevels}
            loading={loadingTopLevels}
          />
        </div>

        {/* Spotlight: Top Monster Hunters */}
        <div className="col-span-2 md:col-span-3">
          <SpotlightBoard
            title="Top Monster Hunters"
            icon={<Crosshair size={14} className="text-[var(--ember)]" />}
            metricLabel="Kills"
            metric={(entry) => entry.monster_kills.toLocaleString()}
            data={topHunters}
            loading={loadingTopHunters}
          />
        </div>
      </div>
    </div>
  )
}

function SpotlightBoard({
  title,
  icon,
  metricLabel,
  metric,
  data,
  loading,
}: {
  title: string
  icon: React.ReactNode
  metricLabel: string
  metric: (entry: LeaderboardEntry) => string
  data: LeaderboardEntry[] | undefined
  loading: boolean
}) {
  return (
    <section className="pixel-box h-full rounded-xl bg-[var(--panel)] p-4 md:p-5">
      <h2 className="flex items-center gap-2 text-sm font-bold text-[var(--text)]">
        {icon}
        {title}
      </h2>
      <div className="mt-3 space-y-1.5">
        {loading &&
          Array.from({ length: 5 }).map((_, i) => (
            <div key={i} className="h-8 animate-pulse rounded bg-[var(--panel-soft)]" />
          ))}
        {!loading && (data ?? []).length === 0 && (
          <p className="py-3 text-sm text-[var(--text-soft)]">No data yet.</p>
        )}
        {!loading &&
          (data ?? []).map((entry, index) => (
            <div key={entry.name} className="flex items-center justify-between rounded-lg bg-[var(--panel-soft)] px-3 py-2">
              <div className="flex items-center gap-2.5">
                <span className={`w-4 text-xs font-mono ${index === 0 ? 'text-[var(--gold)]' : 'text-[var(--muted)]'}`}>{index + 1}</span>
                <Link
                  to={`/player/${encodeURIComponent(entry.name)}`}
                  className="text-sm font-medium text-[var(--text)] hover:text-[var(--gold)]"
                >
                  {entry.name}
                </Link>
              </div>
              <div className="flex items-baseline gap-1.5">
                <span className="font-mono text-sm text-[var(--text)]">{metric(entry)}</span>
                <span className="text-[10px] text-[var(--muted)]">{metricLabel}</span>
              </div>
            </div>
          ))}
      </div>
    </section>
  )
}
