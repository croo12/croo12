use crate::tile::{FlowDir, Tile};
use crate::world::World;

pub fn pass_gravity(world: &mut World) {
	let w = world.width();
	let d = world.depth();
	let h = world.height();

	// Iterate bottom-to-top so fallen water isn't re-processed
	for z in 1..h {
		for y in 0..d {
			for x in 0..w {
				let tile = world.get(x, y, z);
				if let Tile::Water {
					is_source,
					sediment,
					velocity,
					..
				} = tile
				{
					let below = world.get(x, y, z - 1);
					if below.is_air() {
						let new_vel = (velocity + 1).min(7);
						world.set(
							x,
							y,
							z - 1,
							Tile::Water {
								is_source: false,
								sediment,
								velocity: new_vel,
								direction: FlowDir::Down,
							},
						);
						if is_source {
							// Source stays, reset velocity
							world.set(x, y, z, Tile::water_source());
						} else {
							world.set(x, y, z, Tile::Air);
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
	fn water_falls_one_cell() {
		let mut world = World::new(4, 4, 8);
		world.set(0, 0, 3, Tile::water_default());
		pass_gravity(&mut world);
		assert!(world.get(0, 0, 2).is_water());
		assert!(world.get(0, 0, 3).is_air());
	}

	#[test]
	fn water_stops_on_solid() {
		let mut world = World::new(4, 4, 8);
		world.set(0, 0, 0, Tile::Stone);
		world.set(0, 0, 1, Tile::water_default());
		pass_gravity(&mut world);
		assert!(world.get(0, 0, 1).is_water()); // stays
	}

	#[test]
	fn falling_water_accelerates() {
		let mut world = World::new(4, 4, 8);
		world.set(
			0,
			0,
			4,
			Tile::Water {
				is_source: false,
				sediment: 0,
				velocity: 2,
				direction: FlowDir::Down,
			},
		);
		pass_gravity(&mut world);
		match world.get(0, 0, 3) {
			Tile::Water {
				velocity,
				direction,
				..
			} => {
				assert_eq!(velocity, 3); // vel + 1
				assert_eq!(direction, FlowDir::Down);
			}
			_ => panic!("expected Water"),
		}
	}

	#[test]
	fn falling_water_max_velocity() {
		let mut world = World::new(4, 4, 8);
		world.set(
			0,
			0,
			2,
			Tile::Water {
				is_source: false,
				sediment: 0,
				velocity: 7,
				direction: FlowDir::Down,
			},
		);
		pass_gravity(&mut world);
		match world.get(0, 0, 1) {
			Tile::Water { velocity, .. } => assert_eq!(velocity, 7), // capped
			_ => panic!("expected Water"),
		}
	}

	#[test]
	fn water_does_not_fall_through_water() {
		let mut world = World::new(4, 4, 8);
		world.set(0, 0, 0, Tile::Stone); // ground
		world.set(0, 0, 1, Tile::water_default());
		world.set(0, 0, 2, Tile::water_default());
		pass_gravity(&mut world);
		assert!(world.get(0, 0, 1).is_water());
		assert!(world.get(0, 0, 2).is_water()); // blocked by water below
	}
}
