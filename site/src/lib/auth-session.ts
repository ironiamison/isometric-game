export const AUTH_SESSION_KEY = 'solstead_auth_session';

/** Matches /api/characters response entries. */
export type CharacterInfo = {
	id: number;
	name: string;
	level: number;
	playedTime: number;
};

export type AuthSession = {
	token: string;
	username: string;
	characters?: CharacterInfo[];
};

export function readAuthSession(): AuthSession | null {
	if (typeof localStorage === 'undefined') return null;
	try {
		const raw = localStorage.getItem(AUTH_SESSION_KEY);
		if (!raw) return null;
		const data = JSON.parse(raw) as AuthSession;
		if (data?.token && data?.username) return data;
	} catch {
		// ignore corrupt storage
	}
	return null;
}

export function writeAuthSession(session: AuthSession): void {
	localStorage.setItem(AUTH_SESSION_KEY, JSON.stringify(session));
}

export function clearAuthSession(): void {
	localStorage.removeItem(AUTH_SESSION_KEY);
	localStorage.removeItem('remembered_username');
}

/** Prefer the character with the most playtime (fallback: first). */
export function pickPrimaryCharacter(characters: CharacterInfo[]): CharacterInfo | null {
	if (!characters.length) return null;
	return [...characters].sort((a, b) => b.playedTime - a.playedTime || b.level - a.level)[0];
}
