export type SiteNavLink = {
  href: string;
  label: string;
  isActive: (pathname: string) => boolean;
};

export const SITE_NAV_LINKS: SiteNavLink[] = [
  {
    href: '/play/index.html',
    label: 'Home',
    isActive: (p) => p === '/' || p === '' || p.startsWith('/play'),
  },
  {
    href: '/world',
    label: 'World',
    isActive: (p) =>
      p.startsWith('/world') &&
      !p.startsWith('/world/leaderboards') &&
      p !== '/world/leaderboards',
  },
  {
    href: '/world/leaderboards',
    label: 'Leaderboards',
    isActive: (p) => p.startsWith('/world/leaderboards'),
  },
  {
    href: '/wiki',
    label: 'Wiki',
    isActive: (p) => p.startsWith('/wiki'),
  },
];

export type WorldSubNavLink = {
  href: string;
  label: string;
  exact?: boolean;
};

export const WORLD_SUB_NAV: WorldSubNavLink[] = [
  { href: '/world', label: 'World', exact: true },
  { href: '/world/players', label: 'Players' },
  { href: '/world/leaderboards', label: 'Leaderboards' },
  { href: '/world/items', label: 'Items' },
  { href: '/world/bestiary', label: 'Bestiary' },
];

export function worldSubNavActive(pathname: string, href: string, exact = false) {
  if (exact) return pathname === href || pathname === `${href}/`;
  return pathname === href || pathname.startsWith(`${href}/`);
}
