export function formatPlayedTime(seconds: number): string {
  const days = Math.floor(seconds / 86_400);
  const hours = Math.floor((seconds % 86_400) / 3_600);
  if (days > 0) return `${days}d ${hours}h`;
  const minutes = Math.floor((seconds % 3_600) / 60);
  return `${hours}h ${minutes}m`;
}

export function formatChance(chance: number): string {
  if (chance >= 1) return 'Always';
  return `${(chance * 100).toFixed(chance < 0.01 ? 1 : 0)}%`;
}

export function formatItemName(id: string): string {
  return id.replace(/_/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase());
}

export function signed(n: number): string {
  return n >= 0 ? `+${n}` : `${n}`;
}

export function percentile(rank: number, total: number): string {
  if (total <= 1) return 'Top 100%';
  const pct = Math.max(1, Math.round((rank / total) * 100));
  return `Top ${pct}%`;
}
