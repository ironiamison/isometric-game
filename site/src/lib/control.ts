// Authed client for the /control ops panel. All paths are same-origin relative.

export class UnauthorizedError extends Error {
  constructor() {
    super('Unauthorized');
    this.name = 'UnauthorizedError';
  }
}

export interface PerfSnapshot {
  // Loosely typed: we render a subset and JSON-dump the rest.
  current_load: {
    rooms: number;
    connected_players: number;
    overworld_players: number;
    instance_players: number;
    spectators?: number;
  };
  recent_spikes: { context: string; [k: string]: unknown }[];
  [k: string]: unknown;
}

export interface LogEntry {
  level: string;
  message: string;
  timestamp?: string;
  [k: string]: unknown;
}

export interface AdminRoomSummary {
  room_id: string;
  player_count: number;
  npc_count: number;
  overworld_players: number;
  instance_players: number;
}

export interface AdminPlayer {
  id: string;
  name: string;
  room_id: string;
  instance_id: string | null;
  x: number;
  y: number;
  z: number;
  hp: number;
  max_hp: number;
  combat_level: number;
  active: boolean;
  is_dead: boolean;
  target_id: string | null;
  is_admin: boolean;
  is_god_mode: boolean;
  ip_address: string | null;
}

export interface AdminNpc {
  id: string;
  prototype_id: string;
  display_name: string;
  x: number;
  y: number;
  z: number;
  hp: number;
  max_hp: number;
  level: number;
  state: string;
  target_id: string | null;
  hidden: boolean;
  invulnerable: boolean;
}

export interface AdminRoomEntities {
  room_id: string;
  npcs: AdminNpc[];
  players: AdminPlayer[];
}

async function get<T>(path: string, token: string): Promise<T> {
  const r = await fetch(path, {
    headers: { Authorization: `Bearer ${token}` },
  });
  if (r.status === 401) throw new UnauthorizedError();
  if (!r.ok) throw new Error(`API error: ${r.status}`);
  return r.json();
}

export const control = {
  // Used by the login gate to validate a token (200 = valid).
  perf: (token: string) => get<PerfSnapshot>('/api/perf', token),
  logs: (token: string, opts: { count?: number; level?: string; important?: boolean } = {}) => {
    const q = new URLSearchParams();
    if (opts.count != null) q.set('count', String(opts.count));
    if (opts.level) q.set('level', opts.level);
    if (opts.important) q.set('important', 'true');
    const qs = q.toString();
    return get<LogEntry[]>(`/api/logs${qs ? `?${qs}` : ''}`, token);
  },
  rooms: (token: string) => get<AdminRoomSummary[]>('/api/admin/rooms', token),
  players: (token: string) => get<AdminPlayer[]>('/api/admin/players', token),
  roomEntities: (token: string, roomId: string) =>
    get<AdminRoomEntities>(`/api/admin/room/${encodeURIComponent(roomId)}/entities`, token),
};

const TOKEN_KEY = 'aeven_control_token';
export const tokenStore = {
  get: () => (typeof sessionStorage === 'undefined' ? null : sessionStorage.getItem(TOKEN_KEY)),
  set: (t: string) => sessionStorage.setItem(TOKEN_KEY, t),
  clear: () => sessionStorage.removeItem(TOKEN_KEY),
};
