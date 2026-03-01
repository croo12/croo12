export const TileType = {
	Air: 0,
	Grass: 1,
	Dirt: 2,
	Stone: 3,
	Sand: 4,
	Water: 5,
} as const;

export type TileTypeValue = (typeof TileType)[keyof typeof TileType];

export const FlowDir = {
	None: 0,
	Down: 1,
	North: 2,
	South: 3,
	East: 4,
	West: 5,
} as const;

export type FlowDirValue = (typeof FlowDir)[keyof typeof FlowDir];

const OPACITY: Record<TileTypeValue, number> = {
	[TileType.Air]: 0.0,
	[TileType.Grass]: 1.0,
	[TileType.Dirt]: 1.0,
	[TileType.Stone]: 1.0,
	[TileType.Sand]: 1.0,
	[TileType.Water]: 0.3,
};

export const getTileOpacity = (tileType: TileTypeValue): number =>
	OPACITY[tileType] ?? 0.0;
