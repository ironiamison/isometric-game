import { useEffect, useState, useMemo } from 'react'
import { useQuery } from '@tanstack/react-query'
import { api, type Item } from '../api'

const CATEGORIES = ['all', 'equipment', 'consumable', 'material', 'quest'] as const

const categoryBadge: Record<string, string> = {
  equipment: 'bg-[var(--water)]/20 text-[var(--water)]',
  consumable: 'bg-[var(--moss)]/20 text-[var(--moss-light)]',
  material: 'bg-[var(--gold)]/20 text-[var(--gold)]',
  quest: 'bg-[var(--ember)]/20 text-[var(--ember)]',
}

function StatLine({ label, value }: { label: string; value: number }) {
  if (value === 0) return null
  const color = value > 0 ? 'text-[var(--moss-light)]' : 'text-[var(--ember)]'
  const prefix = value > 0 ? '+' : ''
  return (
    <span className={`text-xs ${color}`}>
      {prefix}{value} {label}
    </span>
  )
}

export function ItemRegistry() {
  useEffect(() => { document.title = 'Item Registry — New Aeven World Statistics' }, [])
  const { data, isLoading } = useQuery({
    queryKey: ['items'],
    queryFn: api.items,
  })

  const [search, setSearch] = useState('')
  const [category, setCategory] = useState<string>('all')

  const filtered = useMemo(() => {
    if (!data) return []
    const q = search.toLowerCase()
    return [...data]
      .filter(item => {
        if (category !== 'all' && item.category !== category) return false
        if (q && !item.display_name.toLowerCase().includes(q) && !item.id.toLowerCase().includes(q)) return false
        return true
      })
      .sort((a, b) => a.display_name.localeCompare(b.display_name))
  }, [data, search, category])

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-3">
        <h1 className="text-2xl font-bold text-[var(--text)]">Item Registry</h1>
        {data && (
          <span className="rounded-full bg-[var(--gold)] px-3 py-0.5 text-sm font-bold text-[#1a1210]">
            {filtered.length}
          </span>
        )}
      </div>

      {/* Search */}
      <input
        type="text"
        placeholder="Search items..."
        value={search}
        onChange={e => setSearch(e.target.value)}
        className="w-full max-w-sm rounded-lg border border-[var(--panel-border)] bg-[var(--bg)] px-4 py-2 text-sm text-[var(--text)] placeholder-[var(--muted)] outline-none focus:border-[var(--gold)] transition-colors"
      />

      {/* Category filters */}
      <div className="flex flex-wrap gap-2">
        {CATEGORIES.map(cat => (
          <button
            key={cat}
            onClick={() => setCategory(cat)}
            className={`pixel-btn rounded-md px-4 py-1.5 text-xs font-bold transition-colors ${
              category === cat
                ? 'bg-[var(--gold)] text-[#1a1210]'
                : 'bg-[var(--panel)] text-[var(--text-soft)] hover:text-[var(--text)]'
            }`}
          >
            {cat.charAt(0).toUpperCase() + cat.slice(1)}
          </button>
        ))}
      </div>

      {/* Items grid */}
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
        <p className="text-center py-12 text-[var(--muted)]">No items found</p>
      ) : (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
          {filtered.map(item => (
            <ItemCard key={item.id} item={item} />
          ))}
        </div>
      )}
    </div>
  )
}

function ItemCard({ item }: { item: Item }) {
  const eq = item.equipment

  return (
    <div className="pixel-box bg-[var(--panel)] rounded-lg p-4 hover:border-[var(--gold)]/40 transition-colors space-y-2">
      <p className="font-bold text-[var(--text)]">{item.display_name}</p>

      <div className="flex flex-wrap items-center gap-2">
        <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${categoryBadge[item.category] ?? 'bg-[var(--panel-soft)] text-[var(--muted)]'}`}>
          {item.category}
        </span>
        {eq && (
          <span className="rounded-full bg-[var(--panel-soft)] px-2 py-0.5 text-xs text-[var(--muted)]">
            {eq.slot_type}
          </span>
        )}
      </div>

      {item.description && (
        <p className="text-sm text-[var(--text-soft)] line-clamp-2">{item.description}</p>
      )}

      {item.base_price > 0 && (
        <p className="text-sm text-[var(--gold)]">{item.base_price.toLocaleString()} gold</p>
      )}

      {eq && (
        <div className="space-y-1 pt-1 border-t border-[var(--panel-border)]">
          <div className="flex flex-wrap gap-x-3 gap-y-0.5">
            <StatLine label="Attack" value={eq.attack_bonus} />
            <StatLine label="Strength" value={eq.strength_bonus} />
            <StatLine label="Defence" value={eq.defence_bonus} />
          </div>
          {eq.slot_type === 'weapon' && eq.attack_level_required > 1 && (
            <p className="text-xs text-[var(--gold)]">Requires {eq.attack_level_required} Attack</p>
          )}
          {eq.slot_type !== 'weapon' && eq.defence_level_required > 1 && (
            <p className="text-xs text-[var(--gold)]">Requires {eq.defence_level_required} Defence</p>
          )}
        </div>
      )}
    </div>
  )
}
