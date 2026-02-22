import { useState, useMemo } from 'react'
import { useQuery } from '@tanstack/react-query'
import { Link } from 'react-router-dom'
import { api, type OnlinePlayer } from '../api'

type SortKey = keyof OnlinePlayer
type SortDir = 'asc' | 'desc'

export function OnlinePlayers() {
  const { data, isLoading } = useQuery({
    queryKey: ['online'],
    queryFn: api.online,
    refetchInterval: 15000,
  })

  const [sortKey, setSortKey] = useState<SortKey>('combat_level')
  const [sortDir, setSortDir] = useState<SortDir>('desc')

  const sorted = useMemo(() => {
    if (!data) return []
    return [...data].sort((a, b) => {
      const av = a[sortKey]
      const bv = b[sortKey]
      if (av < bv) return sortDir === 'asc' ? -1 : 1
      if (av > bv) return sortDir === 'asc' ? 1 : -1
      return 0
    })
  }, [data, sortKey, sortDir])

  function toggleSort(key: SortKey) {
    if (sortKey === key) {
      setSortDir(d => (d === 'asc' ? 'desc' : 'asc'))
    } else {
      setSortKey(key)
      setSortDir('desc')
    }
  }

  const columns: { key: SortKey; label: string }[] = [
    { key: 'name', label: 'Name' },
    { key: 'combat_level', label: 'Combat Lv' },
    { key: 'hitpoints_level', label: 'Hitpoints' },
    { key: 'combat_skill_level', label: 'Combat' },
    { key: 'total_level', label: 'Total Lv' },
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-3">
        <h1 className="text-2xl font-bold text-[var(--text)]">Online Players</h1>
        {data && (
          <span className="rounded-full bg-[var(--gold)] px-3 py-0.5 text-sm font-bold text-[#1a1210]">
            {data.length}
          </span>
        )}
      </div>

      <div className="pixel-box bg-[var(--panel)] rounded-lg overflow-x-auto">
        <table className="w-full">
          <thead>
            <tr className="bg-[var(--panel-soft)]">
              {columns.map(col => (
                <th
                  key={col.key}
                  onClick={() => toggleSort(col.key)}
                  className="cursor-pointer px-4 py-3 text-left text-xs uppercase tracking-wider text-[var(--muted)] select-none hover:text-[var(--text)] transition-colors"
                >
                  {col.label}
                  {sortKey === col.key && (
                    <span className="ml-1">{sortDir === 'asc' ? '▲' : '▼'}</span>
                  )}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {isLoading ? (
              Array.from({ length: 6 }).map((_, i) => (
                <tr key={i} className="border-b border-[var(--panel-border)]">
                  {columns.map(col => (
                    <td key={col.key} className="px-4 py-3">
                      <div className="h-4 w-16 rounded bg-[var(--panel-soft)] animate-pulse" />
                    </td>
                  ))}
                </tr>
              ))
            ) : sorted.length === 0 ? (
              <tr>
                <td colSpan={columns.length} className="px-4 py-12 text-center text-[var(--muted)]">
                  Nobody's online right now
                </td>
              </tr>
            ) : (
              sorted.map(player => (
                <tr key={player.name} className="border-b border-[var(--panel-border)] hover:bg-[var(--panel-soft)] transition-colors">
                  <td className="px-4 py-3">
                    <Link
                      to={`/player/${encodeURIComponent(player.name)}`}
                      className="text-[var(--text)] hover:text-[var(--gold)]"
                    >
                      {player.name}
                    </Link>
                  </td>
                  <td className="px-4 py-3 font-mono text-[var(--text)]">{player.combat_level}</td>
                  <td className="px-4 py-3 font-mono text-[var(--text)]">{player.hitpoints_level}</td>
                  <td className="px-4 py-3 font-mono text-[var(--text)]">{player.combat_skill_level}</td>
                  <td className="px-4 py-3 font-mono text-[var(--text)]">{player.total_level}</td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  )
}
