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

	// Phase 2: Horizontal spread
	for z in 0..h {
		for y in 0..d {
			for x in 0..w {
				let idx = world.index(x, y, z);
				let mass = world.water_mass[idx];
				if mass == 0 {
					continue;
				}

				// Calculate remaining after gravity
				let remaining = (mass as i16 + world.mass_delta[idx]).max(0) as u8;
				if remaining == 0 {
					continue;
				}

				// Skip if can still fall (gravity priority)
				if z > 0 {
					let below_idx = world.index(x, y, z - 1);
					let below_expected = (world.water_mass[below_idx] as i16
						+ world.mass_delta[below_idx])
						.min(255)
						.max(0) as u8;
					if !world.get(x, y, z - 1).is_solid() && below_expected < 255 {
						continue;
					}
				}

				// Collect valid neighbors with lower mass (sorted ascending)
				let neighbors: [(usize, usize); 4] = [
					(x.wrapping_sub(1), y),
					(x + 1, y),
					(x, y.wrapping_sub(1)),
					(x, y + 1),
				];

				let mut valid_with_mass: Vec<(usize, u8)> = Vec::new();
				for &(nx, ny) in &neighbors {
					if nx >= w || ny >= d {
						continue;
					}
					if world.get(nx, ny, z).is_solid() {
						continue;
					}
					let n_idx = world.index(nx, ny, z);
					let n_mass =
						(world.water_mass[n_idx] as i16 + world.mass_delta[n_idx]).max(0) as u8;
					if n_mass < remaining {
						valid_with_mass.push((n_idx, n_mass));
					}
				}

				if valid_with_mass.is_empty() {
					continue;
				}

				valid_with_mass.sort_by_key(|&(_, m)| m);

				// Equilibrium: pool shallowest neighbors first, compute average target
				let mut total_mass = remaining as u16;
				let mut receivers: Vec<(usize, u8)> = Vec::new();

				for &(n_idx, n_mass) in &valid_with_mass {
					let avg = total_mass / (receivers.len() as u16 + 1);
					if (n_mass as u16) < avg {
						total_mass += n_mass as u16;
						receivers.push((n_idx, n_mass));
					}
				}

				if !receivers.is_empty() {
					let final_avg = total_mass / (receivers.len() as u16 + 1);

					for &(n_idx, n_mass) in &receivers {
						let transfer = final_avg.saturating_sub(n_mass as u16);
						if transfer == 0 {
							continue;
						}

						let sed = world.water_sediment[idx];
						let sed_transfer =
							calc_sediment_transfer(sed, transfer, remaining, idx, seed);
						world.record_flow(idx, n_idx, transfer, sed_transfer);
					}
				}
			}
		}
	}

	// Phase 3: Pressure (bottom-up) - push excess up
	for z in 0..h.saturating_sub(1) {
		for y in 0..d {
			for x in 0..w {
				let idx = world.index(x, y, z);
				let expected = world.water_mass[idx] as i16 + world.mass_delta[idx];
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
