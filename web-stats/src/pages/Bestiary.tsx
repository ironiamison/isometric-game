import { useEffect, useState, useMemo } from 'react'
import { useQuery } from '@tanstack/react-query'
import { Link } from 'react-router-dom'
import { api, type Entity } from '../api'

export function Bestiary() {
  useEffect(() => { document.title = 'Bestiary — New Aeven World Statistics' }, [])
  const { data, isLoading } = useQuery({
    queryKey: ['entities'],
    queryFn: api.entities,
  })

  const [search, setSearch] = useState('')

  const filtered = useMemo(() => {
    if (!data) return []
    const q = search.toLowerCase()
    return [...data]
      .filter(e => !q || e.display_name.toLowerCase().includes(q) || e.id.toLowerCase().includes(q))
      .sort((a, b) => a.level - b.level || a.display_name.localeCompare(b.display_name))
  }, [data, search])

  return (
    <div className="space-y-6">
      <section className="pixel-box relative overflow-hidden rounded-xl bg-[radial-gradient(circle_at_20%_15%,rgba(180,60,60,0.18),transparent_45%),radial-gradient(circle_at_80%_0%,rgba(212,168,68,0.14),transparent_45%),var(--panel)] px-6 py-7 md:px-8">
        <p className="text-xs uppercase tracking-[0.22em] text-[var(--muted)]" style={{ fontFamily: 'var(--font-display)' }}>Field Guide</p>
        <h1 className="mt-2 text-3xl font-bold text-[var(--text)] md:text-4xl">Bestiary</h1>
        <p className="mt-2 max-w-2xl text-sm text-[var(--text-soft)]">
          Every monster in New Aeven. Stats, drops, and scaling — know your enemy.
        </p>
      </section>

      <div className="flex items-center gap-3">
        <input
          type="text"
          placeholder="Search monsters..."
          value={search}
          onChange={e => setSearch(e.target.value)}
          className="w-full max-w-sm rounded-lg border border-[var(--panel-border)] bg-[var(--bg)] px-4 py-2 text-sm text-[var(--text)] placeholder-[var(--muted)] outline-none focus:border-[var(--gold)] transition-colors"
        />
        {data && (
          <span className="rounded-full bg-[var(--gold)] px-3 py-0.5 text-sm font-bold text-[#1a1210]">
            {filtered.length}
          </span>
        )}
      </div>

      {isLoading ? (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
          {Array.from({ length: 8 }).map((_, i) => (
            <div key={i} className="bg-[var(--panel)] rounded-lg border border-[var(--panel-border)] p-4 space-y-3">
              <div className="h-5 w-32 rounded bg-[var(--panel-soft)] animate-pulse" />
              <div className="h-4 w-20 rounded bg-[var(--panel-soft)] animate-pulse" />
              <div className="h-4 w-full rounded bg-[var(--panel-soft)] animate-pulse" />
            </div>
          ))}
        </div>
      ) : filtered.length === 0 ? (
        <p className="text-center py-12 text-[var(--muted)]">No monsters found</p>
      ) : (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
          {filtered.map(entity => (
            <MonsterCard key={entity.id} entity={entity} />
          ))}
        </div>
      )}
    </div>
  )
}

function MonsterCard({ entity }: { entity: Entity }) {
  return (
    <Link
      to={`/bestiary/${encodeURIComponent(entity.id)}`}
      className="pixel-box bg-[var(--panel)] rounded-lg p-4 hover:border-[var(--gold)]/40 transition-colors space-y-2 block"
    >
      <div className="flex items-center justify-between">
        <p className="font-bold text-[var(--text)]">{entity.display_name}</p>
        <span className="rounded-full bg-[var(--panel-soft)] px-2 py-0.5 text-xs font-mono text-[var(--muted)]">
          Lv {entity.level}
        </span>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${
          entity.hostile
            ? 'bg-[var(--ember)]/20 text-[var(--ember)]'
            : 'bg-[var(--moss)]/20 text-[var(--moss-light)]'
        }`}>
          {entity.hostile ? 'Aggressive' : 'Passive'}
        </span>
        {entity.loot.length > 0 && (
          <span className="rounded-full bg-[var(--gold)]/15 px-2 py-0.5 text-xs text-[var(--gold)]">
            {entity.loot.length} drops
          </span>
        )}
      </div>

      {entity.description && (
        <p className="text-sm text-[var(--text-soft)] line-clamp-2">{entity.description}</p>
      )}

      <div className="flex flex-wrap gap-x-4 gap-y-1 pt-1 border-t border-[var(--panel-border)] text-xs">
        <span className="text-[var(--text-soft)]">HP <span className="font-mono text-[var(--text)]">{entity.max_hp}</span></span>
        <span className="text-[var(--text-soft)]">Dmg <span className="font-mono text-[var(--text)]">{entity.damage}</span></span>
        <span className="text-[var(--text-soft)]">XP <span className="font-mono text-[var(--text)]">{entity.exp_base * entity.level}</span></span>
      </div>
    </Link>
  )
}
