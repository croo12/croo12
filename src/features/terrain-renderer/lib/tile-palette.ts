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
