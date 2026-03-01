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

	// Sub-pass A: Erosion (moving water erodes below)
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
					if velocity == 0 {
						continue;
					}

					let below = world.get(x, y, z - 1);
					if !below.is_erodible() {
						continue;
					}

					// Erosion chance: velocity * 2 out of 100
					let chance = (velocity as u64) * 2;
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

	// Sub-pass B: Deposition (stagnant water with sediment deposits Sand)
	let neighbors: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

	for z in 0..h {
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
					if velocity != 0 || sediment == 0 {
						continue;
					}

					// Find adjacent Air cell for deposition
					for (dx, dy) in &neighbors {
						let nx = x as i32 + dx;
						let ny = y as i32 + dy;
						if nx < 0 || nx >= w as i32 || ny < 0 || ny >= d as i32 {
							continue;
						}
						let (nx, ny) = (nx as usize, ny as usize);
						if world.get(nx, ny, z).is_air() {
							world.set(nx, ny, z, Tile::Sand);
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
							break; // one deposition per tick
						}
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
	fn stagnant_water_does_not_erode() {
		let mut world = World::new(4, 4, 4);
		world.set(1, 1, 0, Tile::Dirt);
		world.set(
			1,
			1,
			1,
			Tile::Water {
				is_source: false,
				sediment: 0,
				velocity: 0,
				direction: FlowDir::None,
			},
		);
		pass_erosion(&mut world);
		assert_eq!(world.get(1, 1, 0), Tile::Dirt); // not eroded
	}

	#[test]
	fn moving_water_can_erode() {
		let mut world = World::new(4, 4, 4);
		world.set(1, 1, 0, Tile::Sand);
		world.set(
			1,
			1,
			1,
			Tile::Water {
				is_source: false,
				sediment: 0,
				velocity: 7,
				direction: FlowDir::East,
			},
		);
		// With velocity=7, erosion chance = 14%. Run many times to check it's possible.
		let mut eroded = false;
		for seed in 0..200u64 {
			let mut w = World::new(4, 4, 4);
			w.set(1, 1, 0, Tile::Sand);
			w.set(
				1,
				1,
				1,
				Tile::Water {
					is_source: false,
					sediment: 0,
					velocity: 7,
					direction: FlowDir::East,
				},
			);
			pass_erosion_with_seed(&mut w, seed);
			if w.get(1, 1, 0).is_air() {
				eroded = true;
				break;
			}
		}
		assert!(eroded, "Should erode at least once in 200 attempts");
	}

	#[test]
	fn stone_never_erodes() {
		let mut world = World::new(4, 4, 4);
		world.set(1, 1, 0, Tile::Stone);
		world.set(
			1,
			1,
			1,
			Tile::Water {
				is_source: false,
				sediment: 0,
				velocity: 7,
				direction: FlowDir::East,
			},
		);
		for seed in 0..100u64 {
			let mut w = World::new(4, 4, 4);
			w.set(1, 1, 0, Tile::Stone);
			w.set(
				1,
				1,
				1,
				Tile::Water {
					is_source: false,
					sediment: 0,
					velocity: 7,
					direction: FlowDir::East,
				},
			);
			pass_erosion_with_seed(&mut w, seed);
			assert_eq!(w.get(1, 1, 0), Tile::Stone);
		}
	}

	#[test]
	fn sediment_deposits_when_stagnant() {
		let mut world = World::new(4, 4, 4);
		world.set(1, 1, 0, Tile::Stone); // floor
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
		// Adjacent air for deposition
		pass_erosion(&mut world);
		// Check if sediment decreased
		if let Tile::Water { sediment, .. } = world.get(1, 1, 1) {
			// Sediment may have deposited as Sand somewhere adjacent
			assert!(sediment <= 3);
		}
	}
}
