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
		use tile::Tile;

		let mut world = World::new(16, 16, 64);
		terrain::generate_terrain(&mut world, 77);

		println!("\n=== SOURCES: {:?} ===", world.sources());

		// Snapshot initial terrain for diff
		let initial_tiles: Vec<Tile> = (0..64)
			.flat_map(|z| (0..16).flat_map(move |y| (0..16).map(move |x| (x, y, z))))
			.map(|(x, y, z)| world.get(x, y, z))
			.collect();

		let source = world.sources().first().copied().unwrap_or((8, 8, 32));
		let source_y = source.1;
		let side = AsciiRenderer::side_view(source_y);

		println!("=== INITIAL STATE (side y={}) ===", source_y);
		println!("{}", side.render(&world));

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

		let count_tile_changes = |w: &World, initial: &[Tile]| -> (usize, usize, usize) {
			let mut eroded = 0usize; // solid -> air
			let mut deposited = 0usize; // air -> sand
			let mut other = 0usize;
			let mut i = 0;
			for z in 0..64 {
				for y in 0..16 {
					for x in 0..16 {
						let old = initial[i];
						let new = w.get(x, y, z);
						if old != new {
							if old.is_solid() && new.is_air() {
								eroded += 1;
							} else if old.is_air() && new == Tile::Sand {
								deposited += 1;
							} else {
								other += 1;
							}
						}
						i += 1;
					}
				}
			}
			(eroded, deposited, other)
		};

		let count_total_sediment = |w: &World| -> u32 {
			let mut total = 0u32;
			for z in 0..w.height() {
				for y in 0..w.depth() {
					for x in 0..w.width() {
						total += w.water_sediment(x, y, z) as u32;
					}
				}
			}
			total
		};

		let max_outflow = |w: &World| -> (u16, usize, usize, usize) {
			let mut max_f = 0u16;
			let mut mx = 0;
			let mut my = 0;
			let mut mz = 0;
			for z in 0..w.height() {
				for y in 0..w.depth() {
					for x in 0..w.width() {
						let idx = w.index(x, y, z);
						let f = w.water_outflow(idx);
						if f > max_f {
							max_f = f;
							mx = x;
							my = y;
							mz = z;
						}
					}
				}
			}
			(max_f, mx, my, mz)
		};

		println!("Initial water mass: {}", count_water_mass(&world));

		for t in 1..=1000 {
			water::tick(&mut world);
			if t % 200 == 0 || t == 50 || t == 100 {
				let (eroded, deposited, other_changes) =
					count_tile_changes(&world, &initial_tiles);
				let (mf, mx, my, mz) = max_outflow(&world);
				println!(
					"\n=== TICK {} | water: {} | atmos: {} | clouds: {} ===",
					t,
					count_water_mass(&world),
					world.atmospheric_moisture,
					world.clouds.len()
				);
				println!(
					"    eroded: {} | deposited: {} | other: {} | sediment: {}",
					eroded, deposited, other_changes, count_total_sediment(&world)
				);
				println!(
					"    max outflow: {} at ({},{},{})",
					mf, mx, my, mz
				);

				println!("{}", side.render(&world));

				// Show top-down at source z level and below
				let sz = source.2;
				for tz in [sz, sz - 1, sz - 2, sz - 4, sz - 8] {
					if tz < world.height() {
						let top = AsciiRenderer::top_down(tz);
						println!("{}", top.render(&world));
					}
				}
			}
		}
	}
}
