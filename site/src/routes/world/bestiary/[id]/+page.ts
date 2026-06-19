import { getStaticEntities } from '$lib/game-content';

export const prerender = true;

export function entries() {
  return getStaticEntities().map((e) => ({ id: e.id }));
}

export function load({ params }: { params: { id: string } }) {
  const entities = getStaticEntities();
  return {
    entities,
    monster: entities.find((e) => e.id === params.id) ?? null,
  };
}
