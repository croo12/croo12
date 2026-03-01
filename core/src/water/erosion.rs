use crate::tile::Tile;
use crate::world::World;

/// Simple hash for deterministic pseudo-random erosion
fn simple_hash(x: usize, y: usize, z: usize, seed: u64) -> u64 {
	let mut h = seed;
	h = h.wrapping_mul(6364136223846793005).wrapping_add(x as u64);
	h = h.wrapping_mul(6364136223846793005).wrapping_add(y as u64);
	h = h.wrapping_mul(6364136223846793005).wrapping_add(z as u64);
	h ^ (h >> 33)
}

/// Count consecutive water cells above (x, y, z), i.e. the water pressure.
fn water_depth_above(world: &World, x: usize, y: usize, z: usize) -> usize {
	let h = world.height();
	let mut depth = 0;
	let mut cz = z + 1;
	while cz < h {
		if world.get(x, y, cz).is_water() {
			depth += 1;
			cz += 1;
		} else {
			break;
		}
	}
	depth
}

static mut EROSION_TICK: u64 = 0;

pub fn pass_erosion(world: &mut World) {
	let seed = unsafe { EROSION_TICK };
	unsafe {
		EROSION_TICK = EROSION_TICK.wrapping_add(1);
	}
	pass_erosion_with_seed(world, seed);
}

pub fn pass_erosion_with_seed(world: &mut World, seed: u64) {
	let w = world.width();
	let d = world.depth();
	let h = world.height();

	// Sub-pass A: Erosion — water with pressure erodes the tile below
	for z in 1..h {
		for y in 0..d {
			for x in 0..w {
				let tile = world.get(x, y, z);
				if let Tile::Water {
					is_source,
					sediment,
					velocity,
					direction,
				} = tile
				{
					let below = world.get(x, y, z - 1);
					if !below.is_erodible() {
						continue;
					}

					// Pressure = water depth above this cell
					let pressure = water_depth_above(world, x, y, z);

					// Erosion chance: pressure * 10 out of 100
					// depth=1 → 10%, depth=2 → 20%, depth=3+ → 30%+
					let chance = ((pressure as u64) * 10).min(50);
					if chance == 0 {
						continue;
					}

					let roll = simple_hash(x, y, z, seed) % 100;
					if roll < chance {
						world.set(x, y, z - 1, Tile::Air);
						let new_sed = (sediment + 1).min(7);
						world.set(
							x,
							y,
							z,
							Tile::Water {
								is_source,
								sediment: new_sed,
								velocity,
								direction,
							},
						);
					}
				}
			}
		}
	}

	// Sub-pass B: Deposition (stagnant water with sediment deposits Sand below)
	for z in 1..h {
		for y in 0..d {
			for x in 0..w {
				let tile = world.get(x, y, z);
				if let Tile::Water {
					is_source,
					sediment,
					velocity,
					direction,
				} = tile
				{
					if sediment == 0 {
						continue;
					}

					// Deposit only if no pressure (single block or top of column)
					let pressure = water_depth_above(world, x, y, z);
					if pressure > 0 {
						continue;
					}

					// Deposit below if air
					if world.get(x, y, z - 1).is_air() {
						world.set(x, y, z - 1, Tile::Sand);
						let new_sed = sediment - 1;
						world.set(
							x,
							y,
							z,
							Tile::Water {
								is_source,
								sediment: new_sed,
								velocity,
								direction,
							},
						);
					}
				}
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tile::{FlowDir, Tile};
	use crate::world::World;

	#[test]
	fn no_pressure_no_erosion() {
		let mut world = World::new(4, 4, 4);
		world.set(1, 1, 0, Tile::Dirt);
		world.set(1, 1, 1, Tile::water_default());
		// Single water block, no water above → no pressure
		for seed in 0..100u64 {
			let mut w = World::new(4, 4, 4);
			w.set(1, 1, 0, Tile::Dirt);
			w.set(1, 1, 1, Tile::water_default());
			pass_erosion_with_seed(&mut w, seed);
			assert_eq!(w.get(1, 1, 0), Tile::Dirt);
		}
	}

	#[test]
	fn pressure_causes_erosion() {
		// Stack water 3 high for pressure=2 on the bottom cell → 20% chance
		let mut eroded = false;
		for seed in 0..100u64 {
			let mut w = World::new(4, 4, 6);
			w.set(1, 1, 0, Tile::Dirt);
			w.set(1, 1, 1, Tile::water_default());
			w.set(1, 1, 2, Tile::water_default());
			w.set(1, 1, 3, Tile::water_default());
			pass_erosion_with_seed(&mut w, seed);
			if w.get(1, 1, 0).is_air() {
				eroded = true;
				break;
			}
		}
		assert!(eroded, "Should erode at least once in 100 attempts with pressure=2");
	}

	#[test]
	fn stone_never_erodes() {
		let mut world = World::new(4, 4, 6);
		for seed in 0..100u64 {
			let mut w = World::new(4, 4, 6);
			w.set(1, 1, 0, Tile::Stone);
			w.set(1, 1, 1, Tile::water_default());
			w.set(1, 1, 2, Tile::water_default());
			w.set(1, 1, 3, Tile::water_default());
			pass_erosion_with_seed(&mut w, seed);
			assert_eq!(w.get(1, 1, 0), Tile::Stone);
		}
	}

	#[test]
	fn sediment_deposits_below() {
		let mut world = World::new(4, 4, 4);
		world.set(
			1,
			1,
			1,
			Tile::Water {
				is_source: false,
				sediment: 3,
				velocity: 0,
				direction: FlowDir::None,
			},
		);
		// Air below at z=0 and no pressure → should deposit
		pass_erosion(&mut world);
		assert_eq!(world.get(1, 1, 0), Tile::Sand);
		if let Tile::Water { sediment, .. } = world.get(1, 1, 1) {
			assert_eq!(sediment, 2);
		}
	}
}
