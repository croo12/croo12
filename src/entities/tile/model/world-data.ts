import type { TileTypeValue } from "./tile-type";
import { TileType } from "./tile-type";

const TYPE_MASK = 0x07;

export interface CloudData {
	x: number;
	y: number;
	radius: number;
	water: number;
}

export class WorldData {
	readonly width: number;
	readonly depth: number;
	readonly height: number;
	private tiles: Uint8Array;
	private water: Uint8Array;
	private moisture: Uint8Array;
	private _clouds: CloudData[] = [];
	private _atmosphericMoisture = 0;

	constructor(
		width: number,
		depth: number,
		height: number,
		tiles: Uint8Array,
		water: Uint8Array,
		moisture: Uint8Array,
	) {
		this.width = width;
		this.depth = depth;
		this.height = height;
		this.tiles = new Uint8Array(tiles);
		this.water = new Uint8Array(water);
		this.moisture = new Uint8Array(moisture);
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

	get clouds(): readonly CloudData[] {
		return this._clouds;
	}

	get atmosphericMoisture(): number {
		return this._atmosphericMoisture;
	}

	getSoilMoisture(x: number, y: number, z: number): number {
		return this.moisture[this.index(x, y, z)];
	}

	updateTiles(
		tiles: Uint8Array,
		water: Uint8Array,
		moisture: Uint8Array,
	): void {
		this.tiles = new Uint8Array(tiles);
		this.water = new Uint8Array(water);
		this.moisture = new Uint8Array(moisture);
	}

	updateClouds(
		cloudBuffer: Float32Array,
		count: number,
		moisture: number,
	): void {
		this._clouds = [];
		for (let i = 0; i < count; i++) {
			const offset = i * 4;
			this._clouds.push({
				x: cloudBuffer[offset],
				y: cloudBuffer[offset + 1],
				radius: cloudBuffer[offset + 2],
				water: cloudBuffer[offset + 3],
			});
		}
		this._atmosphericMoisture = moisture;
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
