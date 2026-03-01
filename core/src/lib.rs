mod terrain;
mod tile;
mod water;
mod render;
mod world;

use std::cell::UnsafeCell;
use wasm_bindgen::prelude::*;
use water::cellular::CellularWaterSimulator;
use world::World;

type GameWorld = World<CellularWaterSimulator>;

struct WorldHolder(UnsafeCell<Option<GameWorld>>);

// SAFETY: Wasm is single-threaded, no concurrent access possible.
unsafe impl Sync for WorldHolder {}

static WORLD: WorldHolder = WorldHolder(UnsafeCell::new(None));

fn with_world<T>(f: impl FnOnce(&GameWorld) -> T, default: T) -> T {
	unsafe { (*WORLD.0.get()).as_ref().map_or(default, f) }
}

fn with_world_mut<T>(f: impl FnOnce(&mut GameWorld) -> T, default: T) -> T {
	unsafe { (*WORLD.0.get()).as_mut().map_or(default, f) }
}

#[wasm_bindgen]
pub fn greet() -> String {
	String::from("Hello from game-core!")
}

#[wasm_bindgen]
pub fn create_world(width: usize, depth: usize, height: usize, seed: u32) {
	let simulator = CellularWaterSimulator::new();
	let mut w = World::new(width, depth, height, simulator);
	terrain::generate_terrain(&mut w, seed);
	w.sync_tiles_cache();
	unsafe {
		*WORLD.0.get() = Some(w);
	}
}

// --- Tiles ---
#[wasm_bindgen]
pub fn world_tiles_ptr() -> *const u16 {
	with_world(|w| w.tiles_cache_ptr(), std::ptr::null())
}

#[wasm_bindgen]
pub fn world_tiles_len() -> usize {
	with_world(|w| w.tiles_cache_len(), 0)
}

#[wasm_bindgen]
pub fn world_width() -> usize {
	with_world(|w| w.width(), 0)
}

#[wasm_bindgen]
pub fn world_depth() -> usize {
	with_world(|w| w.depth(), 0)
}

#[wasm_bindgen]
pub fn world_height() -> usize {
	with_world(|w| w.height(), 0)
}

// --- Water ---
#[wasm_bindgen]
pub fn tick_water() {
	with_world_mut(|w| w.tick_water(), ());
}

#[wasm_bindgen]
pub fn place_water(x: usize, y: usize, z: usize, level: u8) {
	with_world_mut(|w| w.place_water(x, y, z, level), ());
}

#[wasm_bindgen]
pub fn remove_water(x: usize, y: usize, z: usize) {
	with_world_mut(|w| w.remove_water(x, y, z), ());
}

#[wasm_bindgen]
pub fn water_levels_ptr() -> *const u8 {
	with_world(|w| w.water_levels_ptr(), std::ptr::null())
}

#[wasm_bindgen]
pub fn water_levels_len() -> usize {
	with_world(|w| w.water_levels_len(), 0)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn greet_returns_message() {
		assert_eq!(greet(), "Hello from game-core!");
	}
}
