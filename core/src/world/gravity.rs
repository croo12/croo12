use crate::tile::{FlowDir, Tile};
use super::World;

pub fn pass_gravity(world: &mut World) {
	let w = world.width();
	let d = world.depth();
	let h = world.height();

	// Iterate bottom-to-top so lower tiles fall first, then upper tiles follow
	for z in 1..h {
		for y in 0..d {
			for x in 0..w {
				let tile = world.get(x, y, z);
				if !tile.falls() || world.get(x, y, z - 1).is_air() == false {
					continue;
				}

				// Find the lowest air cell in this column
				let mut target_z = z - 1;
				while target_z > 0 && world.get(x, y, target_z - 1).is_air() {
					target_z -= 1;
				}

				// Place tile at target, handle source water specially
				match tile {
					Tile::Water {
						is_source,
						sediment,
						..
					} => {
						world.set(
							x,
							y,
							target_z,
							Tile::Water {
								is_source: false,
								sediment,
								velocity: 0,
								direction: FlowDir::Down,
							},
						);
						if is_source {
							world.set(x, y, z, Tile::water_source());
						} else {
							world.set(x, y, z, Tile::Air);
						}
					}
					_ => {
						// Solid tile (Grass, Dirt, Sand): move directly
						world.set(x, y, target_z, tile);
						world.set(x, y, z, Tile::Air);
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
	use super::World;

	#[test]
	fn water_falls_to_ground() {
		let mut world = World::new(4, 4, 8);
		world.set(0, 0, 0, Tile::Stone);
		world.set(0, 0, 5, Tile::water_default());
		pass_gravity(&mut world);
		assert!(world.get(0, 0, 1).is_water());
		assert!(world.get(0, 0, 5).is_air());
	}

	#[test]
	fn water_stops_on_solid() {
		let mut world = World::new(4, 4, 8);
		world.set(0, 0, 0, Tile::Stone);
		world.set(0, 0, 1, Tile::water_default());
		pass_gravity(&mut world);
		assert!(world.get(0, 0, 1).is_water());
	}

	#[test]
	fn water_stops_on_water() {
		let mut world = World::new(4, 4, 8);
		world.set(0, 0, 0, Tile::Stone);
		world.set(0, 0, 1, Tile::water_default());
		world.set(0, 0, 5, Tile::water_default());
		pass_gravity(&mut world);
		assert!(world.get(0, 0, 2).is_water());
		assert!(world.get(0, 0, 1).is_water());
		assert!(world.get(0, 0, 5).is_air());
	}

	#[test]
	fn source_stays_and_water_falls() {
		let mut world = World::new(4, 4, 8);
		world.set(0, 0, 0, Tile::Stone);
		world.set(0, 0, 5, Tile::water_source());
		pass_gravity(&mut world);
		assert!(world.get(0, 0, 5).is_water());
		assert!(world.get(0, 0, 1).is_water());
		if let Tile::Water { is_source, .. } = world.get(0, 0, 5) {
			assert!(is_source);
		}
	}

	#[test]
	fn dirt_falls_when_unsupported() {
		let mut world = World::new(4, 4, 8);
		world.set(0, 0, 0, Tile::Stone);
		// Air at z=1, Dirt floating at z=2
		world.set(0, 0, 2, Tile::Dirt);
		pass_gravity(&mut world);
		assert_eq!(world.get(0, 0, 1), Tile::Dirt);
		assert!(world.get(0, 0, 2).is_air());
	}

	#[test]
	fn sand_falls_through_air() {
		let mut world = World::new(4, 4, 8);
		world.set(0, 0, 5, Tile::Sand);
		pass_gravity(&mut world);
		assert_eq!(world.get(0, 0, 0), Tile::Sand);
		assert!(world.get(0, 0, 5).is_air());
	}

	#[test]
	fn stone_does_not_fall() {
		let mut world = World::new(4, 4, 8);
		// Stone floating at z=3 with air below
		world.set(0, 0, 3, Tile::Stone);
		pass_gravity(&mut world);
		assert_eq!(world.get(0, 0, 3), Tile::Stone);
		assert!(world.get(0, 0, 0).is_air());
	}

	#[test]
	fn stacked_solids_fall_together() {
		let mut world = World::new(4, 4, 8);
		world.set(0, 0, 0, Tile::Stone);
		// Air at z=1,2 then Dirt+Grass floating at z=3,4
		world.set(0, 0, 3, Tile::Dirt);
		world.set(0, 0, 4, Tile::Grass);
		pass_gravity(&mut world);
		// Bottom-to-top: Dirt(z=3) falls to z=1, Grass(z=4) falls to z=2
		assert_eq!(world.get(0, 0, 1), Tile::Dirt);
		assert_eq!(world.get(0, 0, 2), Tile::Grass);
	}
}
