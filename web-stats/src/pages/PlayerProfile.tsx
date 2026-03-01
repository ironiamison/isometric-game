import { useEffect, useMemo, useState } from 'react'
import { Link, useParams } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'
import { api, type LeaderboardEntry, type PlayerProfileRanks } from '../api'

type ProfileStat = {
  label: string
  value: (player: LeaderboardEntry) => string
  rank: (ranks: PlayerProfileRanks) => number
}

const PROFILE_STATS: ProfileStat[] = [
  { label: 'Total Level', value: (player) => player.total_level.toLocaleString(), rank: (ranks) => ranks.total_level },
  { label: 'Combat Level', value: (player) => player.combat_level.toLocaleString(), rank: (ranks) => ranks.combat_level },
  { label: 'Attack', value: (player) => player.attack_level.toLocaleString(), rank: (ranks) => ranks.attack_level },
  { label: 'Strength', value: (player) => player.strength_level.toLocaleString(), rank: (ranks) => ranks.strength_level },
  { label: 'Defence', value: (player) => player.defence_level.toLocaleString(), rank: (ranks) => ranks.defence_level },
  { label: 'Ranged', value: (player) => player.ranged_level.toLocaleString(), rank: (ranks) => ranks.ranged_level },
  { label: 'Hitpoints', value: (player) => player.hitpoints_level.toLocaleString(), rank: (ranks) => ranks.hitpoints_level },
  { label: 'Fishing', value: (player) => player.fishing_level.toLocaleString(), rank: (ranks) => ranks.fishing_level },
  { label: 'Farming', value: (player) => player.farming_level.toLocaleString(), rank: (ranks) => ranks.farming_level },
  { label: 'Woodcutting', value: (player) => player.woodcutting_level.toLocaleString(), rank: (ranks) => ranks.woodcutting_level },
  { label: 'Mining', value: (player) => player.mining_level.toLocaleString(), rank: (ranks) => ranks.mining_level },
  { label: 'Smithing', value: (player) => player.smithing_level.toLocaleString(), rank: (ranks) => ranks.smithing_level },
  { label: 'Alchemy', value: (player) => player.alchemy_level.toLocaleString(), rank: (ranks) => ranks.alchemy_level },
  { label: 'Prayer', value: (player) => player.prayer_level.toLocaleString(), rank: (ranks) => ranks.prayer_level },
  { label: 'Magic', value: (player) => player.magic_level.toLocaleString(), rank: (ranks) => ranks.magic_level },
  { label: 'Slayer', value: (player) => player.slayer_level.toLocaleString(), rank: (ranks) => ranks.slayer_level },
  { label: 'Monster Kills', value: (player) => player.monster_kills.toLocaleString(), rank: (ranks) => ranks.monster_kills },
  { label: 'Played Time', value: (player) => formatPlayedTime(player.played_time), rank: (ranks) => ranks.played_time },
]

function formatPlayedTime(seconds: number) {
  const days = Math.floor(seconds / 86_400)
  const hours = Math.floor((seconds % 86_400) / 3_600)
  if (days > 0) return `${days}d ${hours}h`
  const minutes = Math.floor((seconds % 3_600) / 60)
  return `${hours}h ${minutes}m`
}

function percentile(rank: number, total: number) {
  if (total <= 1) return 'Top 100%'
  const fraction = rank / total
  const pct = Math.max(1, Math.round(fraction * 100))
  return `Top ${pct}%`
}

export function PlayerProfile() {
  const params = useParams<{ name: string }>()
  const playerName = params.name ?? ''
  const [copied, setCopied] = useState(false)
  const appBase = import.meta.env.BASE_URL

  useEffect(() => {
    document.title = playerName
      ? `${playerName} — New Aeven Player Profile`
      : 'Player Profile — New Aeven World Statistics'
  }, [playerName])

  const { data, isLoading, isError } = useQuery({
    queryKey: ['player-profile', playerName],
    queryFn: () => api.playerProfile(playerName),
    enabled: Boolean(playerName),
  })

  const sharePath = useMemo(
    () => `${appBase.replace(/\/$/, '')}/player/${encodeURIComponent(data?.player.name ?? playerName)}`,
    [appBase, data?.player.name, playerName],
  )

  async function copyUrl() {
    if (typeof window === 'undefined') return
    const url = `${window.location.origin}${sharePath}`
    try {
      await navigator.clipboard.writeText(url)
      setCopied(true)
      setTimeout(() => setCopied(false), 1600)
    } catch {
      setCopied(false)
    }
  }

  if (!playerName) {
    return (
      <div className="rounded-xl border border-[var(--panel-border)] bg-[var(--panel)] p-6">
        <p className="text-[var(--text-soft)]">Missing player name.</p>
      </div>
    )
  }

  if (isLoading) {
    return (
      <div className="space-y-4">
        <div className="h-36 animate-pulse rounded-xl border border-[var(--panel-border)] bg-[var(--panel-soft)]" />
        <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
          {Array.from({ length: 8 }).map((_, i) => (
            <div key={i} className="h-28 animate-pulse rounded-xl border border-[var(--panel-border)] bg-[var(--panel-soft)]" />
          ))}
        </div>
      </div>
    )
  }

  if (isError || !data) {
    return (
      <div className="space-y-4 rounded-xl border border-[var(--panel-border)] bg-[var(--panel)] p-6">
        <h1 className="text-2xl text-[var(--text)]">Player not found</h1>
        <p className="text-[var(--text-soft)]">No profile data exists for "{playerName}".</p>
        <Link to="/leaderboards" className="pixel-btn inline-flex rounded-md bg-[var(--panel-soft)] px-3 py-2 text-sm text-[var(--text)]">
          Back to leaderboards
        </Link>
      </div>
    )
  }

  const { player, ranks, total_characters } = data

  return (
    <div className="space-y-5">
      <section className="pixel-box relative overflow-hidden rounded-xl bg-[radial-gradient(circle_at_25%_10%,rgba(212,168,68,0.22),transparent_50%),radial-gradient(circle_at_90%_0%,rgba(90,114,71,0.16),transparent_48%),var(--panel)] p-6 md:p-7">
        <p className="text-xs uppercase tracking-[0.22em] text-[var(--muted)]" style={{ fontFamily: 'var(--font-display)' }}>Player Showcase</p>
        <h1 className="mt-2 text-4xl text-[var(--text)]">{player.name}</h1>
        <div className="mt-3 flex flex-wrap gap-2 text-sm">
          <span className="rounded-full border border-[var(--panel-border)] bg-[var(--panel-soft)] px-3 py-1 text-[var(--text-soft)]">
            #{ranks.total_level} Total Level
          </span>
          <span className="rounded-full border border-[var(--panel-border)] bg-[var(--panel-soft)] px-3 py-1 text-[var(--text-soft)]">
            #{ranks.monster_kills} Monster Kills
          </span>
          <span className="rounded-full border border-[var(--panel-border)] bg-[var(--panel-soft)] px-3 py-1 text-[var(--text-soft)]">
            {percentile(ranks.total_level, total_characters)}
          </span>
        </div>
        <div className="mt-5 flex flex-wrap gap-3">
          <button
            onClick={copyUrl}
            className="pixel-btn rounded-md bg-[var(--gold)] px-4 py-2 text-sm font-bold text-[#1a1210] hover:bg-[var(--gold-light)]"
          >
            {copied ? 'Link copied' : 'Copy profile URL'}
          </button>
          <Link
            to="/leaderboards"
            className="pixel-btn rounded-md bg-[var(--panel-soft)] px-4 py-2 text-sm font-bold text-[var(--text-soft)] hover:text-[var(--text)]"
          >
            View all leaderboards
          </Link>
        </div>
        <p className="mt-3 text-xs text-[var(--muted)]">{sharePath}</p>
      </section>

      <section className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
        {PROFILE_STATS.map((stat) => (
          <article
            key={stat.label}
            className="pixel-box rounded-xl bg-[var(--panel)] p-4"
          >
            <p className="text-[11px] uppercase tracking-[0.14em] text-[var(--muted)]" style={{ fontFamily: 'var(--font-display)' }}>{stat.label}</p>
            <p className="mt-2 text-2xl font-bold text-[var(--text)]">{stat.value(player)}</p>
            <p className="mt-2 text-xs text-[var(--text-soft)]">
              Global rank <span className="font-mono text-[var(--gold)]">#{stat.rank(ranks)}</span>
            </p>
          </article>
        ))}
      </section>
    </div>
  )
}
