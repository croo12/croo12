use crate::tile::{FlowDir, Tile};
use super::World;

pub fn pass_gravity(world: &mut World) {
	let w = world.width();
	let d = world.depth();
	let h = world.height();

	// Iterate top-to-bottom; water falls to the lowest air cell instantly
	for z in (1..h).rev() {
		for y in 0..d {
			for x in 0..w {
				let tile = world.get(x, y, z);
				if let Tile::Water {
					is_source,
					sediment,
					..
				} = tile
				{
					if !world.get(x, y, z - 1).is_air() {
						continue;
					}

					// Find the lowest air cell in this column
					let mut target_z = z - 1;
					while target_z > 0 && world.get(x, y, target_z - 1).is_air() {
						target_z -= 1;
					}

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
		// Falls instantly to z=1 (above Stone)
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
		// z=5 water falls to z=2 (above existing water at z=1)
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
		// Source stays, emitted water falls to z=1
		assert!(world.get(0, 0, 5).is_water());
		assert!(world.get(0, 0, 1).is_water());
		if let Tile::Water { is_source, .. } = world.get(0, 0, 5) {
			assert!(is_source);
		}
	}

	#[test]
	fn water_falls_to_z0_if_no_ground() {
		let mut world = World::new(4, 4, 8);
		world.set(0, 0, 6, Tile::water_default());
		pass_gravity(&mut world);
		assert!(world.get(0, 0, 0).is_water());
		assert!(world.get(0, 0, 6).is_air());
	}
}
