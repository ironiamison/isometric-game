import type { ObjectDefinition, ObjectsConfig } from '@/types';

interface SpriteManifestAtlas {
  file: string;
  sprites: Record<string, { x: number; y: number; w: number; h: number }>;
}

interface SpriteManifest {
  objects_atlas: SpriteManifestAtlas;
  walls_atlas: SpriteManifestAtlas;
}

interface AnimatedSpriteEntry {
  fps: number;
  frames: number;
}

interface AnimatedSpritesData {
  objects: Record<string, AnimatedSpriteEntry>;
  walls: Record<string, AnimatedSpriteEntry>;
}

export class ObjectLoader {
  private objects: Map<number, ObjectDefinition> = new Map();
  private walls: Map<number, ObjectDefinition> = new Map();
  private firstGid: number = 1001;
  private wallsFirstGid: number = 1;
  private manifest: SpriteManifest | null = null;
  private animatedSprites: AnimatedSpritesData | null = null;

  private async loadManifest(): Promise<SpriteManifest> {
    if (this.manifest) return this.manifest;
    const resp = await fetch('/assets/sprite_manifest.json');
    this.manifest = await resp.json();
    return this.manifest!;
  }

  private async loadAnimatedSprites(): Promise<AnimatedSpritesData> {
    if (this.animatedSprites) return this.animatedSprites;
    try {
      const resp = await fetch('/assets/animated_sprites.json');
      this.animatedSprites = await resp.json();
    } catch {
      this.animatedSprites = { objects: {}, walls: {} };
    }
    return this.animatedSprites!;
  }

  private loadImage(src: string): Promise<HTMLImageElement> {
    return new Promise((resolve, reject) => {
      const img = new Image();
      img.onload = () => resolve(img);
      img.onerror = () => reject(new Error(`Failed to load: ${src}`));
      img.src = src;
    });
  }

  async loadObjects(config: ObjectsConfig): Promise<void> {
    this.firstGid = config.firstGid;

    const manifest = await this.loadManifest();
    const animData = await this.loadAnimatedSprites();
    const atlasInfo = manifest.objects_atlas;
    const atlasImage = await this.loadImage(`/assets/${atlasInfo.file}`);

    for (const item of config.items) {
      const spriteData = atlasInfo.sprites[String(item.id)];
      const anim = animData.objects[String(item.id)];
      const obj: ObjectDefinition = {
        ...item,
        image: spriteData ? atlasImage : undefined,
        atlasRect: spriteData ? {
          x: spriteData.x,
          y: spriteData.y,
          w: anim ? spriteData.w / anim.frames : spriteData.w,
          h: spriteData.h,
        } : undefined,
        frames: anim?.frames,
        fps: anim?.fps,
      };
      this.objects.set(item.id, obj);
    }
  }

  getObject(id: number): ObjectDefinition | undefined {
    return this.objects.get(id);
  }

  getAllObjects(): ObjectDefinition[] {
    return Array.from(this.objects.values());
  }

  getObjectsWithImages(): ObjectDefinition[] {
    return this.getAllObjects().filter((obj) => obj.image !== undefined);
  }

  // Convert file ID to GID (for storage in chunk)
  idToGid(id: number): number {
    return this.firstGid + id;
  }

  // Convert GID back to file ID
  gidToId(gid: number): number {
    return gid - this.firstGid;
  }

  getFirstGid(): number {
    return this.firstGid;
  }

  // Load wall sprites (separate from objects)
  async loadWalls(config: ObjectsConfig): Promise<void> {
    this.wallsFirstGid = config.firstGid;

    const manifest = await this.loadManifest();
    const animData = await this.loadAnimatedSprites();
    const atlasInfo = manifest.walls_atlas;
    const atlasImage = await this.loadImage(`/assets/${atlasInfo.file}`);

    for (const item of config.items) {
      const spriteData = atlasInfo.sprites[String(item.id)];
      const anim = animData.walls[String(item.id)];
      const obj: ObjectDefinition = {
        ...item,
        image: spriteData ? atlasImage : undefined,
        atlasRect: spriteData ? {
          x: spriteData.x,
          y: spriteData.y,
          w: anim ? spriteData.w / anim.frames : spriteData.w,
          h: spriteData.h,
        } : undefined,
        frames: anim?.frames,
        fps: anim?.fps,
      };
      this.walls.set(item.id, obj);
    }
  }

  getWall(id: number): ObjectDefinition | undefined {
    return this.walls.get(id);
  }

  getAllWalls(): ObjectDefinition[] {
    return Array.from(this.walls.values());
  }

  getWallsWithImages(): ObjectDefinition[] {
    return this.getAllWalls().filter((obj) => obj.image !== undefined);
  }

  // Convert wall file ID to GID
  wallIdToGid(id: number): number {
    return this.wallsFirstGid + id;
  }

  // Convert wall GID back to file ID
  wallGidToId(gid: number): number {
    return gid - this.wallsFirstGid;
  }

  // Get wall by GID
  getWallByGid(gid: number): ObjectDefinition | undefined {
    const id = this.wallGidToId(gid);
    return this.walls.get(id);
  }

  // Get object OR wall by GID (checks both)
  getObjectByGid(gid: number): ObjectDefinition | undefined {
    // Check walls first (wall GIDs are typically lower)
    if (gid >= this.wallsFirstGid && gid < this.wallsFirstGid + 10000) {
      const wall = this.getWallByGid(gid);
      if (wall) return wall;
    }
    // Fall back to objects
    const id = this.gidToId(gid);
    return this.objects.get(id);
  }

  getWallsFirstGid(): number {
    return this.wallsFirstGid;
  }
}

// Singleton instance
export const objectLoader = new ObjectLoader();
