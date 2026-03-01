import type { FlowDirValue, TileTypeValue } from "./tile-type";
import { TileType } from "./tile-type";

const TYPE_MASK = 0x07;
const DIR_SHIFT = 3;
const DIR_MASK = 0x07;
const SOURCE_BIT = 1 << 6;

export class WorldData {
	readonly width: number;
	readonly depth: number;
	readonly height: number;
	private tiles: Uint8Array;

	constructor(width: number, depth: number, height: number, tiles: Uint8Array) {
		this.width = width;
		this.depth = depth;
		this.height = height;
		this.tiles = new Uint8Array(tiles);
	}

	private index(x: number, y: number, z: number): number {
		return x + y * this.width + z * this.width * this.depth;
	}

	getTile(x: number, y: number, z: number): TileTypeValue {
		return (this.tiles[this.index(x, y, z)] & TYPE_MASK) as TileTypeValue;
	}

	getFlowDir(x: number, y: number, z: number): FlowDirValue {
		return ((this.tiles[this.index(x, y, z)] >> DIR_SHIFT) &
			DIR_MASK) as FlowDirValue;
	}

	isSource(x: number, y: number, z: number): boolean {
		return (this.tiles[this.index(x, y, z)] & SOURCE_BIT) !== 0;
	}

	updateTiles(tiles: Uint8Array): void {
		this.tiles = new Uint8Array(tiles);
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
