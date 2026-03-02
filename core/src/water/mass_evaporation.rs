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

				// Count water depth below — deep bodies evaporate much slower
				let mut water_depth: u32 = 0;
				for dz in 1..=z {
					if world.water_mass(x, y, z - dz) > 0 {
						water_depth += 1;
					} else {
						break;
					}
				}

				let (evap_chance, evap_max): (u64, u8) = if water_depth >= 4 {
					(1, 2) // deep ocean: 1% chance, max 2
				} else if water_depth >= 2 {
					(3, 5) // moderate depth: 3% chance, max 5
				} else {
					(5, 10) // shallow/puddle: original behavior
				};

				let roll = simple_hash(x, y, z, seed) % 100;
				if roll < evap_chance {
					let evap = mass.min(evap_max);
					world.water_mass[idx] = mass - evap;
					world.atmospheric_moisture += evap as u32;
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

	#[test]
	fn evaporation_feeds_atmospheric_moisture() {
		let mut w = World::new(4, 4, 4);
		w.set(1, 1, 0, crate::tile::Tile::Stone);
		w.set_water_mass(1, 1, 1, 100);
		let initial_moisture = w.atmospheric_moisture;
		for _ in 0..200 {
			pass_evaporation(&mut w);
		}
		assert!(
			w.atmospheric_moisture > initial_moisture,
			"Atmospheric moisture should increase from evaporation"
		);
	}

	#[test]
	fn deep_water_evaporates_slower() {
		// Shallow water (depth 0)
		let mut shallow = World::new(4, 4, 8);
		shallow.set(1, 1, 0, crate::tile::Tile::Stone);
		shallow.set_water_mass(1, 1, 1, 255);

		// Deep water (depth 5)
		let mut deep = World::new(4, 4, 8);
		deep.set(1, 1, 0, crate::tile::Tile::Stone);
		for z in 1..=5 {
			deep.set_water_mass(1, 1, z, 255);
		}

		for _ in 0..200 {
			pass_evaporation(&mut shallow);
			pass_evaporation(&mut deep);
		}

		let shallow_lost = 255 - shallow.water_mass(1, 1, 1);
		let deep_top_lost = 255 - deep.water_mass(1, 1, 5);
		assert!(
			deep_top_lost < shallow_lost,
			"Deep water surface ({}) should evaporate less than shallow ({})",
			deep_top_lost,
			shallow_lost
		);
	}
}
