import { getStaticItems } from '$lib/game-content';

export const prerender = true;

export function load() {
  return { items: getStaticItems() };
}
