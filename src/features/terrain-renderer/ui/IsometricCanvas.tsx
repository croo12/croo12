import type React from "react";
import { useCallback, useEffect, useRef } from "react";
import { TileType, type WorldData } from "@/entities/tile";
import type { WaterData } from "@/entities/water";
import {
	TILE_DEPTH,
	TILE_HEIGHT,
	TILE_WIDTH,
	toScreenX,
	toScreenY,
} from "../lib/isometric";
import { getTileFaces } from "../lib/tile-palette";
import { useCamera } from "./use-camera";

interface IsometricCanvasProps {
	world: WorldData;
	waterData: WaterData | null;
	width: number;
	height: number;
}

const isOccluded = (
	world: WorldData,
	x: number,
	y: number,
	z: number,
): boolean => {
	const w = world.width;
	const d = world.depth;
	const h = world.height;

	const hasAbove = z + 1 < h && world.getTile(x, y, z + 1) !== TileType.Air;
	const hasFrontLeft =
		y + 1 < d && world.getTile(x, y + 1, z) !== TileType.Air;
	const hasFrontRight =
		x + 1 < w && world.getTile(x + 1, y, z) !== TileType.Air;

	return hasAbove && hasFrontLeft && hasFrontRight;
};

const drawTile = (
	ctx: CanvasRenderingContext2D,
	sx: number,
	sy: number,
	tileType: number,
	level: number,
): void => {
	const faces = getTileFaces(tileType);
	if (!faces) return;

	const hw = TILE_WIDTH / 2;
	const hh = TILE_HEIGHT / 2;
	const tileHeight = (level / 8) * TILE_DEPTH;
	const yOffset = TILE_DEPTH - tileHeight;

	// Top face — shifted down by yOffset
	ctx.fillStyle = faces.top;
	ctx.beginPath();
	ctx.moveTo(sx, sy - hh + yOffset);
	ctx.lineTo(sx + hw, sy + yOffset);
	ctx.lineTo(sx, sy + hh + yOffset);
	ctx.lineTo(sx - hw, sy + yOffset);
	ctx.closePath();
	ctx.fill();

	// Left face — height = tileHeight
	ctx.fillStyle = faces.left;
	ctx.beginPath();
	ctx.moveTo(sx - hw, sy + yOffset);
	ctx.lineTo(sx, sy + hh + yOffset);
	ctx.lineTo(sx, sy + hh + TILE_DEPTH);
	ctx.lineTo(sx - hw, sy + TILE_DEPTH);
	ctx.closePath();
	ctx.fill();

	// Right face — height = tileHeight
	ctx.fillStyle = faces.right;
	ctx.beginPath();
	ctx.moveTo(sx + hw, sy + yOffset);
	ctx.lineTo(sx, sy + hh + yOffset);
	ctx.lineTo(sx, sy + hh + TILE_DEPTH);
	ctx.lineTo(sx + hw, sy + TILE_DEPTH);
	ctx.closePath();
	ctx.fill();
};

export const IsometricCanvas: React.FC<IsometricCanvasProps> = ({
	world,
	waterData,
	width,
	height,
}) => {
	const canvasRef = useRef<HTMLCanvasElement>(null);

	const centerX = world.width / 2;
	const centerY = world.depth / 2;
	const centerZ = world.height * 0.3;
	const initialCamX = toScreenX(centerX, centerY);
	const initialCamY = toScreenY(centerX, centerY, centerZ);

	const { camera, onWheel } = useCamera(initialCamX, initialCamY);
	const cameraRef = useRef(camera);
	cameraRef.current = camera;

	const render = useCallback(() => {
		const canvas = canvasRef.current;
		if (!canvas) return;
		const ctx = canvas.getContext("2d");
		if (!ctx) return;

		const cam = cameraRef.current;
		ctx.clearRect(0, 0, canvas.width, canvas.height);

		ctx.save();
		ctx.translate(canvas.width / 2, canvas.height / 4);
		ctx.scale(cam.zoom, cam.zoom);
		ctx.translate(-cam.x, -cam.y);

		for (let z = 0; z < world.height; z++) {
			for (let y = 0; y < world.depth; y++) {
				for (let x = 0; x < world.width; x++) {
					const tile = world.getTile(x, y, z);
					const waterLevel = waterData?.getLevel(x, y, z) ?? 0;

					if (tile === TileType.Air && waterLevel === 0) continue;
					if (isOccluded(world, x, y, z)) continue;

					const sx = toScreenX(x, y);
					const sy = toScreenY(x, y, z);

					// 고체 타일
					if (tile !== TileType.Air && tile !== TileType.Water) {
						drawTile(ctx, sx, sy, tile, world.getTileLevel(x, y, z));
					}

					// 물 렌더링 (Water 타일이거나 water level이 있는 경우)
					if (waterLevel > 0) {
						drawTile(ctx, sx, sy, TileType.Water, waterLevel);
					}
				}
			}
		}

		ctx.restore();
	}, [world, waterData]);

	useEffect(() => {
		let rafId: number;
		const loop = (): void => {
			render();
			rafId = requestAnimationFrame(loop);
		};
		rafId = requestAnimationFrame(loop);
		return () => cancelAnimationFrame(rafId);
	}, [render]);

	useEffect(() => {
		const canvas = canvasRef.current;
		if (!canvas) return;

		const handleWheel = (e: WheelEvent): void => {
			e.preventDefault();
			onWheel(e);
		};

		canvas.addEventListener("wheel", handleWheel, { passive: false });
		return () => canvas.removeEventListener("wheel", handleWheel);
	}, [onWheel]);

	return (
		<canvas
			ref={canvasRef}
			width={width}
			height={height}
			style={{ display: "block", background: "#1a1a2e" }}
		/>
	);
};
