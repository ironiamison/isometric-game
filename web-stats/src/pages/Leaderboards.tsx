import { useMemo, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { api, type LeaderboardEntry, type LeaderboardSort } from '../api'

type NumericField = Exclude<keyof LeaderboardEntry, 'name'>

type Metric = {
  label: string
  sort: LeaderboardSort
  field: NumericField
  hint: string
}

const METRIC_GROUPS: { title: string; metrics: Metric[] }[] = [
  {
    title: 'Showcase',
    metrics: [
      { label: 'Total Level', sort: 'total_level', field: 'total_level', hint: 'Overall progression' },
      { label: 'Combat Power', sort: 'combat_level', field: 'combat_level', hint: 'Combat + hitpoints strength' },
      { label: 'Played Time', sort: 'played_time', field: 'played_time', hint: 'Most active adventurers' },
      { label: 'Monster Kills', sort: 'monster_kills', field: 'monster_kills', hint: 'Total monsters defeated' },
    ],
  },
  {
    title: 'Combat',
    metrics: [
      { label: 'Combat Skill', sort: 'combat_skill_level', field: 'combat_skill_level', hint: 'Direct combat skill level' },
      { label: 'Hitpoints', sort: 'hitpoints_level', field: 'hitpoints_level', hint: 'Toughness and survivability' },
      { label: 'Prayer', sort: 'prayer_level', field: 'prayer_level', hint: 'Prayer mastery' },
      { label: 'Magic', sort: 'magic_level', field: 'magic_level', hint: 'Arcane power' },
    ],
  },
  {
    title: 'Gathering',
    metrics: [
      { label: 'Fishing', sort: 'fishing_level', field: 'fishing_level', hint: 'Waterside progress' },
      { label: 'Farming', sort: 'farming_level', field: 'farming_level', hint: 'Harvest expertise' },
      { label: 'Woodcutting', sort: 'woodcutting_level', field: 'woodcutting_level', hint: 'Forestry strength' },
      { label: 'Mining', sort: 'mining_level', field: 'mining_level', hint: 'Ore extraction skill' },
    ],
  },
  {
    title: 'Crafting',
    metrics: [
      { label: 'Smithing', sort: 'smithing_level', field: 'smithing_level', hint: 'Forge progression' },
      { label: 'Alchemy', sort: 'alchemy_level', field: 'alchemy_level', hint: 'Potion and transmutation skill' },
    ],
  },
]

const METRICS = METRIC_GROUPS.flatMap(group => group.metrics)

function formatPlayedTime(seconds: number) {
  const days = Math.floor(seconds / 86_400)
  const hours = Math.floor((seconds % 86_400) / 3_600)
  if (days > 0) return `${days}d ${hours}h`
  const minutes = Math.floor((seconds % 3_600) / 60)
  return `${hours}h ${minutes}m`
}

function metricValue(metric: Metric, entry: LeaderboardEntry) {
  if (metric.sort === 'played_time') {
    return formatPlayedTime(entry.played_time)
  }
  const raw = entry[metric.field]
  return Number(raw).toLocaleString()
}

function rankStyle(rank: number) {
  if (rank === 1) return 'border-[#d9b25f] bg-[#d9b25f]/10'
  if (rank === 2) return 'border-[#9ca3af] bg-[#9ca3af]/10'
  if (rank === 3) return 'border-[#ad7b46] bg-[#ad7b46]/10'
  return 'border-[var(--panel-border)] bg-[var(--panel-soft)]'
}

export function Leaderboards() {
  const [activeSort, setActiveSort] = useState<LeaderboardSort>('total_level')
  const [search, setSearch] = useState('')
  const metric = METRICS.find(m => m.sort === activeSort) ?? METRICS[0]

  const { data, isLoading } = useQuery({
    queryKey: ['leaderboard', metric.sort],
    queryFn: () => api.leaderboard(metric.sort, 200),
  })

  const ranked = useMemo(
    () => (data ?? []).map((entry, index) => ({ rank: index + 1, entry })),
    [data],
  )

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase()
    if (!q) return ranked
    return ranked.filter(item => item.entry.name.toLowerCase().includes(q))
  }, [ranked, search])

  const champions = ranked.slice(0, 3)

  return (
    <div className="space-y-6">
      <section className="relative overflow-hidden rounded-2xl border border-[var(--panel-border)] bg-[radial-gradient(circle_at_20%_15%,rgba(217,178,95,0.25),transparent_45%),radial-gradient(circle_at_80%_0%,rgba(75,136,207,0.18),transparent_45%),var(--panel)] px-6 py-7 md:px-8">
        <p className="text-xs uppercase tracking-[0.22em] text-[var(--muted)]">Hall Of Legends</p>
        <h1 className="mt-2 text-3xl font-semibold text-[var(--text)] md:text-4xl">Player Leaderboards</h1>
        <p className="mt-2 max-w-2xl text-sm text-[var(--text-soft)]">
          Browse every major ranking and spot the players setting the pace.
        </p>
      </section>

      <section className="grid gap-4 md:grid-cols-3">
        {isLoading
          ? Array.from({ length: 3 }).map((_, i) => (
              <div key={i} className="h-32 animate-pulse rounded-xl border border-[var(--panel-border)] bg-[var(--panel-soft)]" />
            ))
          : champions.map(({ rank, entry }) => (
              <div
                key={entry.name}
                className={`rounded-xl border p-4 transition-colors ${rankStyle(rank)}`}
              >
                <p className="text-xs uppercase tracking-[0.18em] text-[var(--muted)]">Rank {rank}</p>
                <p className="mt-2 text-xl font-semibold text-[var(--text)]">{entry.name}</p>
                <p className="mt-1 text-sm text-[var(--text-soft)]">{metricValue(metric, entry)} {metric.label}</p>
              </div>
            ))}
      </section>

      <section className="space-y-4 rounded-2xl border border-[var(--panel-border)] bg-[var(--panel)] p-4 md:p-5">
        {METRIC_GROUPS.map(group => (
          <div key={group.title}>
            <p className="mb-2 text-[11px] uppercase tracking-[0.2em] text-[var(--muted)]">{group.title}</p>
            <div className="flex flex-wrap gap-2">
              {group.metrics.map(item => (
                <button
                  key={item.sort}
                  onClick={() => setActiveSort(item.sort)}
                  className={`rounded-full border px-3 py-1.5 text-sm transition-colors ${
                    item.sort === metric.sort
                      ? 'border-[var(--gold)] bg-[var(--gold)]/20 text-[var(--text)]'
                      : 'border-[var(--panel-border)] bg-[var(--panel-soft)] text-[var(--text-soft)] hover:text-[var(--text)]'
                  }`}
                >
                  {item.label}
                </button>
              ))}
            </div>
          </div>
        ))}

        <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
          <p className="text-sm text-[var(--text-soft)]">
            Active board: <span className="text-[var(--text)]">{metric.label}</span> • {metric.hint}
          </p>
          <input
            type="text"
            placeholder="Search player..."
            value={search}
            onChange={e => setSearch(e.target.value)}
            className="w-full rounded-lg border border-[var(--panel-border)] bg-[var(--bg)] px-3 py-2 text-sm text-[var(--text)] outline-none transition-colors focus:border-[var(--gold)] md:max-w-xs"
          />
        </div>

        <div className="overflow-x-auto rounded-xl border border-[var(--panel-border)]">
          <table className="w-full min-w-[720px]">
            <thead>
              <tr className="bg-[var(--panel-soft)]">
                <th className="px-4 py-3 text-left text-[11px] uppercase tracking-[0.14em] text-[var(--muted)]">Rank</th>
                <th className="px-4 py-3 text-left text-[11px] uppercase tracking-[0.14em] text-[var(--muted)]">Player</th>
                <th className="px-4 py-3 text-left text-[11px] uppercase tracking-[0.14em] text-[var(--muted)]">{metric.label}</th>
                <th className="px-4 py-3 text-left text-[11px] uppercase tracking-[0.14em] text-[var(--muted)]">Total Level</th>
                <th className="px-4 py-3 text-left text-[11px] uppercase tracking-[0.14em] text-[var(--muted)]">Monster Kills</th>
                <th className="px-4 py-3 text-left text-[11px] uppercase tracking-[0.14em] text-[var(--muted)]">Played</th>
              </tr>
            </thead>
            <tbody>
              {isLoading &&
                Array.from({ length: 10 }).map((_, i) => (
                  <tr key={i} className="border-t border-[var(--panel-border)]">
                    <td className="px-4 py-3"><div className="h-3.5 w-8 animate-pulse rounded bg-[var(--panel-border)]" /></td>
                    <td className="px-4 py-3"><div className="h-3.5 w-24 animate-pulse rounded bg-[var(--panel-border)]" /></td>
                    <td className="px-4 py-3"><div className="h-3.5 w-16 animate-pulse rounded bg-[var(--panel-border)]" /></td>
                    <td className="px-4 py-3"><div className="h-3.5 w-14 animate-pulse rounded bg-[var(--panel-border)]" /></td>
                    <td className="px-4 py-3"><div className="h-3.5 w-14 animate-pulse rounded bg-[var(--panel-border)]" /></td>
                    <td className="px-4 py-3"><div className="h-3.5 w-16 animate-pulse rounded bg-[var(--panel-border)]" /></td>
                  </tr>
                ))}
              {!isLoading && filtered.length === 0 && (
                <tr>
                  <td colSpan={6} className="px-4 py-10 text-center text-[var(--text-soft)]">
                    No players matched that search.
                  </td>
                </tr>
              )}
              {!isLoading &&
                filtered.map(({ rank, entry }) => (
                  <tr key={`${entry.name}-${metric.sort}`} className="border-t border-[var(--panel-border)] hover:bg-[var(--panel-soft)]/70">
                    <td className="px-4 py-3 font-mono text-sm text-[var(--text-soft)]">{rank}</td>
                    <td className="px-4 py-3 font-medium text-[var(--text)]">{entry.name}</td>
                    <td className="px-4 py-3 font-mono text-[var(--text)]">{metricValue(metric, entry)}</td>
                    <td className="px-4 py-3 font-mono text-[var(--text-soft)]">{entry.total_level.toLocaleString()}</td>
                    <td className="px-4 py-3 font-mono text-[var(--text-soft)]">{entry.monster_kills.toLocaleString()}</td>
                    <td className="px-4 py-3 text-[var(--text-soft)]">{formatPlayedTime(entry.played_time)}</td>
                  </tr>
                ))}
            </tbody>
          </table>
        </div>
      </section>
    </div>
  )
}
