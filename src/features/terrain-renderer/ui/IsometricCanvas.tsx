import type React from "react";
import { useCallback, useEffect, useRef } from "react";
import { getTileOpacity, TileType, type WorldData } from "@/entities/tile";
import {
	TILE_DEPTH,
	TILE_HEIGHT,
	TILE_WIDTH,
	toScreenX,
	toScreenY,
} from "../lib/isometric";
import { getTileFaces, getMoistureTileFaces } from "../lib/tile-palette";
import { useCamera } from "./use-camera";

interface IsometricCanvasProps {
	world: WorldData;
	width: number;
	height: number;
}

const WATER_FACES = {
	top: "rgba(64, 164, 223, 1)",
	left: "rgba(48, 130, 190, 1)",
	right: "rgba(38, 110, 165, 1)",
};

const drawWaterTile = (
	ctx: CanvasRenderingContext2D,
	sx: number,
	sy: number,
	alpha: number,
): void => {
	const hw = TILE_WIDTH / 2;
	const hh = TILE_HEIGHT / 2;

	ctx.globalAlpha = alpha;

	ctx.fillStyle = WATER_FACES.top;
	ctx.beginPath();
	ctx.moveTo(sx, sy - hh);
	ctx.lineTo(sx + hw, sy);
	ctx.lineTo(sx, sy + hh);
	ctx.lineTo(sx - hw, sy);
	ctx.closePath();
	ctx.fill();

	ctx.fillStyle = WATER_FACES.left;
	ctx.beginPath();
	ctx.moveTo(sx - hw, sy);
	ctx.lineTo(sx, sy + hh);
	ctx.lineTo(sx, sy + hh + TILE_DEPTH);
	ctx.lineTo(sx - hw, sy + TILE_DEPTH);
	ctx.closePath();
	ctx.fill();

	ctx.fillStyle = WATER_FACES.right;
	ctx.beginPath();
	ctx.moveTo(sx + hw, sy);
	ctx.lineTo(sx, sy + hh);
	ctx.lineTo(sx, sy + hh + TILE_DEPTH);
	ctx.lineTo(sx + hw, sy + TILE_DEPTH);
	ctx.closePath();
	ctx.fill();

	ctx.globalAlpha = 1.0;
};

const drawTileWithFaces = (
	ctx: CanvasRenderingContext2D,
	sx: number,
	sy: number,
	faces: { top: string; left: string; right: string },
	alpha: number,
): void => {
	const hw = TILE_WIDTH / 2;
	const hh = TILE_HEIGHT / 2;

	ctx.globalAlpha = alpha;

	ctx.fillStyle = faces.top;
	ctx.beginPath();
	ctx.moveTo(sx, sy - hh);
	ctx.lineTo(sx + hw, sy);
	ctx.lineTo(sx, sy + hh);
	ctx.lineTo(sx - hw, sy);
	ctx.closePath();
	ctx.fill();

	ctx.fillStyle = faces.left;
	ctx.beginPath();
	ctx.moveTo(sx - hw, sy);
	ctx.lineTo(sx, sy + hh);
	ctx.lineTo(sx, sy + hh + TILE_DEPTH);
	ctx.lineTo(sx - hw, sy + TILE_DEPTH);
	ctx.closePath();
	ctx.fill();

	ctx.fillStyle = faces.right;
	ctx.beginPath();
	ctx.moveTo(sx + hw, sy);
	ctx.lineTo(sx, sy + hh);
	ctx.lineTo(sx, sy + hh + TILE_DEPTH);
	ctx.lineTo(sx + hw, sy + TILE_DEPTH);
	ctx.closePath();
	ctx.fill();

	ctx.globalAlpha = 1.0;
};

export const IsometricCanvas: React.FC<IsometricCanvasProps> = ({
	world,
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

		const w = world.width;
		const d = world.depth;

		const getOpacityCutZ = (cx: number, cy: number): number => {
			const top = world.getTopZ(cx, cy);
			let accum = 0;
			for (let z = top; z >= 0; z--) {
				const tt = world.getTile(cx, cy, z);
				const waterMass = world.getWaterMass(cx, cy, z);
				if (tt === TileType.Air && waterMass === 0) continue;
				accum += getTileOpacity(tt);
				if (waterMass > 0) accum += (waterMass / 255) * 0.3;
				if (accum >= 1.0) {
					for (let zz = z - 1; zz >= 0; zz--) {
						if (
							world.getTile(cx, cy, zz) !== TileType.Air ||
							world.getWaterMass(cx, cy, zz) > 0
						) {
							return zz;
						}
					}
					return z;
				}
			}
			return 0;
		};

		for (let y = 0; y < d; y++) {
			for (let x = 0; x < w; x++) {
				const topZ = world.getTopZ(x, y);

				// Neighbor opacity cutoff (not raw topZ) for side face visibility
				const rightCutZ = x + 1 < w ? getOpacityCutZ(x + 1, y) : 0;
				const frontCutZ = y + 1 < d ? getOpacityCutZ(x, y + 1) : 0;
				const neighborMinZ = Math.min(rightCutZ, frontCutZ, topZ);

				// Own column opacity cutoff
				const opacityCutZ = getOpacityCutZ(x, y);

				const renderFromZ = Math.min(opacityCutZ, neighborMinZ);

				// Draw bottom-to-top so upper tiles paint over lower ones
				for (let z = renderFromZ; z <= topZ; z++) {
					const tileType = world.getTile(x, y, z);

					const sx = toScreenX(x, y);
					const sy = toScreenY(x, y, z);

					if (tileType !== TileType.Air) {
						const moisture = world.getSoilMoisture(x, y, z);
						const faces =
							moisture > 0
								? getMoistureTileFaces(tileType, moisture)
								: getTileFaces(tileType);
						if (faces) {
							drawTileWithFaces(
								ctx,
								sx,
								sy,
								faces,
								getTileOpacity(tileType),
							);
						}
					}

					// Draw water overlay
					const waterMass = world.getWaterMass(x, y, z);
					if (waterMass > 0) {
						const waterAlpha = 0.3 + (waterMass / 255) * 0.4;
						drawWaterTile(ctx, sx, sy, waterAlpha);
					}
				}
			}
		}

		// Draw clouds above terrain
		const cloudZ = world.height * 0.7;
		for (const cloud of world.clouds) {
			const cloudSx = toScreenX(cloud.x, cloud.y);
			const cloudSy = toScreenY(cloud.x, cloud.y, cloudZ);
			const radiusPx = cloud.radius * TILE_WIDTH;
			const radiusPy = cloud.radius * TILE_HEIGHT * 0.5;

			ctx.fillStyle = "rgba(180, 180, 200, 0.35)";
			ctx.beginPath();
			ctx.ellipse(
				cloudSx,
				cloudSy,
				radiusPx,
				radiusPy,
				0,
				0,
				Math.PI * 2,
			);
			ctx.fill();
		}

		ctx.restore();
	}, [world]);

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
