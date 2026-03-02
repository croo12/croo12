use std::sync::atomic::{AtomicU64, Ordering};

use crate::world::World;

static GW_TICK: AtomicU64 = AtomicU64::new(0);

pub fn pass_groundwater(world: &mut World) {
	let _seed = GW_TICK.fetch_add(1, Ordering::Relaxed);
	let w = world.width();
	let d = world.depth();
	let h = world.height();

	// Phase A: Absorption (surface water → soil moisture)
	for z in 1..h {
		for y in 0..d {
			for x in 0..w {
				let idx = world.index(x, y, z);
				let water = world.water_mass[idx];
				if water == 0 {
					continue;
				}
				if world.get(x, y, z).is_solid() {
					continue;
				}
				let below_idx = world.index(x, y, z - 1);
				let below_tile = world.get(x, y, z - 1);
				if !below_tile.is_solid() {
					continue;
				}
				let cap = below_tile.moisture_capacity();
				if cap == 0 {
					continue;
				}
				let current = world.soil_moisture[below_idx];
				let remaining_cap = cap.saturating_sub(current);
				if remaining_cap == 0 {
					continue;
				}
				let rate = below_tile.absorb_rate();
				let transfer = rate.min(remaining_cap).min(water);
				if transfer == 0 {
					continue;
				}
				world.water_mass[idx] -= transfer;
				world.soil_moisture[below_idx] += transfer;
			}
		}
	}

	// Phase B: Underground gravity (top → down through soil)
	for z in (1..h).rev() {
		for y in 0..d {
			for x in 0..w {
				let idx = world.index(x, y, z);
				let tile = world.get(x, y, z);
				if tile.moisture_capacity() == 0 {
					continue;
				}
				let moisture = world.soil_moisture[idx];
				if moisture == 0 {
					continue;
				}
				let below_idx = world.index(x, y, z - 1);
				let below_tile = world.get(x, y, z - 1);
				let below_cap = below_tile.moisture_capacity();
				if below_cap == 0 {
					continue;
				}
				let below_current = (world.soil_moisture[below_idx] as i16
					+ world.moisture_delta[below_idx])
					.max(0) as u8;
				let below_remaining = below_cap.saturating_sub(below_current);
				if below_remaining == 0 {
					continue;
				}
				let perm = tile.permeability().min(below_tile.permeability());
				let transfer = perm.min(below_remaining).min(moisture);
				if transfer == 0 {
					continue;
				}
				world.moisture_delta[idx] -= transfer as i16;
				world.moisture_delta[below_idx] += transfer as i16;
			}
		}
	}

	// Phase C: Horizontal pressure equalization
	let dir_offsets: [(isize, isize); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

	for z in 0..h {
		for y in 0..d {
			for x in 0..w {
				let idx = world.index(x, y, z);
				let tile = world.get(x, y, z);
				let cap = tile.moisture_capacity();
				if cap == 0 {
					continue;
				}
				let moisture =
					(world.soil_moisture[idx] as i16 + world.moisture_delta[idx]).max(0) as u8;
				if moisture == 0 {
					continue;
				}

				let budget = (moisture as u16) / 8;
				if budget == 0 {
					continue;
				}

				let mut total_diff: u16 = 0;
				let mut targets: [(usize, u8); 4] = [(0, 0); 4];

				for (i, &(dx, dy)) in dir_offsets.iter().enumerate() {
					let nx = x as isize + dx;
					let ny = y as isize + dy;
					if nx < 0 || nx >= w as isize || ny < 0 || ny >= d as isize {
						continue;
					}
					let nx = nx as usize;
					let ny = ny as usize;
					let n_tile = world.get(nx, ny, z);
					let n_cap = n_tile.moisture_capacity();
					if n_cap == 0 {
						continue;
					}
					let n_idx = world.index(nx, ny, z);
					let n_moisture = (world.soil_moisture[n_idx] as i16
						+ world.moisture_delta[n_idx])
						.max(0) as u8;
					if n_moisture >= moisture {
						continue;
					}
					let diff = moisture - n_moisture;
					targets[i] = (n_idx, diff);
					total_diff += diff as u16;
				}

				if total_diff == 0 {
					continue;
				}

				for &(n_idx, diff) in targets.iter() {
					if diff == 0 {
						continue;
					}
					let transfer = ((budget as u32 * diff as u32) / total_diff as u32) as i16;
					if transfer == 0 {
						continue;
					}
					world.moisture_delta[idx] -= transfer;
					world.moisture_delta[n_idx] += transfer;
				}
			}
		}
	}

	// Phase D: Seepage (soil moisture → surface water at Air boundaries)
	for z in 0..h {
		for y in 0..d {
			for x in 0..w {
				let idx = world.index(x, y, z);
				let tile = world.get(x, y, z);
				let cap = tile.moisture_capacity();
				if cap == 0 {
					continue;
				}
				let moisture =
					(world.soil_moisture[idx] as i16 + world.moisture_delta[idx]).max(0) as u8;
				let threshold = cap / 2;
				if moisture <= threshold {
					continue;
				}

				// Find first adjacent Air cell to seep into
				let mut seep_idx: Option<usize> = None;

				// Horizontal neighbors
				for &(dx, dy) in &dir_offsets {
					let nx = x as isize + dx;
					let ny = y as isize + dy;
					if nx < 0 || nx >= w as isize || ny < 0 || ny >= d as isize {
						continue;
					}
					let n_idx = world.index(nx as usize, ny as usize, z);
					if world.get(nx as usize, ny as usize, z).is_air() {
						seep_idx = Some(n_idx);
						break;
					}
				}
				// Above
				if seep_idx.is_none() && z + 1 < h && world.get(x, y, z + 1).is_air() {
					seep_idx = Some(world.index(x, y, z + 1));
				}

				if let Some(target) = seep_idx {
					let available = moisture - threshold;
					let capacity = 255u8.saturating_sub(world.water_mass[target]);
					let seep = available.min(2).min(capacity);
					if seep > 0 {
						world.moisture_delta[idx] -= seep as i16;
						world.water_mass[target] += seep;
					}
				}
			}
		}
	}

	// Phase E: Apply moisture deltas
	world.apply_moisture_deltas();
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tile::Tile;
	use crate::world::World;

	#[test]
	fn absorption_moves_surface_water_to_soil() {
		let mut w = World::new(4, 4, 4);
		w.set(1, 1, 0, Tile::Dirt); // capacity=128, rate=2
		w.set_water_mass(1, 1, 1, 100);
		pass_groundwater(&mut w);
		assert!(w.soil_moisture(1, 1, 0) > 0, "Dirt should absorb water");
		assert!(w.water_mass(1, 1, 1) < 100, "Surface water should decrease");
	}

	#[test]
	fn absorption_rate_varies_by_tile() {
		let mut w = World::new(4, 4, 4);
		w.set(0, 0, 0, Tile::Sand); // rate=8
		w.set(1, 0, 0, Tile::Dirt); // rate=2
		w.set_water_mass(0, 0, 1, 100);
		w.set_water_mass(1, 0, 1, 100);
		pass_groundwater(&mut w);
		let sand_absorbed = w.soil_moisture(0, 0, 0);
		let dirt_absorbed = w.soil_moisture(1, 0, 0);
		assert!(
			sand_absorbed > dirt_absorbed,
			"Sand ({}) should absorb faster than Dirt ({})",
			sand_absorbed,
			dirt_absorbed
		);
	}

	#[test]
	fn stone_does_not_absorb() {
		let mut w = World::new(4, 4, 4);
		w.set(1, 1, 0, Tile::Stone);
		w.set_water_mass(1, 1, 1, 100);
		pass_groundwater(&mut w);
		assert_eq!(w.soil_moisture(1, 1, 0), 0);
		assert_eq!(w.water_mass(1, 1, 1), 100);
	}

	#[test]
	fn absorption_respects_capacity() {
		let mut w = World::new(4, 4, 4);
		w.set(1, 1, 0, Tile::Sand); // capacity=48
		w.set_soil_moisture(1, 1, 0, 45); // nearly full
		w.set_water_mass(1, 1, 1, 100);
		pass_groundwater(&mut w);
		assert!(
			w.soil_moisture(1, 1, 0) <= 48,
			"Should not exceed capacity: {}",
			w.soil_moisture(1, 1, 0)
		);
	}

	#[test]
	fn gravity_moves_moisture_down() {
		let mut w = World::new(4, 4, 4);
		w.set(1, 1, 2, Tile::Dirt);
		w.set(1, 1, 1, Tile::Dirt);
		w.set_soil_moisture(1, 1, 2, 50);
		pass_groundwater(&mut w);
		assert!(
			w.soil_moisture(1, 1, 1) > 0,
			"Moisture should flow down"
		);
	}

	#[test]
	fn stone_blocks_underground_flow() {
		let mut w = World::new(4, 4, 4);
		w.set(1, 1, 2, Tile::Dirt);
		w.set(1, 1, 1, Tile::Stone);
		w.set(1, 1, 0, Tile::Dirt);
		w.set_soil_moisture(1, 1, 2, 50);
		pass_groundwater(&mut w);
		assert_eq!(
			w.soil_moisture(1, 1, 0),
			0,
			"Stone should block flow"
		);
	}

	#[test]
	fn seepage_creates_surface_water() {
		let mut w = World::new(5, 5, 4);
		for x in 0..5 {
			for y in 0..5 {
				w.set(x, y, 0, Tile::Stone);
			}
		}
		w.set(2, 2, 1, Tile::Dirt); // capacity=128, threshold=64
		w.set_soil_moisture(2, 2, 1, 100); // above threshold
		// (1,2,1), (3,2,1), (2,1,1), (2,3,1) are Air → seepage targets
		pass_groundwater(&mut w);
		let has_seepage = w.water_mass(1, 2, 1) > 0
			|| w.water_mass(3, 2, 1) > 0
			|| w.water_mass(2, 1, 1) > 0
			|| w.water_mass(2, 3, 1) > 0
			|| w.water_mass(2, 2, 2) > 0;
		assert!(has_seepage, "Moisture above threshold adjacent to Air should seep");
	}

	#[test]
	fn seepage_requires_threshold() {
		let mut w = World::new(5, 5, 4);
		for x in 0..5 {
			for y in 0..5 {
				w.set(x, y, 0, Tile::Stone);
			}
		}
		w.set(2, 2, 1, Tile::Dirt); // threshold = 128/2 = 64
		w.set_soil_moisture(2, 2, 1, 30); // below threshold
		pass_groundwater(&mut w);
		let total_water: u16 = (0..5)
			.flat_map(|x| {
				(0..5).flat_map(move |y| (0..4).map(move |z| (x, y, z)))
			})
			.map(|(x, y, z)| w.water_mass(x, y, z) as u16)
			.sum();
		assert_eq!(total_water, 0, "Below threshold should not seep");
	}

	#[test]
	fn horizontal_equalization_spreads_moisture() {
		let mut w = World::new(5, 5, 4);
		// Fill bottom with stone, z=1 with dirt
		for x in 0..5 {
			for y in 0..5 {
				w.set(x, y, 0, Tile::Stone);
				w.set(x, y, 1, Tile::Dirt);
			}
		}
		w.set_soil_moisture(2, 2, 1, 100);
		pass_groundwater(&mut w);
		// Neighbors should have some moisture
		let total_neighbor: u16 = [(1, 2), (3, 2), (2, 1), (2, 3)]
			.iter()
			.map(|&(x, y)| w.soil_moisture(x, y, 1) as u16)
			.sum();
		assert!(
			total_neighbor > 0,
			"Moisture should spread horizontally"
		);
	}
}
