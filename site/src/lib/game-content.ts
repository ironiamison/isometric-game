import gameData from './wiki/game-data.json';
import type { Entity, Item } from './api';

type GameDataFile = {
  items: Item[];
  entities: Entity[];
};

const data = gameData as GameDataFile;

export function getStaticItems(): Item[] {
  return data.items;
}

export function getStaticEntities(): Entity[] {
  return data.entities;
}

export function getStaticEntity(id: string): Entity | undefined {
  return data.entities.find((e) => e.id === id);
}
