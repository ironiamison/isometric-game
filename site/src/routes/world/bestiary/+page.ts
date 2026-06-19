import { getStaticEntities } from '$lib/game-content';

export const prerender = true;

export function load() {
  return { entities: getStaticEntities() };
}
