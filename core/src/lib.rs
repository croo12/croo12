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
pub fn tick_water() {
	with_world_mut(|w| water::tick(w));
}

#[wasm_bindgen]
pub fn place_water(x: usize, y: usize, z: usize) {
	with_world_mut(|w| {
		w.set(x, y, z, tile::Tile::water_default());
		w.sync_tiles_cache();
	});
}

#[wasm_bindgen]
pub fn place_water_source(x: usize, y: usize, z: usize) {
	with_world_mut(|w| {
		w.set(x, y, z, tile::Tile::water_source());
		w.add_source(x, y, z);
		w.sync_tiles_cache();
	});
}

#[wasm_bindgen]
pub fn remove_water(x: usize, y: usize, z: usize) {
	with_world_mut(|w| {
		w.set(x, y, z, tile::Tile::Air);
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
}
