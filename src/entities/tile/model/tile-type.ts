export const TileType = {
	Air: 0,
	Grass: 1,
	Dirt: 2,
	Stone: 3,
	Sand: 4,
} as const;

export type TileTypeValue = (typeof TileType)[keyof typeof TileType];

const OPACITY: Record<TileTypeValue, number> = {
	[TileType.Air]: 0.0,
	[TileType.Grass]: 1.0,
	[TileType.Dirt]: 1.0,
	[TileType.Stone]: 1.0,
	[TileType.Sand]: 1.0,
};

export const getTileOpacity = (tileType: TileTypeValue): number =>
	OPACITY[tileType] ?? 0.0;
