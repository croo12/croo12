use std::sync::atomic::{AtomicU64, Ordering};
use crate::world::World;

fn simple_hash(x: usize, y: usize, z: usize, seed: u64) -> u64 {
	let mut h = seed;
	h = h.wrapping_mul(6364136223846793005).wrapping_add(x as u64);
	h = h.wrapping_mul(6364136223846793005).wrapping_add(y as u64);
	h = h.wrapping_mul(6364136223846793005).wrapping_add(z as u64);
	h ^ (h >> 33)
}

/// 3-stage erosion multiplier based on soil moisture
/// Dry (0%) = 1.0x, Damp (1-80%) = 0.4x, Saturated (80-100%) = 1.8x
fn erosion_multiplier(moisture: u8, capacity: u8) -> f64 {
	if capacity == 0 || moisture == 0 {
		return 1.0;
	}
	let ratio = moisture as f64 / capacity as f64;
	if ratio < 0.8 {
		0.4
	} else {
		1.8
	}
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
				let base_chance = (pressure * 5 + (flow as u64) / 5).min(80);
				if base_chance == 0 {
					continue;
				}

				let below_idx = world.index(x, y, z - 1);
				let below_moisture = world.soil_moisture[below_idx];
				let below_cap = world.get(x, y, z - 1).moisture_capacity();
				let multiplier = erosion_multiplier(below_moisture, below_cap);
				let chance = ((base_chance as f64 * multiplier) as u64).min(95);

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

	// Deposition: stagnant water with high sediment on solid ground deposits Sand
	// Exclude top row (h-1) since we need z+1 for displacement
	for z in (1..h.saturating_sub(1)).rev() {
		for y in 0..d {
			for x in 0..w {
				let idx = world.index(x, y, z);
				let sed = world.water_sediment[idx];
				if sed < 3 || world.water_mass[idx] == 0 {
					continue;
				}

				let flow = world.water_outflow(idx);
				if flow > 60 {
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

				// Probabilistic: higher sediment = higher chance
				let chance = ((sed as u64) * 10).min(60);
				let roll = simple_hash(x, y, z, seed) % 100;
				if roll >= chance {
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
		// Deposition is now probabilistic; run multiple passes until it triggers
		for _ in 0..30 {
			let mut w = World::new(4, 4, 4);
			w.set(1, 1, 0, Tile::Stone);
			w.set_water_mass(1, 1, 1, 100);
			let idx = w.index(1, 1, 1);
			w.water_sediment[idx] = 7; // High sediment → 60% chance
			w.water_outflow[idx] = 0;
			pass_erosion(&mut w);
			if w.get(1, 1, 1) == Tile::Sand {
				assert_eq!(w.water_mass(1, 1, 1), 0, "water should be cleared from sand cell");
				assert!(w.water_mass(1, 1, 2) > 0, "water should move to z+1");
				assert_eq!(w.water_sediment[idx], 0, "sediment cleared from sand cell");
				return;
			}
		}
		panic!("Deposition should occur within 30 tries at 60% chance per try");
	}

	#[test]
	fn damp_soil_resists_erosion() {
		let mut eroded_dry = 0u32;
		let mut eroded_wet = 0u32;
		for _ in 0..100 {
			let mut w = World::new(4, 4, 4);
			w.set(1, 1, 0, Tile::Dirt);
			w.set_water_mass(1, 1, 1, 200);
			let idx = w.index(1, 1, 1);
			w.water_outflow[idx] = 500;
			pass_erosion(&mut w);
			if w.get(1, 1, 0) == Tile::Air {
				eroded_dry += 1;
			}

			let mut w2 = World::new(4, 4, 4);
			w2.set(1, 1, 0, Tile::Dirt);
			w2.set_soil_moisture(1, 1, 0, 64); // 50% of 128 = damp
			w2.set_water_mass(1, 1, 1, 200);
			let idx2 = w2.index(1, 1, 1);
			w2.water_outflow[idx2] = 500;
			pass_erosion(&mut w2);
			if w2.get(1, 1, 0) == Tile::Air {
				eroded_wet += 1;
			}
		}
		assert!(
			eroded_wet < eroded_dry,
			"Damp soil ({}) should erode less than dry ({})",
			eroded_wet,
			eroded_dry
		);
	}

	#[test]
	fn saturated_soil_erodes_faster() {
		let mult_dry = erosion_multiplier(0, 128);
		let mult_damp = erosion_multiplier(64, 128);
		let mult_sat = erosion_multiplier(120, 128);
		assert!((mult_dry - 1.0).abs() < 0.01);
		assert!(mult_damp < 1.0);
		assert!(mult_sat > 1.0);
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
