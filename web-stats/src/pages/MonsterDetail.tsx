import { useEffect, useMemo } from 'react'
import { Link, useParams } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'
import { api } from '../api'
import { Skull, ArrowLeft, TrendingUp, Droplets, ScrollText } from 'lucide-react'

function scaleHp(baseHp: number, level: number) {
  return Math.round(baseHp * (1 + 0.10 * Math.max(0, level - 1)))
}

function scaleDamage(baseDmg: number, level: number) {
  return Math.round(baseDmg * (1 + 0.15 * Math.max(0, level - 1)))
}

function formatRespawn(ms: number) {
  const seconds = ms / 1000
  if (seconds >= 60) {
    const m = Math.floor(seconds / 60)
    const s = seconds % 60
    return s > 0 ? `${m}m ${s}s` : `${m}m`
  }
  return `${seconds}s`
}

export function MonsterDetail() {
  const params = useParams<{ id: string }>()
  const monsterId = params.id ?? ''

  const { data: entities, isLoading } = useQuery({
    queryKey: ['entities'],
    queryFn: api.entities,
  })

  const monster = useMemo(
    () => entities?.find(e => e.id === monsterId),
    [entities, monsterId],
  )

  useEffect(() => {
    document.title = monster
      ? `${monster.display_name} — New Aeven Bestiary`
      : 'Bestiary — New Aeven World Statistics'
  }, [monster])

  // Generate level scaling table
  // Show levels from 1 up to max(monster.level, 20) to give a useful range
  const scalingRows = useMemo(() => {
    if (!monster) return []
    const maxLevel = Math.max(monster.level, 20)
    return Array.from({ length: maxLevel }, (_, i) => {
      const level = i + 1
      return {
        level,
        hp: scaleHp(monster.max_hp, level),
        damage: scaleDamage(monster.damage, level),
        exp: monster.exp_base * level,
        goldMin: monster.gold_min * level,
        goldMax: monster.gold_max * level,
      }
    })
  }, [monster])

  if (isLoading) {
    return (
      <div className="space-y-4">
        <div className="h-36 animate-pulse rounded-xl border border-[var(--panel-border)] bg-[var(--panel-soft)]" />
        <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
          {Array.from({ length: 6 }).map((_, i) => (
            <div key={i} className="h-24 animate-pulse rounded-xl border border-[var(--panel-border)] bg-[var(--panel-soft)]" />
          ))}
        </div>
      </div>
    )
  }

  if (!monster) {
    return (
      <div className="space-y-4 rounded-xl border border-[var(--panel-border)] bg-[var(--panel)] p-6">
        <h1 className="text-2xl text-[var(--text)]">Monster not found</h1>
        <p className="text-[var(--text-soft)]">No data exists for "{monsterId}".</p>
        <Link to="/bestiary" className="pixel-btn inline-flex rounded-md bg-[var(--panel-soft)] px-3 py-2 text-sm text-[var(--text)]">
          Back to Bestiary
        </Link>
      </div>
    )
  }

  return (
    <div className="space-y-5">
      {/* Header */}
      <section className="pixel-box relative overflow-hidden rounded-xl bg-[radial-gradient(circle_at_25%_10%,rgba(180,60,60,0.18),transparent_50%),radial-gradient(circle_at_90%_0%,rgba(212,168,68,0.14),transparent_48%),var(--panel)] p-6 md:p-7">
        <p className="flex items-center gap-2 text-xs uppercase tracking-[0.22em] text-[var(--muted)]" style={{ fontFamily: 'var(--font-display)' }}>
          <Skull size={14} className="text-[var(--ember)]" />
          Bestiary Entry
        </p>
        <h1 className="mt-2 text-4xl text-[var(--text)]">{monster.display_name}</h1>
        {monster.description && (
          <p className="mt-2 text-sm text-[var(--text-soft)]">{monster.description}</p>
        )}
        <div className="mt-3 flex flex-wrap gap-2 text-sm">
          <span className="rounded-full border border-[var(--panel-border)] bg-[var(--panel-soft)] px-3 py-1 font-mono text-[var(--text-soft)]">
            Level {monster.level}
          </span>
          <span className={`rounded-full px-3 py-1 font-medium ${
            monster.hostile
              ? 'border border-[var(--ember)]/30 bg-[var(--ember)]/10 text-[var(--ember)]'
              : 'border border-[var(--moss)]/30 bg-[var(--moss)]/10 text-[var(--moss-light)]'
          }`}>
            {monster.hostile ? 'Aggressive' : 'Passive'}
          </span>
        </div>
        <div className="mt-5">
          <Link
            to="/bestiary"
            className="pixel-btn inline-flex items-center gap-1.5 rounded-md bg-[var(--panel-soft)] px-4 py-2 text-sm font-bold text-[var(--text-soft)] hover:text-[var(--text)]"
          >
            <ArrowLeft size={14} />
            Bestiary
          </Link>
        </div>
      </section>

      {/* Combat Stats */}
      <section className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
        <StatCard label="Hitpoints" value={monster.max_hp.toString()} />
        <StatCard label="Damage" value={monster.damage.toString()} />
        <StatCard label="Attack Bonus" value={signed(monster.attack_bonus)} />
        <StatCard label="Defence Bonus" value={signed(monster.defence_bonus)} />
        <StatCard label="Attack Range" value={`${monster.attack_range} tile${monster.attack_range !== 1 ? 's' : ''}`} />
        <StatCard label="Aggro Range" value={`${monster.aggro_range} tiles`} />
        <StatCard label="Respawn" value={formatRespawn(monster.respawn_time_ms)} />
        <StatCard label="Base XP" value={monster.exp_base.toString()} />
      </section>

      {/* Level Scaling Table */}
      <section className="pixel-box rounded-xl bg-[var(--panel)] p-4 md:p-5 space-y-3">
        <p className="flex items-center gap-2 text-[11px] uppercase tracking-[0.2em] text-[var(--muted)]" style={{ fontFamily: 'var(--font-display)' }}>
          <TrendingUp size={13} className="text-[var(--gold)]" />
          Level Scaling
        </p>
        <p className="text-xs text-[var(--text-soft)]">
          HP scales +10% per level, damage +15% per level, XP and gold multiply by level.
        </p>
        <div className="overflow-x-auto rounded-xl border border-[var(--panel-border)]">
          <table className="w-full min-w-[500px]">
            <thead>
              <tr className="bg-[var(--panel-soft)]">
                <th className="px-4 py-3 text-left text-[11px] uppercase tracking-[0.14em] text-[var(--muted)]">Level</th>
                <th className="px-4 py-3 text-left text-[11px] uppercase tracking-[0.14em] text-[var(--muted)]">HP</th>
                <th className="px-4 py-3 text-left text-[11px] uppercase tracking-[0.14em] text-[var(--muted)]">Damage</th>
                <th className="px-4 py-3 text-left text-[11px] uppercase tracking-[0.14em] text-[var(--muted)]">XP</th>
                <th className="px-4 py-3 text-left text-[11px] uppercase tracking-[0.14em] text-[var(--muted)]">Gold</th>
              </tr>
            </thead>
            <tbody>
              {scalingRows.map(row => (
                <tr
                  key={row.level}
                  className={`border-t border-[var(--panel-border)] ${
                    row.level === monster.level ? 'bg-[var(--gold)]/8' : 'hover:bg-[var(--panel-soft)]/70'
                  }`}
                >
                  <td className="px-4 py-2 font-mono text-sm text-[var(--text-soft)]">
                    {row.level}
                    {row.level === monster.level && (
                      <span className="ml-2 text-[10px] text-[var(--gold)]">BASE</span>
                    )}
                  </td>
                  <td className="px-4 py-2 font-mono text-sm text-[var(--text)]">{row.hp}</td>
                  <td className="px-4 py-2 font-mono text-sm text-[var(--text)]">{row.damage}</td>
                  <td className="px-4 py-2 font-mono text-sm text-[var(--text)]">{row.exp}</td>
                  <td className="px-4 py-2 font-mono text-sm text-[var(--text)]">
                    {row.goldMin === row.goldMax ? row.goldMin : `${row.goldMin}–${row.goldMax}`}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>

      {/* Drop Table */}
      {monster.loot.length > 0 && (
        <section className="pixel-box rounded-xl bg-[var(--panel)] p-4 md:p-5 space-y-3">
          <p className="flex items-center gap-2 text-[11px] uppercase tracking-[0.2em] text-[var(--muted)]" style={{ fontFamily: 'var(--font-display)' }}>
            <Droplets size={13} className="text-[var(--water)]" />
            Drop Table
          </p>
          <div className="overflow-x-auto rounded-xl border border-[var(--panel-border)]">
            <table className="w-full">
              <thead>
                <tr className="bg-[var(--panel-soft)]">
                  <th className="px-4 py-3 text-left text-[11px] uppercase tracking-[0.14em] text-[var(--muted)]">Item</th>
                  <th className="px-4 py-3 text-left text-[11px] uppercase tracking-[0.14em] text-[var(--muted)]">Drop Chance</th>
                  <th className="px-4 py-3 text-left text-[11px] uppercase tracking-[0.14em] text-[var(--muted)]">Quantity</th>
                </tr>
              </thead>
              <tbody>
                {monster.loot.map((drop, i) => (
                  <tr key={i} className="border-t border-[var(--panel-border)] hover:bg-[var(--panel-soft)]/70">
                    <td className="px-4 py-2 text-sm font-medium text-[var(--text)]">
                      {formatItemName(drop.item_id)}
                    </td>
                    <td className="px-4 py-2 font-mono text-sm">
                      <span className={drop.drop_chance >= 1 ? 'text-[var(--moss-light)]' : 'text-[var(--text)]'}>
                        {formatChance(drop.drop_chance)}
                      </span>
                    </td>
                    <td className="px-4 py-2 font-mono text-sm text-[var(--text-soft)]">
                      {drop.quantity_min === drop.quantity_max
                        ? drop.quantity_min
                        : `${drop.quantity_min}–${drop.quantity_max}`}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>
      )}

      {/* Related Quests */}
      {monster.quest_ids.length > 0 && (
        <section className="pixel-box rounded-xl bg-[var(--panel)] p-4 md:p-5 space-y-3">
          <p className="flex items-center gap-2 text-[11px] uppercase tracking-[0.2em] text-[var(--muted)]" style={{ fontFamily: 'var(--font-display)' }}>
            <ScrollText size={13} className="text-[var(--gold)]" />
            Related Quests
          </p>
          <div className="flex flex-wrap gap-2">
            {monster.quest_ids.map(qid => (
              <span
                key={qid}
                className="rounded-full border border-[var(--panel-border)] bg-[var(--panel-soft)] px-3 py-1 text-sm text-[var(--text-soft)]"
              >
                {formatItemName(qid)}
              </span>
            ))}
          </div>
        </section>
      )}
    </div>
  )
}

function StatCard({ label, value }: { label: string; value: string }) {
  return (
    <article className="pixel-box rounded-xl bg-[var(--panel)] p-4">
      <p className="text-[11px] uppercase tracking-[0.14em] text-[var(--muted)]" style={{ fontFamily: 'var(--font-display)' }}>{label}</p>
      <p className="mt-2 text-2xl font-bold text-[var(--text)]">{value}</p>
    </article>
  )
}

function signed(n: number) {
  return n >= 0 ? `+${n}` : `${n}`
}

function formatChance(chance: number) {
  if (chance >= 1) return 'Always'
  return `${(chance * 100).toFixed(chance < 0.01 ? 1 : 0)}%`
}

function formatItemName(id: string) {
  return id.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase())
}
