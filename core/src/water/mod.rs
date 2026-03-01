pub mod erosion;
pub mod gravity;
pub mod source;
pub mod spread;

use crate::world::World;

pub fn tick(world: &mut World) {
	gravity::pass_gravity(world);
	spread::pass_spread(world);
	erosion::pass_erosion(world);
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
		world.set(0, 0, 0, Tile::Stone); // ground
		world.set(0, 0, 2, Tile::water_default()); // water at z=2
		tick(&mut world);
		assert!(world.get(0, 0, 1).is_water()); // moved to z=1
		assert!(world.get(0, 0, 2).is_air()); // vacated
	}
}
