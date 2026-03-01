import type { TileTypeValue } from "./tile-type";
import { TileType } from "./tile-type";

const TYPE_SHIFT = 12;
const TYPE_MASK = 0x0f;
const LEVEL_SHIFT = 8;
const LEVEL_MASK = 0x0f;

export class WorldData {
	readonly width: number;
	readonly depth: number;
	readonly height: number;
	private readonly tiles: Uint16Array;

	constructor(
		width: number,
		depth: number,
		height: number,
		tiles: Uint16Array,
	) {
		this.width = width;
		this.depth = depth;
		this.height = height;
		this.tiles = new Uint16Array(tiles);
	}

	getTile(x: number, y: number, z: number): TileTypeValue {
		const packed =
			this.tiles[x + y * this.width + z * this.width * this.depth];
		return ((packed >> TYPE_SHIFT) & TYPE_MASK) as TileTypeValue;
	}

	getTileLevel(x: number, y: number, z: number): number {
		const packed =
			this.tiles[x + y * this.width + z * this.width * this.depth];
		return (packed >> LEVEL_SHIFT) & LEVEL_MASK;
	}

	getTopZ(x: number, y: number): number {
		for (let z = this.height - 1; z >= 0; z--) {
			if (this.getTile(x, y, z) !== TileType.Air) {
				return z;
			}
		}
		return 0;
	}
}
