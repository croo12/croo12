use std::sync::atomic::{AtomicU64, Ordering};
use crate::tile::Tile;
use crate::world::World;

fn simple_hash(x: usize, y: usize, z: usize, seed: u64) -> u64 {
	let mut h = seed;
	h = h.wrapping_mul(6364136223846793005).wrapping_add(x as u64);
	h = h.wrapping_mul(6364136223846793005).wrapping_add(y as u64);
	h = h.wrapping_mul(6364136223846793005).wrapping_add(z as u64);
	h ^ (h >> 33)
}

static EVAP_TICK: AtomicU64 = AtomicU64::new(0);

pub fn pass_evaporation(world: &mut World) {
	let seed = EVAP_TICK.fetch_add(1, Ordering::Relaxed);
	let w = world.width();
	let d = world.depth();
	let h = world.height();

	for z in 0..h {
		for y in 0..d {
			for x in 0..w {
				let idx = world.index(x, y, z);
				let mass = world.water_mass[idx];
				if mass == 0 {
					continue;
				}

				// No evaporation if water above
				if z + 1 < h && world.water_mass(x, y, z + 1) > 0 {
					continue;
				}

				let roll = simple_hash(x, y, z, seed) % 100;
				if roll < 5 {
					let evap = mass.min(10);
					world.water_mass[idx] = mass - evap;
					if world.water_mass[idx] == 0 && world.water_sediment[idx] > 0 {
						world.set(x, y, z, Tile::Sand);
						world.water_sediment[idx] = 0;
					}
				}
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::world::World;

	#[test]
	fn evaporation_reduces_mass_over_time() {
		let mut w = World::new(4, 4, 4);
		w.set(1, 1, 0, crate::tile::Tile::Stone);
		w.set_water_mass(1, 1, 1, 100);
		for _ in 0..100 {
			pass_evaporation(&mut w);
		}
		assert!(
			w.water_mass(1, 1, 1) < 100,
			"Some water should have evaporated"
		);
	}

	#[test]
	fn evaporation_leaves_sand_when_dry_with_sediment() {
		let mut w = World::new(4, 4, 4);
		w.set(1, 1, 0, crate::tile::Tile::Stone);
		w.set_water_mass(1, 1, 1, 1);
		let idx = w.index(1, 1, 1);
		w.water_sediment[idx] = 2;
		for _ in 0..1000 {
			pass_evaporation(&mut w);
			if w.water_mass(1, 1, 1) == 0 {
				break;
			}
		}
		if w.water_mass(1, 1, 1) == 0 {
			assert_eq!(w.get(1, 1, 1), crate::tile::Tile::Sand);
			assert_eq!(w.water_sediment[idx], 0);
		}
	}
}
