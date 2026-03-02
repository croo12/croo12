import { useQuery } from "@tanstack/react-query";
import type React from "react";
import { useEffect, useMemo, useState } from "react";
import { WorldData } from "@/entities/tile";
import { IsometricCanvas } from "@/features/terrain-renderer";
import { colors, effects, layout, spacing } from "@/shared/theme";
import { Body, Title } from "@/shared/ui";
import { createWasmLoader } from "@/shared/wasm";
import initGameCore, {
	create_world,
	tick_water,
	world_depth,
	world_height,
	world_tiles_len,
	world_tiles_ptr,
	world_water_len,
	world_water_ptr,
	world_width,
	world_clouds_ptr,
	world_clouds_len,
	world_clouds_count,
	world_atmospheric_moisture,
	world_moisture_ptr,
	world_moisture_len,
} from "../../../../core/build/game_core";

const CANVAS_WIDTH = 800;
const CANVAS_HEIGHT = 600;
const WORLD_SIZE = 64;
const WORLD_HEIGHT = 128;
const DEFAULT_SEED = 77;
const TICK_INTERVAL_MS = 200;

const gameCoreQueryOptions = createWasmLoader("game-core", initGameCore);

export const GamePage: React.FC = () => {
	const { data: wasmOutput, isSuccess } = useQuery(gameCoreQueryOptions);
	const [seed, setSeed] = useState(DEFAULT_SEED);
	const [inputSeed, setInputSeed] = useState(String(DEFAULT_SEED));

	const world = useMemo(() => {
		if (!isSuccess || !wasmOutput) return null;

		create_world(WORLD_SIZE, WORLD_SIZE, WORLD_HEIGHT, seed);

		const ptr = world_tiles_ptr();
		const len = world_tiles_len();
		const w = world_width();
		const d = world_depth();
		const h = world_height();

		const tiles = new Uint8Array(wasmOutput.memory.buffer, ptr, len);
		const waterPtr = world_water_ptr();
		const waterLen = world_water_len();
		const water = new Uint8Array(wasmOutput.memory.buffer, waterPtr, waterLen);
		const moisturePtr = world_moisture_ptr();
		const moistureLen = world_moisture_len();
		const moisture = new Uint8Array(
			wasmOutput.memory.buffer,
			moisturePtr,
			moistureLen,
		);
		return new WorldData(w, d, h, tiles, water, moisture);
	}, [isSuccess, wasmOutput, seed]);

	useEffect(() => {
		if (!wasmOutput || !world) return;
		const interval = setInterval(() => {
			tick_water();
			const ptr = world_tiles_ptr();
			const len = world_tiles_len();
			const tiles = new Uint8Array(wasmOutput.memory.buffer, ptr, len);
			const waterPtr = world_water_ptr();
			const waterLen = world_water_len();
			const water = new Uint8Array(
				wasmOutput.memory.buffer,
				waterPtr,
				waterLen,
			);
			const moisturePtr = world_moisture_ptr();
			const moistureLen = world_moisture_len();
			const moisture = new Uint8Array(
				wasmOutput.memory.buffer,
				moisturePtr,
				moistureLen,
			);
			world.updateTiles(tiles, water, moisture);

			const cloudsCount = world_clouds_count();
			const cloudsPtr = world_clouds_ptr();
			const cloudsLen = world_clouds_len();
			const cloudBuffer = new Float32Array(
				wasmOutput.memory.buffer,
				cloudsPtr,
				cloudsLen,
			);
			const atmosMoisture = world_atmospheric_moisture();
			world.updateClouds(cloudBuffer, cloudsCount, atmosMoisture);
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
				<div
					style={{
						display: "flex",
						alignItems: "center",
						justifyContent: "center",
						gap: spacing.sm,
						marginBottom: spacing.sm,
					}}
				>
					<Body>Seed:</Body>
					<input
						type="number"
						min={0}
						max={4294967295}
						value={inputSeed}
						onChange={(e) => setInputSeed(e.target.value)}
						onKeyDown={(e) => {
							if (e.key === "Enter") {
								const value = Number(inputSeed);
								if (!Number.isNaN(value) && value >= 0 && value <= 4294967295) {
									setSeed(value);
								}
							}
						}}
						style={{
							width: "140px",
							padding: `${spacing.xs} ${spacing.sm}`,
							borderRadius: layout.radius,
							border: `1px solid ${colors.border}`,
							background: colors.bgPrimary,
							color: colors.textPrimary,
							textAlign: "center",
						}}
					/>
					<button
						type="button"
						onClick={() => {
							const value = Number(inputSeed);
							if (!Number.isNaN(value) && value >= 0 && value <= 4294967295) {
								setSeed(value);
							}
						}}
						style={{
							padding: `${spacing.xs} ${spacing.md}`,
							borderRadius: layout.radius,
							border: `1px solid ${colors.border}`,
							background: colors.bgElevated,
							color: colors.textPrimary,
							cursor: "pointer",
						}}
					>
						Generate
					</button>
				</div>
				<Body>WASD / Arrow keys to pan, mouse wheel to zoom.</Body>
			</div>
		</div>
	);
};
