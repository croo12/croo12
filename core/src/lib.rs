mod render;
mod terrain;
mod tile;
mod water;
mod world;

use std::cell::UnsafeCell;
use wasm_bindgen::prelude::*;
use world::World;

struct WorldHolder(UnsafeCell<Option<World>>);
unsafe impl Sync for WorldHolder {}

static WORLD: WorldHolder = WorldHolder(UnsafeCell::new(None));

fn with_world<T>(f: impl FnOnce(&World) -> T) -> T {
	unsafe {
		let world = &*WORLD.0.get();
		f(world.as_ref().expect("World not initialized"))
	}
}

fn with_world_mut<T>(f: impl FnOnce(&mut World) -> T) -> T {
	unsafe {
		let world = &mut *WORLD.0.get();
		f(world.as_mut().expect("World not initialized"))
	}
}

#[wasm_bindgen]
pub fn greet() -> String {
	"Hello from game_core!".to_string()
}

#[wasm_bindgen]
pub fn create_world(width: usize, depth: usize, height: usize, seed: u32) {
	let mut world = World::new(width, depth, height);
	terrain::generate_terrain(&mut world, seed);
	world.sync_tiles_cache();
	unsafe {
		*WORLD.0.get() = Some(world);
	}
}

#[wasm_bindgen]
pub fn world_width() -> usize {
	with_world(|w| w.width())
}

#[wasm_bindgen]
pub fn world_depth() -> usize {
	with_world(|w| w.depth())
}

#[wasm_bindgen]
pub fn world_height() -> usize {
	with_world(|w| w.height())
}

#[wasm_bindgen]
pub fn world_tiles_ptr() -> *const u8 {
	with_world(|w| w.tiles_cache_ptr())
}

#[wasm_bindgen]
pub fn world_tiles_len() -> usize {
	with_world(|w| w.tiles_cache_len())
}

#[wasm_bindgen]
pub fn world_water_ptr() -> *const u8 {
	with_world(|w| w.water_mass_ptr())
}

#[wasm_bindgen]
pub fn world_water_len() -> usize {
	with_world(|w| w.water_mass_len())
}

#[wasm_bindgen]
pub fn world_clouds_ptr() -> *const f32 {
	with_world(|w| w.cloud_buffer_ptr())
}

#[wasm_bindgen]
pub fn world_clouds_len() -> usize {
	with_world(|w| w.cloud_buffer_len())
}

#[wasm_bindgen]
pub fn world_clouds_count() -> usize {
	with_world(|w| w.clouds_count())
}

#[wasm_bindgen]
pub fn world_atmospheric_moisture() -> u32 {
	with_world(|w| w.atmospheric_moisture)
}

#[wasm_bindgen]
pub fn tick_water() {
	with_world_mut(|w| water::tick(w));
}

#[wasm_bindgen]
pub fn place_water(x: usize, y: usize, z: usize) {
	with_world_mut(|w| {
		w.set_water_mass(x, y, z, 255);
		w.sync_tiles_cache();
	});
}

#[wasm_bindgen]
pub fn place_water_source(x: usize, y: usize, z: usize) {
	with_world_mut(|w| {
		w.set_water_mass(x, y, z, 255);
		w.add_source(x, y, z);
		w.sync_tiles_cache();
	});
}

#[wasm_bindgen]
pub fn remove_water(x: usize, y: usize, z: usize) {
	with_world_mut(|w| {
		w.set_water_mass(x, y, z, 0);
		w.sync_tiles_cache();
	});
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn greet_returns_message() {
		assert_eq!(greet(), "Hello from game_core!");
	}

	#[test]
	fn cli_simulation_debug() {
		use render::ascii::AsciiRenderer;

		let mut world = World::new(16, 16, 64);
		terrain::generate_terrain(&mut world, 77);

		println!("\n=== SOURCES: {:?} ===", world.sources());

		let source_y = world.sources().first().map(|s| s.1).unwrap_or(8);
		let side = AsciiRenderer::side_view(source_y);

		println!("=== INITIAL STATE (side y={}) ===", source_y);
		println!("{}", side.render(&world));

		// Count total water mass
		let count_water_mass = |w: &World| -> u32 {
			let mut total = 0u32;
			for z in 0..w.height() {
				for y in 0..w.depth() {
					for x in 0..w.width() {
						total += w.water_mass(x, y, z) as u32;
					}
				}
			}
			total
		};

		println!("Initial water mass: {}", count_water_mass(&world));

		for t in 1..=200 {
			water::tick(&mut world);
			if t % 50 == 0 {
				println!(
					"\n=== TICK {} | water mass: {} | atmos: {} | clouds: {} ===",
					t, count_water_mass(&world), world.atmospheric_moisture, world.clouds.len()
				);

				for tz in [37, 36, 35, 34, 32, 30] {
					if tz < world.height() {
						let top = AsciiRenderer::top_down(tz);
						println!("{}", top.render(&world));
					}
				}
			}
		}
	}
}
