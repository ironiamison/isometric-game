import type { ContentCatalog, ContentData } from './types';

async function readJson<T>(response: Response): Promise<T> {
  const body = await response.json() as T & { error?: string };
  if (!response.ok) {
    throw new Error(body.error || `Request failed (${response.status})`);
  }
  return body;
}

export async function loadContentCatalog(): Promise<ContentCatalog> {
  const response = await fetch('/api/content/catalog', { cache: 'no-store' });
  return readJson<ContentCatalog>(response);
}

export async function saveContentEntry(
  file: string,
  id: string,
  data: ContentData
): Promise<void> {
  const response = await fetch('/api/content/entry', {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ file, id, data }),
  });
  await readJson(response);
}

export async function deleteContentEntry(file: string, id: string): Promise<void> {
  const query = new URLSearchParams({ file, id });
  const response = await fetch(`/api/content/entry?${query}`, { method: 'DELETE' });
  await readJson(response);
}
