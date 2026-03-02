use crate::world::World;

/// Simple hash for deterministic pseudo-random sediment rounding
fn simple_hash(idx: usize, seed: u64) -> u64 {
	let mut h = seed;
	h = h.wrapping_mul(6364136223846793005).wrapping_add(idx as u64);
	h ^ (h >> 33)
}

/// Transfer sediment proportionally when water flows.
/// Uses probabilistic rounding for small amounts.
fn calc_sediment_transfer(sediment: u8, transfer: u16, mass: u8, idx: usize, seed: u64) -> i16 {
	if sediment == 0 || mass == 0 {
		return 0;
	}
	let exact = (sediment as u32 * transfer as u32) as f64 / mass as f64;
	let base = exact.floor() as i16;
	let frac = exact - exact.floor();
	let roll = (simple_hash(idx, seed) % 1000) as f64 / 1000.0;
	if roll < frac {
		base + 1
	} else {
		base
	}
}

static FLOW_TICK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn pass_flow(world: &mut World) {
	let seed = FLOW_TICK.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

	// Clear previous tick's outflow before recording new flows
	world.clear_outflow();

	let w = world.width();
	let d = world.depth();
	let h = world.height();

	// Phase 1: Gravity (top-down)
	for z in (1..h).rev() {
		for y in 0..d {
			for x in 0..w {
				let idx = world.index(x, y, z);
				let mass = world.water_mass[idx];
				if mass == 0 {
					continue;
				}

				let below_idx = world.index(x, y, z - 1);
				if world.get(x, y, z - 1).is_solid() {
					continue;
				}

				let below_mass = world.water_mass[below_idx];
				if below_mass >= 255 {
					continue;
				}

				let capacity = 255u16.saturating_sub(below_mass as u16);
				let transfer = (mass as u16).min(capacity);
				if transfer == 0 {
					continue;
				}

				let sed = world.water_sediment[idx];
				let sed_transfer = calc_sediment_transfer(sed, transfer, mass, idx, seed);
				world.record_flow(idx, below_idx, transfer, sed_transfer);
			}
		}
	}

	// Phase 2: Slope-proportional horizontal spread with flow memory
	// Directions: 0=-x, 1=+x, 2=-y, 3=+y
	let dir_offsets: [(isize, isize); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

	// Snapshot post-gravity expected mass — all cells read from the same state,
	// eliminating directional bias from iteration order.
	// OPTIMIZATION: Use pre-allocated water_snapshot to avoid per-tick allocation
	for i in 0..(w * d * h) {
		world.water_snapshot[i] =
			(world.water_mass[i] as i16 + world.mass_delta[i]).clamp(0, 255) as u8;
	}

	for z in 0..h {
		for y in 0..d {
			for x in 0..w {
				let idx = world.index(x, y, z);
				let remaining = world.water_snapshot[idx];
				if remaining == 0 {
					continue;
				}

				// Skip if can still fall (gravity priority)
				if z > 0 {
					let below_idx = world.index(x, y, z - 1);
					if !world.get(x, y, z - 1).is_solid() && world.water_snapshot[below_idx] < 255 {
						continue;
					}
				}

				let prev_dir = world.flow_dir[idx];
				let mut slopes: [(usize, u16, u8); 4] = [(0, 0, 0); 4]; // (n_idx, slope, n_mass)
				let mut total_slope: u32 = 0;
				let mut lower_sum: u32 = 0;
				let mut lower_count: u32 = 0;

				for (i, &(dx, dy)) in dir_offsets.iter().enumerate() {
					let nx = x as isize + dx;
					let ny = y as isize + dy;
					if nx < 0 || nx >= w as isize || ny < 0 || ny >= d as isize {
						continue;
					}
					let nx = nx as usize;
					let ny = ny as usize;
					if world.get(nx, ny, z).is_solid() {
						continue;
					}
					let n_idx = world.index(nx, ny, z);
					let n_mass = world.water_snapshot[n_idx];
					if n_mass >= remaining {
						continue;
					}

					let diff = (remaining - n_mass) as u16;
					
					// Surface tension: require a minimum mass difference (e.g. 5) to spread laterally
					// This stops water from spreading infinitely into a 1-depth puddle
					if diff < 5 {
						continue;
					}

					// Flow memory: 300% bonus (diff * 4) in previously flowing directions
					// Strongly encourages straight rivers rather than radial spreads
					let slope = if prev_dir & (1 << i) != 0 {
						diff * 4
					} else {
						diff
					};

					slopes[i] = (n_idx, slope, n_mass);
					total_slope += slope as u32;
					lower_sum += n_mass as u32;
					lower_count += 1;
				}

				if lower_count == 0 || total_slope == 0 {
					continue;
				}

				// Equalization-based budget: target = avg of self + lower neighbors
				let target = ((remaining as u32 + lower_sum) / (1 + lower_count)) as u16;
				
				// Ensure the budget leaves the spread threshold behind to maintain surface tension
				if remaining as u16 <= target + 2 {
					continue;
				}

				let budget = remaining as u16 - target;
				let mut new_dir: u8 = 0;

				for (i, &(n_idx, slope, n_mass)) in slopes.iter().enumerate() {
					if slope == 0 {
						continue;
					}
					// Slope-proportional share, capped at (target - n_mass)
					let proportional = ((budget as u32 * slope as u32) / total_slope) as u16;
					let equalize_cap = target.saturating_sub(n_mass as u16);
					let transfer = proportional.min(equalize_cap);
					if transfer == 0 {
						continue;
					}

					new_dir |= 1 << i;

					let sed = (world.water_sediment[idx] as i16
						+ world.sediment_delta[idx])
						.max(0) as u8;
					let sed_transfer =
						calc_sediment_transfer(sed, transfer, remaining, idx, seed);
					world.record_flow(idx, n_idx, transfer, sed_transfer);
				}

				world.flow_dir[idx] = new_dir;
			}
		}
	}

	// Phase 3: Pressure (bottom-up) - push excess up
	for z in 0..h.saturating_sub(1) {
		for y in 0..d {
			for x in 0..w {
				let idx = world.index(x, y, z);
				let expected = world.water_mass[idx] as i16 + world.mass_delta[idx];
				
				// OPTIMIZATION: Early exit for empty blocks or non-overflowing blocks
				if expected <= 255 {
					continue;
				}

				let excess = (expected - 255) as u16;
				let above_idx = world.index(x, y, z + 1);
				// Only push up if above is not solid
				if world.get(x, y, z + 1).is_solid() {
					continue;
				}

				world.mass_delta[idx] -= excess as i16;
				world.mass_delta[above_idx] += excess as i16;
			}
		}
	}

	// Phase 4: Apply deltas
	world.apply_water_deltas();
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tile::Tile;
	use crate::world::World;

	#[test]
	fn gravity_water_falls_into_air() {
		let mut w = World::new(4, 4, 4);
		// Walled column so horizontal spread doesn't drain water
		for z in 0..4 {
			for x in 0..4 {
				for y in 0..4 {
					if x == 0 && y == 0 {
						continue;
					}
					w.set(x, y, z, Tile::Stone);
				}
			}
		}
		w.set(0, 0, 0, Tile::Stone);
		w.set_water_mass(0, 0, 2, 100);
		pass_flow(&mut w);
		assert_eq!(w.water_mass(0, 0, 1), 100);
		assert_eq!(w.water_mass(0, 0, 2), 0);
	}

	#[test]
	fn gravity_fills_below_to_capacity() {
		let mut w = World::new(4, 4, 4);
		// Walled column so horizontal spread doesn't drain water
		for z in 0..4 {
			for x in 0..4 {
				for y in 0..4 {
					if x == 0 && y == 0 {
						continue;
					}
					w.set(x, y, z, Tile::Stone);
				}
			}
		}
		w.set(0, 0, 0, Tile::Stone);
		w.set_water_mass(0, 0, 1, 200);
		w.set_water_mass(0, 0, 2, 100);
		pass_flow(&mut w);
		// below was 200, can take 55 more; column is walled so no horizontal spread
		assert_eq!(w.water_mass(0, 0, 1), 255);
		assert_eq!(w.water_mass(0, 0, 2), 45);
	}

	#[test]
	fn gravity_stops_on_solid() {
		let mut w = World::new(4, 4, 4);
		// Walled column so horizontal spread doesn't move water away
		for z in 0..4 {
			for x in 0..4 {
				for y in 0..4 {
					if x == 0 && y == 0 {
						continue;
					}
					w.set(x, y, z, Tile::Stone);
				}
			}
		}
		w.set(0, 0, 0, Tile::Stone);
		w.set_water_mass(0, 0, 1, 100);
		pass_flow(&mut w);
		assert_eq!(w.water_mass(0, 0, 1), 100); // stays
	}

	#[test]
	fn horizontal_spread_equalizes() {
		let mut w = World::new(4, 4, 4);
		// Solid floor
		for x in 0..4 {
			for y in 0..4 {
				w.set(x, y, 0, Tile::Stone);
			}
		}
		w.set_water_mass(2, 2, 1, 200);
		pass_flow(&mut w);
		// Water should have spread to neighbors
		let center = w.water_mass(2, 2, 1);
		let total: u16 = (0..4)
			.flat_map(|x| (0..4).map(move |y| (x, y)))
			.map(|(x, y)| w.water_mass(x, y, 1) as u16)
			.sum();
		assert_eq!(total, 200); // mass conserved
		assert!(center < 200); // some spread out
	}

	#[test]
	fn spread_no_uphill_flow() {
		// Scenario: center=100, east=50, west=0, others blocked
		// Equilibrium: all should reach 50 (100+50+0)/3 = 50
		// Use a 5x5 world, wall everything except the 3 target cells
		let mut w = World::new(5, 5, 3);
		// Fill everything with stone
		for x in 0..5 {
			for y in 0..5 {
				for z in 0..3 {
					w.set(x, y, z, Tile::Stone);
				}
			}
		}
		// Open only the 3 cells in a row: (1,2,1), (2,2,1), (3,2,1)
		w.set(1, 2, 1, Tile::Air);
		w.set(2, 2, 1, Tile::Air);
		w.set(3, 2, 1, Tile::Air);
		// West=0, Center=100, East=50
		w.set_water_mass(1, 2, 1, 0);
		w.set_water_mass(2, 2, 1, 100);
		w.set_water_mass(3, 2, 1, 50);
		pass_flow(&mut w);
		let west = w.water_mass(1, 2, 1);
		let center = w.water_mass(2, 2, 1);
		let east = w.water_mass(3, 2, 1);
		// East should NOT exceed center (no uphill flow)
		assert!(
			east <= center,
			"East ({}) should not exceed center ({}) - uphill flow bug!",
			east,
			center
		);
		// Mass conserved
		assert_eq!(
			west as u16 + center as u16 + east as u16,
			150,
			"Mass not conserved: west={} center={} east={}",
			west,
			center,
			east
		);
	}

	#[test]
	fn pressure_pushes_up() {
		let mut w = World::new(4, 4, 4);
		// Walled box: solid everywhere except (1,1,1) and (1,1,2)
		for x in 0..4 {
			for y in 0..4 {
				for z in 0..4 {
					w.set(x, y, z, Tile::Stone);
				}
			}
		}
		w.set(1, 1, 1, Tile::Air);
		w.set(1, 1, 2, Tile::Air);
		// Overfill (1,1,1) via deltas
		w.set_water_mass(1, 1, 1, 200);
		let idx = w.index(1, 1, 1);
		w.mass_delta[idx] = 100; // would be 300, excess 45
		// Manually run Phase 3 + apply
		pass_flow(&mut w);
		// Some mass should appear at z=2
		assert!(w.water_mass(1, 1, 2) > 0);
		assert!(w.water_mass(1, 1, 1) <= 255);
	}

	#[test]
	fn source_fills_continuously() {
		let mut w = World::new(4, 4, 8);
		for x in 0..4 {
			for y in 0..4 {
				w.set(x, y, 0, Tile::Stone);
			}
		}
		w.add_source(2, 2, 1);
		// Run many ticks - source should produce water
		for _ in 0..50 {
			pass_flow(&mut w);
			// Source replenishment
			for &(sx, sy, sz) in w.sources().to_vec().iter() {
				let idx = w.index(sx, sy, sz);
				w.water_mass[idx] = w.water_mass[idx].saturating_add(50);
			}
		}
		// Water should have spread away from source
		let total: u32 = (0..4)
			.flat_map(|x| (0..4).flat_map(move |y| (0..8).map(move |z| (x, y, z))))
			.map(|(x, y, z)| w.water_mass(x, y, z) as u32)
			.sum();
		assert!(total > 255, "Should have significant water: {}", total);
	}

	#[test]
	fn phase2_sediment_reflects_gravity_loss() {
		// Cell (2,2,2): water=100, sediment=10
		// Below (2,2,1): water=205 → capacity=50
		// Phase 1: 50 water + 5 sediment fall down
		// Phase 2: snapshot remaining=50, effective sediment=5
		//   One horizontal neighbor (3,2,2): target=(50+0)/2=25, transfer=25
		//   Correct: calc_sediment_transfer(5, 25, 50) = 2.5 → 2 or 3
		//   Buggy (raw sed=10): calc_sediment_transfer(10, 25, 50) = 5
		let mut w = World::new(5, 5, 5);
		for x in 0..5 {
			for y in 0..5 {
				for z in 0..5 {
					w.set(x, y, z, Tile::Stone);
				}
			}
		}
		w.set(2, 2, 2, Tile::Air);
		w.set(2, 2, 1, Tile::Air);
		w.set(3, 2, 2, Tile::Air);

		w.set_water_mass(2, 2, 2, 100);
		w.set_water_sediment(2, 2, 2, 10);
		w.set_water_mass(2, 2, 1, 205);

		pass_flow(&mut w);

		let below_sed = w.water_sediment(2, 2, 1);
		let horiz_sed = w.water_sediment(3, 2, 2);
		let remain_sed = w.water_sediment(2, 2, 2);
		let total = below_sed as u16 + horiz_sed as u16 + remain_sed as u16;

		// Sediment must be conserved
		assert_eq!(
			total, 10,
			"Sediment conserved: below={} horiz={} remain={}",
			below_sed, horiz_sed, remain_sed
		);
		// Gravity carries 50% water → 5 sediment
		assert_eq!(below_sed, 5, "Gravity should carry 5 sediment");
		// Horizontal should use effective sediment (5), not raw (10)
		// 5 * 25/50 = 2.5 → expect 2 or 3, NOT 5
		assert!(
			horiz_sed <= 3,
			"Expected ~2-3 from effective sed=5, got {} (raw sed bug?)",
			horiz_sed
		);
	}

	#[test]
	fn sediment_moves_with_water() {
		let mut w = World::new(4, 4, 4);
		w.set(0, 0, 0, Tile::Stone);
		w.set_water_mass(0, 0, 2, 200);
		w.set_water_sediment(0, 0, 2, 4);
		pass_flow(&mut w);
		// Water fell to z=1, sediment should follow
		assert!(w.water_sediment(0, 0, 1) > 0);
	}
}
