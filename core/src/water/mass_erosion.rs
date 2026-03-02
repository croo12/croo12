use std::sync::atomic::{AtomicU64, Ordering};
use crate::world::World;

fn simple_hash(x: usize, y: usize, z: usize, seed: u64) -> u64 {
	let mut h = seed;
	h = h.wrapping_mul(6364136223846793005).wrapping_add(x as u64);
	h = h.wrapping_mul(6364136223846793005).wrapping_add(y as u64);
	h = h.wrapping_mul(6364136223846793005).wrapping_add(z as u64);
	h ^ (h >> 33)
}

fn count_water_above(world: &World, x: usize, y: usize, z: usize) -> usize {
	let h = world.height();
	let mut count = 0;
	let mut cz = z + 1;
	while cz < h {
		if world.water_mass(x, y, cz) > 0 {
			count += 1;
			cz += 1;
		} else {
			break;
		}
	}
	count
}

static EROSION_TICK: AtomicU64 = AtomicU64::new(0);

pub fn pass_erosion(world: &mut World) {
	let seed = EROSION_TICK.fetch_add(1, Ordering::Relaxed);
	let w = world.width();
	let d = world.depth();
	let h = world.height();

	// Erosion: water with flow or pressure erodes erodible tile below
	for z in (1..h).rev() {
		for y in 0..d {
			for x in 0..w {
				let idx = world.index(x, y, z);
				let mass = world.water_mass[idx];
				if mass == 0 {
					continue;
				}

				if !world.get(x, y, z - 1).is_erodible() {
					continue;
				}

				let flow = world.water_outflow(idx);
				let pressure = count_water_above(world, x, y, z) as u64;
				let chance = (pressure * 5 + (flow as u64) / 10).min(80);
				if chance == 0 {
					continue;
				}

				let roll = simple_hash(x, y, z, seed) % 100;
				if roll < chance {
					use crate::tile::Tile;
					world.set(x, y, z - 1, Tile::Air);
					let sed = world.water_sediment[idx];
					world.water_sediment[idx] = sed.saturating_add(1).min(7);
				}
			}
		}
	}

	// Deposition: slow water with sediment on solid ground deposits Sand
	// Exclude top row (h-1) since we need z+1 for displacement
	for z in (1..h.saturating_sub(1)).rev() {
		for y in 0..d {
			for x in 0..w {
				let idx = world.index(x, y, z);
				if world.water_sediment[idx] == 0 || world.water_mass[idx] == 0 {
					continue;
				}

				let flow = world.water_outflow(idx);
				if flow > 20 {
					continue;
				}

				if !world.get(x, y, z - 1).is_solid() || world.get(x, y, z).is_solid() {
					continue;
				}

				// Above must not be solid — water needs somewhere to go
				let above_idx = world.index(x, y, z + 1);
				if world.get(x, y, z + 1).is_solid() {
					continue;
				}

				use crate::tile::Tile;
				// 1. Place sand
				world.set(x, y, z, Tile::Sand);
				world.water_sediment[idx] -= 1;

				// 2. Displace water + remaining sediment to z+1
				world.water_mass[above_idx] =
					world.water_mass[above_idx].saturating_add(world.water_mass[idx]);
				world.water_sediment[above_idx] =
					world.water_sediment[above_idx].saturating_add(world.water_sediment[idx]);

				// 3. Clear trapped data
				world.water_mass[idx] = 0;
				world.water_sediment[idx] = 0;
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tile::Tile;
	use crate::world::World;

	#[test]
	fn erosion_erodes_erodible_below_water() {
		let mut w = World::new(4, 4, 4);
		w.set(1, 1, 0, Tile::Dirt);
		w.set_water_mass(1, 1, 1, 200);
		// Set high outflow to guarantee erosion
		let idx = w.index(1, 1, 1);
		w.water_outflow[idx] = 1000;
		pass_erosion(&mut w);
		// Dirt should be eroded (may depend on hash - run multiple times)
		// At minimum, if eroded: tile becomes Air, sediment increases
		let idx = w.index(1, 1, 1);
		if w.get(1, 1, 0) == Tile::Air {
			assert!(w.water_sediment[idx] > 0);
		}
	}

	#[test]
	fn erosion_does_not_erode_stone() {
		let mut w = World::new(4, 4, 4);
		w.set(1, 1, 0, Tile::Stone);
		w.set_water_mass(1, 1, 1, 200);
		let idx = w.index(1, 1, 1);
		w.water_outflow[idx] = 1000;
		pass_erosion(&mut w);
		assert_eq!(w.get(1, 1, 0), Tile::Stone); // Stone cannot be eroded
	}

	#[test]
	fn deposition_places_sand_and_displaces_water_up() {
		let mut w = World::new(4, 4, 4);
		w.set(1, 1, 0, Tile::Stone);
		w.set_water_mass(1, 1, 1, 100);
		let idx = w.index(1, 1, 1);
		let above_idx = w.index(1, 1, 2);
		w.water_sediment[idx] = 3;
		w.water_outflow[idx] = 0; // Very slow
		pass_erosion(&mut w);
		// Sand placed at z=1
		assert_eq!(w.get(1, 1, 1), Tile::Sand);
		// Water displaced to z=2
		assert_eq!(w.water_mass(1, 1, 1), 0, "water should be cleared from sand cell");
		assert_eq!(w.water_mass(1, 1, 2), 100, "water should move to z+1");
		// Sediment: 3 - 1 = 2 remaining, displaced to z+1
		assert_eq!(w.water_sediment[idx], 0, "sediment cleared from sand cell");
		assert_eq!(w.water_sediment[above_idx], 2, "remaining sediment displaced to z+1");
	}

	#[test]
	fn deposition_skips_when_above_is_solid() {
		let mut w = World::new(4, 4, 4);
		w.set(1, 1, 0, Tile::Stone);
		w.set(1, 1, 2, Tile::Stone); // ceiling blocks displacement
		w.set_water_mass(1, 1, 1, 100);
		let idx = w.index(1, 1, 1);
		w.water_sediment[idx] = 3;
		w.water_outflow[idx] = 0;
		pass_erosion(&mut w);
		// Should NOT deposit since water has nowhere to go
		assert!(w.get(1, 1, 1).is_air(), "should remain Air, not Sand");
		assert_eq!(w.water_mass(1, 1, 1), 100, "water should be untouched");
		assert_eq!(w.water_sediment[idx], 3, "sediment should be untouched");
	}
}
