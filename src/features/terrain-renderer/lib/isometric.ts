export const TILE_WIDTH = 32;
export const TILE_HEIGHT = 16;
export const TILE_DEPTH = 2; // was 8, now 2 for 128 height

export const toScreenX = (x: number, y: number): number =>
	(x - y) * (TILE_WIDTH / 2);

export const toScreenY = (x: number, y: number, z: number): number =>
	(x + y) * (TILE_HEIGHT / 2) - z * TILE_DEPTH;
