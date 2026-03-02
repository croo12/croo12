use noise::{Fbm, MultiFractal, NoiseFn, Perlin, RidgedMulti};

use crate::tile::Tile;
use crate::world::World;

const WATER_LEVEL: usize = 32;
const SEA_FLOOR: usize = 16;
const OCEAN_THRESHOLD: f64 = 0.38;
const CONTINENT_FREQ: f64 = 0.015;
const MOUNTAIN_FREQ: f64 = 0.08;
const DETAIL_FREQ: f64 = 0.15;
const BEACH_DEPTH: usize = 3;
const RIVER_SOURCE_COUNT: usize = 3;
const DIRT_LAYERS: usize = 3;

pub fn generate_terrain(world: &mut World, seed: u32) {
	let w = world.width();
	let d = world.depth();
	let h = world.height();

	let continent = Fbm::<Perlin>::new(seed)
		.set_octaves(2)
		.set_frequency(CONTINENT_FREQ);

	let mountain = RidgedMulti::<Perlin>::new(seed.wrapping_add(1))
		.set_octaves(4)
		.set_frequency(MOUNTAIN_FREQ);

	let detail = Fbm::<Perlin>::new(seed.wrapping_add(2))
		.set_octaves(4)
		.set_frequency(DETAIL_FREQ);

	// Generate surface height map
	let mut surface_heights = vec![0usize; w * d];
	for y in 0..d {
		for x in 0..w {
			let continent_raw = continent.get([x as f64, y as f64]);
			let mountain_n = (mountain.get([x as f64, y as f64]) + 1.0) / 2.0;
			let detail_n = (detail.get([x as f64, y as f64]) + 1.0) / 2.0;
			let continent_n = (continent_raw + 1.0) / 2.0;

			let surface = if continent_n < OCEAN_THRESHOLD {
				// Ocean: SEA_FLOOR ~ WATER_LEVEL
				let depth_factor = continent_n / OCEAN_THRESHOLD;
				let base = SEA_FLOOR as f64
					+ depth_factor * (WATER_LEVEL - SEA_FLOOR) as f64
					+ (detail_n - 0.5) * 3.0;
				base as usize
			} else {
				// Land
				let land_factor = (continent_n - OCEAN_THRESHOLD) / (1.0 - OCEAN_THRESHOLD);
				let base = WATER_LEVEL as f64 + 2.0 + land_factor * 30.0;
				// Powf(1.5) makes mountains spike instead of rolling smoothly.
				let mountain_contrib =
					mountain_n.powf(1.5) * land_factor * 80.0;
				let detail_contrib = (detail_n - 0.5) * 15.0;
				let raw = base + mountain_contrib + detail_contrib;
				raw.clamp((WATER_LEVEL + 1) as f64, (h - 2) as f64) as usize
			};

			surface_heights[x + y * w] = surface.min(h - 1);
		}
	}

	// Safety: if all surfaces are at or below WATER_LEVEL, boost heights
	let max_surface = surface_heights.iter().copied().max().unwrap_or(0);
	if max_surface <= WATER_LEVEL {
		let boost = WATER_LEVEL + 5 - max_surface;
		for s in surface_heights.iter_mut() {
			*s = (*s + boost).min(h - 2);
		}
	}

	// Fill terrain column by column
	for y in 0..d {
		for x in 0..w {
			let surface = surface_heights[x + y * w];
			for z in 0..h {
				let tile = if z > surface {
					Tile::Air
				} else if z == surface {
					if surface <= WATER_LEVEL + BEACH_DEPTH {
						Tile::Sand
					} else {
						Tile::Grass
					}
				} else if z > surface.saturating_sub(DIRT_LAYERS) {
					Tile::Dirt
				} else {
					Tile::Stone
				};
				world.set(x, y, z, tile);
			}
		}
	}

	// Fill ocean: set water in cells where surface < WATER_LEVEL
	for y in 0..d {
		for x in 0..w {
			let surface = surface_heights[x + y * w];
			if surface < WATER_LEVEL {
				for z in (surface + 1)..=WATER_LEVEL {
					if z < h {
						world.set_water_mass(x, y, z, 255);
					}
				}
			}
		}
	}

	// Place water sources at high points
	let mut candidates: Vec<(usize, usize, usize)> = Vec::new();
	for y in 0..d {
		for x in 0..w {
			let s = surface_heights[x + y * w];
			if s > WATER_LEVEL {
				candidates.push((x, y, s + 1));
			}
		}
	}

	// Sort by height descending, pick top N
	candidates.sort_by(|a, b| b.2.cmp(&a.2));

	let count = RIVER_SOURCE_COUNT.min(candidates.len());
	// Deterministic selection using seed
	let step = if candidates.len() > count {
		candidates.len() / count
	} else {
		1
	};
	for i in 0..count {
		let idx = (i * step + seed as usize) % candidates.len();
		let (sx, sy, sz) = candidates[idx];
		if sz < h {
			world.set_water_mass(sx, sy, sz, 255);
			world.add_source(sx, sy, sz);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tile::Tile;
	use crate::world::World;

	#[test]
	fn terrain_fills_ground() {
		let mut world = World::new(8, 8, 128);
		generate_terrain(&mut world, 42);
		// Bottom should be Stone
		assert_eq!(world.get(0, 0, 0), Tile::Stone);
	}

	#[test]
	fn terrain_has_air_at_top() {
		let mut world = World::new(8, 8, 128);
		generate_terrain(&mut world, 42);
		assert_eq!(world.get(0, 0, 127), Tile::Air);
	}

	#[test]
	fn terrain_has_grass_surface() {
		let mut world = World::new(8, 8, 128);
		generate_terrain(&mut world, 42);
		let mut found_grass = false;
		for x in 0..8 {
			for y in 0..8 {
				for z in 0..128 {
					if world.get(x, y, z) == Tile::Grass {
						found_grass = true;
					}
				}
			}
		}
		assert!(found_grass);
	}

	#[test]
	fn terrain_has_water_sources() {
		let mut world = World::new(16, 16, 128);
		generate_terrain(&mut world, 42);
		assert!(!world.sources().is_empty());
	}

	#[test]
	fn terrain_is_deterministic() {
		let mut w1 = World::new(8, 8, 128);
		let mut w2 = World::new(8, 8, 128);
		generate_terrain(&mut w1, 99);
		generate_terrain(&mut w2, 99);
		for i in 0..(8 * 8 * 128) {
			assert_eq!(w1.tiles()[i], w2.tiles()[i]);
		}
	}
}
