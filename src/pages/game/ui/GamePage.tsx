import { useQuery } from "@tanstack/react-query";
import type React from "react";
import { useEffect, useMemo } from "react";
import initGameCore, {
	create_world,
	tick_water,
	world_depth,
	world_height,
	world_tiles_len,
	world_tiles_ptr,
	world_width,
} from "../../../../core/build/game_core";
import { WorldData } from "@/entities/tile";
import { IsometricCanvas } from "@/features/terrain-renderer";
import { createWasmLoader } from "@/shared/wasm";
import { colors, effects, layout, spacing } from "@/shared/theme";
import { Body, Title } from "@/shared/ui";

const CANVAS_WIDTH = 800;
const CANVAS_HEIGHT = 600;
const WORLD_SIZE = 64;
const WORLD_HEIGHT = 128;
const SEED = 1;
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

		const tiles = new Uint8Array(wasmOutput.memory.buffer, ptr, len);
		return new WorldData(w, d, h, tiles);
	}, [isSuccess, wasmOutput]);

	useEffect(() => {
		if (!wasmOutput || !world) return;
		const interval = setInterval(() => {
			tick_water();
			const ptr = world_tiles_ptr();
			const len = world_tiles_len();
			const tiles = new Uint8Array(wasmOutput.memory.buffer, ptr, len);
			world.updateTiles(tiles);
		}, TICK_INTERVAL_MS);
		return () => clearInterval(interval);
	}, [wasmOutput, world]);

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
