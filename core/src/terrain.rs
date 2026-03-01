use noise::{Fbm, MultiFractal, NoiseFn, Perlin};

use crate::tile::Tile;
use crate::world::World;

const WATER_LEVEL: usize = 32;
const SEA_FLOOR: usize = 16;
const NOISE_SCALE: f64 = 0.03;
const RIVER_SOURCE_COUNT: usize = 3;
const DIRT_LAYERS: usize = 3;

pub fn generate_terrain(world: &mut World, seed: u32) {
	let w = world.width();
	let d = world.depth();
	let h = world.height();

	let fbm = Fbm::<Perlin>::new(seed)
		.set_octaves(4)
		.set_frequency(NOISE_SCALE);

	// Generate surface height map
	let mut surface_heights = vec![0usize; w * d];
	for y in 0..d {
		for x in 0..w {
			let val = fbm.get([x as f64, y as f64]);
			let normalized = (val + 1.0) / 2.0; // 0.0..1.0
			let surface = SEA_FLOOR + (normalized * (h - SEA_FLOOR) as f64 * 0.6) as usize;
			surface_heights[x + y * w] = surface.min(h - 1);
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
					if surface <= WATER_LEVEL {
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
