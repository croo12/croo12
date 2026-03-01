import { useQuery } from "@tanstack/react-query";
import type React from "react";
import { useEffect, useMemo } from "react";
import initGameCore, {
	create_world,
	tick_water,
	water_levels_len,
	water_levels_ptr,
	world_depth,
	world_height,
	world_tiles_len,
	world_tiles_ptr,
	world_width,
} from "../../../../core/build/game_core";
import { WorldData } from "@/entities/tile";
import { WaterData } from "@/entities/water";
import { IsometricCanvas } from "@/features/terrain-renderer";
import { createWasmLoader } from "@/shared/wasm";
import { colors, effects, layout, spacing } from "@/shared/theme";
import { Body, Title } from "@/shared/ui";

const CANVAS_WIDTH = 800;
const CANVAS_HEIGHT = 600;
const WORLD_SIZE = 64;
const WORLD_HEIGHT = 32;
const SEED = 42;
const TICK_INTERVAL_MS = 200;

const gameCoreQueryOptions = createWasmLoader("game-core", initGameCore);

export const GamePage: React.FC = () => {
	const { data: wasmOutput, isSuccess } = useQuery(gameCoreQueryOptions);

	const world = useMemo(() => {
		if (!isSuccess || !wasmOutput) return null;

		create_world(WORLD_SIZE, WORLD_SIZE, WORLD_HEIGHT, SEED);

		const ptr = world_tiles_ptr();
		const len = world_tiles_len();
		const w = world_width();
		const d = world_depth();
		const h = world_height();

		const tiles = new Uint16Array(wasmOutput.memory.buffer, ptr, len);
		return new WorldData(w, d, h, tiles);
	}, [isSuccess, wasmOutput]);

	const waterData = useMemo(() => {
		if (!isSuccess || !wasmOutput || !world) return null;
		const ptr = water_levels_ptr();
		const len = water_levels_len();
		const levels = new Uint8Array(wasmOutput.memory.buffer, ptr, len);
		return new WaterData(world.width, world.depth, world.height, levels);
	}, [isSuccess, wasmOutput, world]);

	useEffect(() => {
		if (!wasmOutput || !waterData || !world) return;
		const interval = setInterval(() => {
			tick_water();
			const wPtr = water_levels_ptr();
			const wLen = water_levels_len();
			const levels = new Uint8Array(wasmOutput.memory.buffer, wPtr, wLen);
			waterData.updateLevels(levels);
			const tPtr = world_tiles_ptr();
			const tLen = world_tiles_len();
			const tiles = new Uint16Array(wasmOutput.memory.buffer, tPtr, tLen);
			world.updateTiles(tiles);
		}, TICK_INTERVAL_MS);
		return () => clearInterval(interval);
	}, [wasmOutput, waterData, world]);

	return (
		<div
			style={{
				display: "flex",
				flexDirection: "column",
				alignItems: "center",
			}}
		>
			<div
				id="game-container"
				style={{
					width: `${CANVAS_WIDTH}px`,
					height: `${CANVAS_HEIGHT}px`,
					backgroundColor: "#1a1a2e",
					border: `2px solid ${colors.border}`,
					borderRadius: layout.radius,
					display: "flex",
					alignItems: "center",
					justifyContent: "center",
					boxShadow: effects.shadowElevated,
					marginBottom: spacing.md,
				}}
			>
				{world ? (
					<IsometricCanvas
						world={world}
						waterData={waterData}
						width={CANVAS_WIDTH}
						height={CANVAS_HEIGHT}
					/>
				) : (
					<Body>Loading terrain...</Body>
				)}
			</div>

			<div
				className="controls"
				style={{
					padding: spacing.md,
					background: colors.bgElevated,
					borderRadius: layout.radius,
					width: `${CANVAS_WIDTH}px`,
					boxSizing: "border-box",
					textAlign: "center",
				}}
			>
				<Title>Isometric Terrain Sandbox</Title>
				<Body>WASD / Arrow keys to pan, mouse wheel to zoom.</Body>
			</div>
		</div>
	);
};
