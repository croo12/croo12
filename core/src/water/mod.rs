pub mod flow;
pub mod mass_erosion;
pub mod mass_evaporation;
pub mod weather;

use crate::world::World;

pub fn tick(world: &mut World) {
	// Solid gravity (sand, dirt falling) - reuse existing
	crate::world::gravity::pass_gravity(world);

	// Mass-based water flow
	flow::pass_flow(world);

	// Source replenishment
	let sources: Vec<_> = world.sources().to_vec();
	for &(sx, sy, sz) in &sources {
		let idx = world.index(sx, sy, sz);
		world.water_mass[idx] = world.water_mass[idx].saturating_add(50);
	}

	// Erosion & deposition (uses outflow data from flow pass)
	mass_erosion::pass_erosion(world);

	// Evaporation
	mass_evaporation::pass_evaporation(world);

	// Weather: cloud movement/rain, then spawn check
	weather::pass_cloud_update(world);
	weather::pass_cloud_spawn(world);

	world.sync_cloud_buffer();
	world.sync_tiles_cache();
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tile::Tile;
	use crate::world::World;

	#[test]
	fn tick_source_produces_flowing_water() {
		let mut world = World::new(8, 8, 8);
		for x in 0..8 {
			for y in 0..8 {
				world.set(x, y, 0, Tile::Stone);
			}
		}
		world.add_source(4, 4, 1);
		world.set_water_mass(4, 4, 1, 255);

		for _ in 0..100 {
			tick(&mut world);
		}

		// Water should have spread significantly
		let total: u32 = (0..8)
			.flat_map(|x| (0..8).flat_map(move |y| (0..8).map(move |z| (x, y, z))))
			.map(|(x, y, z)| world.water_mass(x, y, z) as u32)
			.sum();
		assert!(total > 500, "Total water should be significant: {}", total);

		// Water should exist at positions away from source
		let distant_water = world.water_mass(0, 4, 1) + world.water_mass(4, 0, 1);
		assert!(distant_water > 0, "Water should reach distant cells");
	}

	#[test]
	fn tick_weather_cycle_produces_rain() {
		let mut world = World::new(8, 8, 8);
		for x in 0..8 {
			for y in 0..8 {
				world.set(x, y, 0, Tile::Stone);
			}
		}
		world.atmospheric_moisture = 10000;

		for _ in 0..50 {
			tick(&mut world);
		}

		let total: u32 = (0..8)
			.flat_map(|x| (0..8).flat_map(move |y| (0..8).map(move |z| (x, y, z))))
			.map(|(x, y, z)| world.water_mass(x, y, z) as u32)
			.sum();
		assert!(total > 0, "Rain should have added water: {}", total);
	}
}
