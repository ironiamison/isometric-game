const UTM_KEYS = ['utm_source', 'utm_medium', 'utm_campaign', 'utm_term', 'utm_content'] as const;
const STORAGE_KEY = 'aeven_utm';

export function captureUtms(): void {
  if (typeof window === 'undefined') return;
  const params = new URLSearchParams(window.location.search);
  const captured: Record<string, string> = {};
  for (const key of UTM_KEYS) {
    const value = params.get(key);
    if (value) captured[key] = value;
  }
  if (Object.keys(captured).length > 0) {
    sessionStorage.setItem(STORAGE_KEY, JSON.stringify(captured));
  }
}

export function appendUtms(url: string): string {
  if (typeof window === 'undefined') return url;
  const raw = sessionStorage.getItem(STORAGE_KEY);
  const normalized = url.endsWith('/') || url.includes('.') ? url : `${url}/`;
  if (!raw) return normalized;
  try {
    const utms = JSON.parse(raw) as Record<string, string>;
    const target = new URL(normalized, window.location.origin);
    for (const [key, value] of Object.entries(utms)) {
      target.searchParams.set(key, value);
    }
    return `${target.pathname}${target.search}${target.hash}`;
  } catch {
    return normalized;
  }
}
