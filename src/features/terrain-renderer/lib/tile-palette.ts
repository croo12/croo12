import { TileType } from "@/entities/tile";

interface TileFaces {
	top: string;
	left: string;
	right: string;
}

const palette: Record<number, TileFaces> = {
	[TileType.Grass]: {
		top: "#7ec850",
		left: "#5a9a3c",
		right: "#4a8032",
	},
	[TileType.Dirt]: {
		top: "#b87840",
		left: "#9a6030",
		right: "#805028",
	},
	[TileType.Stone]: {
		top: "#a0a0a0",
		left: "#808080",
		right: "#6a6a6a",
	},
	[TileType.Sand]: {
		top: "#e8d678",
		left: "#c8b860",
		right: "#b0a050",
	},
};

export const getTileFaces = (tileType: number): TileFaces | undefined =>
	palette[tileType];

const MOISTURE_CAPACITIES: Record<number, number> = {
	[TileType.Sand]: 48,
	[TileType.Grass]: 160,
	[TileType.Dirt]: 128,
};

const hexToRgb = (hex: string): [number, number, number] => {
	const r = parseInt(hex.slice(1, 3), 16);
	const g = parseInt(hex.slice(3, 5), 16);
	const b = parseInt(hex.slice(5, 7), 16);
	return [r, g, b];
};

const darkenHex = (hex: string, factor: number): string => {
	const [r, g, b] = hexToRgb(hex);
	const nr = Math.round(r * factor);
	const ng = Math.round(g * factor);
	const nb = Math.round(b * factor);
	return `rgb(${nr},${ng},${nb})`;
};

export const getMoistureTileFaces = (
	tileType: number,
	moisture: number,
): TileFaces | undefined => {
	const base = palette[tileType];
	if (!base) return undefined;

	const capacity = MOISTURE_CAPACITIES[tileType] ?? 0;
	if (capacity === 0 || moisture === 0) return base;

	const ratio = Math.min(moisture / capacity, 1.0);
	const darkening = 1 - ratio * 0.35;

	return {
		top: darkenHex(base.top, darkening),
		left: darkenHex(base.left, darkening),
		right: darkenHex(base.right, darkening),
	};
};
