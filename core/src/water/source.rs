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
