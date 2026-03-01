pub mod erosion;
pub mod evaporation;
pub mod flow;
pub mod mass_erosion;
pub mod mass_evaporation;
pub mod source;
pub mod spread;

use crate::world::World;

pub fn tick(world: &mut World) {
	crate::world::gravity::pass_gravity(world);
	spread::pass_spread(world);
	erosion::pass_erosion(world);
	evaporation::pass_evaporation(world);
	let sources: Vec<_> = world.sources().to_vec();
	source::pass_source(world, &sources);
	world.sync_tiles_cache();
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tile::Tile;
	use crate::world::World;

	#[test]
	fn tick_moves_water_down() {
		let mut world = World::new(4, 4, 8);
		// Build a fully walled column with stone floor
		world.set(0, 0, 0, Tile::Stone);
		for z in 0..8 {
			for x in 0..4 {
				for y in 0..4 {
					if x == 0 && y == 0 {
						continue;
					}
					world.set(x, y, z, Tile::Stone);
				}
			}
		}
		// Stack three water blocks so the lower two have pressure and won't evaporate
		world.set(0, 0, 4, Tile::water_default());
		world.set(0, 0, 5, Tile::water_default());
		world.set(0, 0, 6, Tile::water_default());
		tick(&mut world);
		// Water should have fallen to z=1,2,3. Bottom two have pressure.
		assert!(world.get(0, 0, 1).is_water());
		assert!(world.get(0, 0, 2).is_water());
	}
}
