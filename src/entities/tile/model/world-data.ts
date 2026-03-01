import type { TileTypeValue } from "./tile-type";
import { TileType } from "./tile-type";

const TYPE_MASK = 0x07;

export class WorldData {
	readonly width: number;
	readonly depth: number;
	readonly height: number;
	private tiles: Uint8Array;
	private water: Uint8Array;

	constructor(
		width: number,
		depth: number,
		height: number,
		tiles: Uint8Array,
		water: Uint8Array,
	) {
		this.width = width;
		this.depth = depth;
		this.height = height;
		this.tiles = new Uint8Array(tiles);
		this.water = new Uint8Array(water);
	}

	private index(x: number, y: number, z: number): number {
		return x + y * this.width + z * this.width * this.depth;
	}

	getTile(x: number, y: number, z: number): TileTypeValue {
		return (this.tiles[this.index(x, y, z)] & TYPE_MASK) as TileTypeValue;
	}

	getWaterMass(x: number, y: number, z: number): number {
		return this.water[this.index(x, y, z)];
	}

	updateTiles(tiles: Uint8Array, water: Uint8Array): void {
		this.tiles = new Uint8Array(tiles);
		this.water = new Uint8Array(water);
	}

	getTopZ(x: number, y: number): number {
		for (let z = this.height - 1; z >= 0; z--) {
			if (
				this.getTile(x, y, z) !== TileType.Air ||
				this.getWaterMass(x, y, z) > 0
			) {
				return z;
			}
		}
		return 0;
	}
}
