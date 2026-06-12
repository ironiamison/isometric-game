const MUTATING_METHODS = new Set(['POST', 'PUT', 'PATCH', 'DELETE']);

function cookie(name: string): string | null {
  const prefix = `${encodeURIComponent(name)}=`;
  for (const part of document.cookie.split(';')) {
    const value = part.trim();
    if (value.startsWith(prefix)) return decodeURIComponent(value.slice(prefix.length));
  }
  return null;
}

function isMapperApi(input: RequestInfo | URL): boolean {
  const raw = input instanceof Request ? input.url : input.toString();
  const url = new URL(raw, window.location.origin);
  return url.origin === window.location.origin
    && (url.pathname.startsWith('/api/') || url.pathname.startsWith('/mapper/api/'));
}

export function installApiSecurity(): void {
  const nativeFetch = window.fetch.bind(window);
  window.fetch = async (input: RequestInfo | URL, init: RequestInit = {}) => {
    const requestMethod = input instanceof Request ? input.method : undefined;
    const method = (init.method || requestMethod || 'GET').toUpperCase();
    const headers = new Headers(input instanceof Request ? input.headers : undefined);
    new Headers(init.headers).forEach((value, key) => headers.set(key, value));

    if (isMapperApi(input) && MUTATING_METHODS.has(method)) {
      const csrfToken = cookie('mapper_csrf');
      if (!csrfToken) throw new Error('Missing mapper CSRF token; sign in again');
      headers.set('x-csrf-token', csrfToken);
    }

    const response = await nativeFetch(input, {
      ...init,
      headers,
      credentials: 'same-origin',
    });
    if (response.status === 401) {
      window.location.assign('/mapper/login');
    }
    return response;
  };
}
