import { useState } from 'react'
import { NavLink, Outlet } from 'react-router-dom'
import { LayoutGrid, Users, Trophy, Gem, Skull, Gamepad2, X, Menu } from 'lucide-react'

const navItems = [
  { to: '/', label: 'Overview', icon: LayoutGrid },
  { to: '/players', label: 'Players', icon: Users },
  { to: '/leaderboards', label: 'Leaderboards', icon: Trophy },
  { to: '/items', label: 'Items', icon: Gem },
  { to: '/bestiary', label: 'Bestiary', icon: Skull },
]

export function Layout() {
  const [mobileOpen, setMobileOpen] = useState(false)

  return (
    <div className="relative min-h-screen overflow-x-clip bg-[var(--bg)]">
      <div
        aria-hidden
        className="pointer-events-none fixed inset-0 bg-[radial-gradient(900px_480px_at_12%_-10%,rgba(90,64,30,0.22),transparent_62%),radial-gradient(800px_440px_at_96%_0%,rgba(212,168,68,0.1),transparent_58%),radial-gradient(1100px_700px_at_50%_100%,rgba(42,30,20,0.28),transparent_65%)]"
      />

      {/* Top header */}
      <header className="sticky top-0 z-40 border-b border-[var(--panel-border)]/50 bg-[var(--bg)]/80 px-6 backdrop-blur-md md:px-10">
        <div className="mx-auto flex max-w-6xl items-center justify-between py-3">
          <NavLink to="/">
            <span
              className="text-base font-bold tracking-[0.22em] text-[var(--gold)]"
              style={{ fontFamily: 'var(--font-display)' }}
            >
              NEW AEVEN
            </span>
          </NavLink>

          {/* Desktop nav */}
          <nav className="hidden items-center gap-1 md:flex">
            {navItems.map((item) => (
              <NavLink
                key={item.to}
                to={item.to}
                end={item.to === '/'}
                className={({ isActive }) =>
                  `relative flex items-center gap-1.5 px-3 py-1.5 text-[13px] font-semibold tracking-wide transition-colors duration-150 ${
                    isActive
                      ? 'text-[var(--gold)]'
                      : 'text-[var(--text-soft)] hover:text-[var(--text)]'
                  }`
                }
              >
                {({ isActive }) => (
                  <>
                    <item.icon size={13} strokeWidth={2} className="shrink-0 -translate-y-px" />
                    {item.label}
                    {isActive && (
                      <span className="absolute bottom-0 left-3 right-3 h-px bg-[var(--gold)]" />
                    )}
                  </>
                )}
              </NavLink>
            ))}
          </nav>

          {/* Back to game link - desktop */}
          <a
            href="https://aeven.xyz/#play"
            className="hidden items-center gap-1.5 text-[10px] tracking-[0.12em] text-[var(--muted)] uppercase transition-colors hover:text-[var(--text-soft)] md:inline-flex"
            style={{ fontFamily: 'var(--font-display)' }}
          >
            <Gamepad2 size={13} />
            Play game
          </a>

          {/* Mobile menu button */}
          <button
            onClick={() => setMobileOpen(!mobileOpen)}
            className="flex h-8 w-8 items-center justify-center text-[var(--text-soft)] md:hidden"
            aria-label="Toggle menu"
          >
            {mobileOpen ? <X size={18} /> : <Menu size={18} />}
          </button>
        </div>

        {/* Mobile nav dropdown */}
        {mobileOpen && (
          <nav className="border-t border-[var(--panel-border)]/30 bg-[var(--bg)]/95 px-6 py-3 backdrop-blur-md md:hidden">
            <div className="flex flex-col gap-1">
              {navItems.map((item) => (
                <NavLink
                  key={item.to}
                  to={item.to}
                  end={item.to === '/'}
                  onClick={() => setMobileOpen(false)}
                  className={({ isActive }) =>
                    `flex items-center gap-2 rounded-md px-3 py-2 text-sm font-semibold transition-colors ${
                      isActive
                        ? 'text-[var(--gold)]'
                        : 'text-[var(--text-soft)] active:text-[var(--text)]'
                    }`
                  }
                >
                  <item.icon size={15} strokeWidth={2} />
                  {item.label}
                </NavLink>
              ))}
              <div className="mt-2 border-t border-[var(--panel-border)]/30 pt-2">
                <a
                  href="https://aeven.xyz/#play"
                  className="flex items-center gap-2 px-3 py-2 text-xs tracking-[0.1em] text-[var(--muted)] uppercase"
                  style={{ fontFamily: 'var(--font-display)' }}
                >
                  <Gamepad2 size={13} />
                  Play game
                </a>
              </div>
            </div>
          </nav>
        )}
      </header>

      {/* Page content */}
      <main className="relative z-10 px-6 py-8 md:px-10 md:py-10">
        <div className="mx-auto max-w-6xl">
          <Outlet />
        </div>
      </main>
    </div>
  )
}
