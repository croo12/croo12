use crate::tile::Tile;
use crate::world::World;

pub fn pass_source(world: &mut World, sources: &[(usize, usize, usize)]) {
	for &(x, y, z) in sources {
		let tile = world.get(x, y, z);
		match tile {
			Tile::Air => {
				world.set(x, y, z, Tile::water_source());
			}
			Tile::Water {
				sediment,
				velocity,
				direction,
				..
			} => {
				world.set(
					x,
					y,
					z,
					Tile::Water {
						is_source: true,
						sediment,
						velocity,
						direction,
					},
				);
			}
			_ => {} // don't overwrite solid
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tile::Tile;
	use crate::world::World;

	#[test]
	fn source_replenishes_when_empty() {
		let mut world = World::new(4, 4, 4);
		let sources = vec![(1, 1, 2)];
		// Source position is Air (water moved away)
		pass_source(&mut world, &sources);
		assert!(world.get(1, 1, 2).is_water());
		if let Tile::Water { is_source, .. } = world.get(1, 1, 2) {
			assert!(is_source);
		}
	}

	#[test]
	fn source_does_not_overwrite_solid() {
		let mut world = World::new(4, 4, 4);
		world.set(1, 1, 2, Tile::Stone);
		let sources = vec![(1, 1, 2)];
		pass_source(&mut world, &sources);
		assert_eq!(world.get(1, 1, 2), Tile::Stone); // not overwritten
	}

	#[test]
	fn source_does_not_overwrite_existing_water() {
		let mut world = World::new(4, 4, 4);
		world.set(1, 1, 2, Tile::water_default());
		let sources = vec![(1, 1, 2)];
		pass_source(&mut world, &sources);
		// Should mark as source
		if let Tile::Water { is_source, .. } = world.get(1, 1, 2) {
			assert!(is_source);
		}
	}
}
