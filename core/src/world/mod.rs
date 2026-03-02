pub mod gravity;

pub(crate) const CLOUD_THRESHOLD: u32 = 10000;
pub(crate) const CLOUD_WATER: u32 = 8000;
pub(crate) const DROPS_PER_TICK: u32 = 3;
pub(crate) const RAIN_MASS_PER_DROP: u8 = 2;
pub(crate) const CLOUD_SPEED: f32 = 0.3;
pub(crate) const MAX_CLOUDS: usize = 3;

#[derive(Clone, Debug)]
pub(crate) struct Cloud {
	pub x: f32,
	pub y: f32,
	pub dx: f32,
	pub dy: f32,
	pub water: u32,
	pub radius: f32,
}

use crate::tile::Tile;

pub struct World {
	width: usize,
	depth: usize,
	height: usize,
	tiles: Vec<Tile>,
	tiles_cache: Vec<u8>,
	// Water layer fields (mass-based cellular automata)
	pub(crate) water_mass: Vec<u8>,
	pub(crate) water_sediment: Vec<u8>,
	pub(crate) water_snapshot: Vec<u8>,
	pub(crate) mass_delta: Vec<i16>,
	pub(crate) sediment_delta: Vec<i16>,
	pub(crate) water_outflow: Vec<u16>,
	pub(crate) flow_dir: Vec<u8>,
	pub(crate) soil_moisture: Vec<u8>,
	pub(crate) moisture_delta: Vec<i16>,
	sources: Vec<(usize, usize, usize)>,
	pub(crate) atmospheric_moisture: u32,
	pub(crate) clouds: Vec<Cloud>,
	cloud_buffer: Vec<f32>,
}

impl World {
	pub fn new(width: usize, depth: usize, height: usize) -> Self {
		let size = width * depth * height;
		Self {
			width,
			depth,
			height,
			tiles: vec![Tile::Air; size],
			tiles_cache: vec![0u8; size],
			water_mass: vec![0u8; size],
			water_sediment: vec![0u8; size],
			water_snapshot: vec![0u8; size],
			mass_delta: vec![0i16; size],
			sediment_delta: vec![0i16; size],
			water_outflow: vec![0u16; size],
			flow_dir: vec![0u8; size],
			soil_moisture: vec![0u8; size],
			moisture_delta: vec![0i16; size],
			sources: Vec::new(),
			atmospheric_moisture: 0,
			clouds: Vec::new(),
			cloud_buffer: Vec::new(),
		}
	}

	pub fn width(&self) -> usize {
		self.width
	}

	pub fn depth(&self) -> usize {
		self.depth
	}

	pub fn height(&self) -> usize {
		self.height
	}

	pub fn index(&self, x: usize, y: usize, z: usize) -> usize {
		x + y * self.width + z * self.width * self.depth
	}

	pub fn get(&self, x: usize, y: usize, z: usize) -> Tile {
		self.tiles[self.index(x, y, z)]
	}

	pub fn set(&mut self, x: usize, y: usize, z: usize, tile: Tile) {
		let idx = self.index(x, y, z);
		self.tiles[idx] = tile;
	}

	pub fn tiles(&self) -> &[Tile] {
		&self.tiles
	}

	pub fn tiles_mut(&mut self) -> &mut [Tile] {
		&mut self.tiles
	}

	pub fn sync_tiles_cache(&mut self) {
		for (i, tile) in self.tiles.iter().enumerate() {
			self.tiles_cache[i] = tile.pack();
		}
	}

	pub fn tiles_cache(&self) -> &[u8] {
		&self.tiles_cache
	}

	pub fn tiles_cache_ptr(&self) -> *const u8 {
		self.tiles_cache.as_ptr()
	}

	pub fn tiles_cache_len(&self) -> usize {
		self.tiles_cache.len()
	}

	pub fn in_bounds(&self, x: usize, y: usize, z: usize) -> bool {
		x < self.width && y < self.depth && z < self.height
	}

	pub fn sources(&self) -> &[(usize, usize, usize)] {
		&self.sources
	}

	pub fn add_source(&mut self, x: usize, y: usize, z: usize) {
		self.sources.push((x, y, z));
	}

	pub fn clear_sources(&mut self) {
		self.sources.clear();
	}

	// --- Water layer accessors ---

	pub fn water_mass(&self, x: usize, y: usize, z: usize) -> u8 {
		self.water_mass[self.index(x, y, z)]
	}

	pub fn set_water_mass(&mut self, x: usize, y: usize, z: usize, mass: u8) {
		let idx = self.index(x, y, z);
		self.water_mass[idx] = mass;
	}

	pub fn water_mass_ptr(&self) -> *const u8 {
		self.water_mass.as_ptr()
	}

	pub fn water_mass_len(&self) -> usize {
		self.water_mass.len()
	}

	pub fn water_sediment(&self, x: usize, y: usize, z: usize) -> u8 {
		self.water_sediment[self.index(x, y, z)]
	}

	pub fn set_water_sediment(&mut self, x: usize, y: usize, z: usize, sed: u8) {
		let idx = self.index(x, y, z);
		self.water_sediment[idx] = sed;
	}

	// --- Soil moisture accessors ---

	pub fn soil_moisture(&self, x: usize, y: usize, z: usize) -> u8 {
		self.soil_moisture[self.index(x, y, z)]
	}

	pub fn set_soil_moisture(&mut self, x: usize, y: usize, z: usize, moisture: u8) {
		let idx = self.index(x, y, z);
		self.soil_moisture[idx] = moisture;
	}

	pub fn soil_moisture_ptr(&self) -> *const u8 {
		self.soil_moisture.as_ptr()
	}

	pub fn soil_moisture_len(&self) -> usize {
		self.soil_moisture.len()
	}

	pub fn apply_moisture_deltas(&mut self) {
		for i in 0..self.soil_moisture.len() {
			if self.moisture_delta[i] != 0 {
				let cap = self.tiles[i].moisture_capacity() as i16;
				let new_val =
					(self.soil_moisture[i] as i16 + self.moisture_delta[i]).clamp(0, cap);
				self.soil_moisture[i] = new_val as u8;
				self.moisture_delta[i] = 0;
			}
		}
	}

	// --- Delta / outflow helpers ---

	pub fn mass_delta_ref(&self) -> &[i16] {
		&self.mass_delta
	}

	pub fn record_flow(&mut self, from: usize, to: usize, amount: u16, sed_amount: i16) {
		self.mass_delta[from] -= amount as i16;
		self.mass_delta[to] += amount as i16;
		self.water_outflow[from] += amount;
		if sed_amount != 0 {
			self.sediment_delta[from] -= sed_amount;
			self.sediment_delta[to] += sed_amount;
		}
	}

	pub fn apply_water_deltas(&mut self) {
		for i in 0..self.water_mass.len() {
			if self.mass_delta[i] != 0 {
				let new_mass = (self.water_mass[i] as i16 + self.mass_delta[i]).clamp(0, 255);
				self.water_mass[i] = new_mass as u8;
				self.mass_delta[i] = 0;
			}
			if self.sediment_delta[i] != 0 {
				let new_sed =
					(self.water_sediment[i] as i16 + self.sediment_delta[i]).clamp(0, 255);
				self.water_sediment[i] = new_sed as u8;
				self.sediment_delta[i] = 0;
			}
		}
	}

	pub fn clear_outflow(&mut self) {
		for v in self.water_outflow.iter_mut() {
			*v = 0;
		}
	}

	pub fn water_outflow(&self, idx: usize) -> u16 {
		self.water_outflow[idx]
	}

	// --- Cloud / weather accessors ---

	pub fn clouds_count(&self) -> usize {
		self.clouds.len()
	}

	pub fn sync_cloud_buffer(&mut self) {
		self.cloud_buffer.clear();
		for c in &self.clouds {
			self.cloud_buffer.push(c.x);
			self.cloud_buffer.push(c.y);
			self.cloud_buffer.push(c.radius);
			self.cloud_buffer.push(c.water as f32);
		}
	}

	pub fn cloud_buffer_ptr(&self) -> *const f32 {
		self.cloud_buffer.as_ptr()
	}

	pub fn cloud_buffer_len(&self) -> usize {
		self.cloud_buffer.len()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tile::Tile;

	#[test]
	fn world_new_creates_air_grid() {
		let world = World::new(4, 4, 8);
		assert_eq!(world.width(), 4);
		assert_eq!(world.depth(), 4);
		assert_eq!(world.height(), 8);
		assert_eq!(world.get(0, 0, 0), Tile::Air);
	}

	#[test]
	fn world_set_and_get() {
		let mut world = World::new(4, 4, 8);
		world.set(1, 2, 3, Tile::Stone);
		assert_eq!(world.get(1, 2, 3), Tile::Stone);
		assert_eq!(world.get(0, 0, 0), Tile::Air);
	}

	#[test]
	fn world_sync_cache_packs_correctly() {
		let mut world = World::new(4, 4, 8);
		world.set(0, 0, 0, Tile::Grass);
		world.set(1, 0, 0, Tile::Stone);
		world.sync_tiles_cache();
		assert_eq!(world.tiles_cache()[0], Tile::Grass.pack());
		assert_eq!(world.tiles_cache()[1], Tile::Stone.pack());
	}

	#[test]
	fn world_tiles_cache_ptr_and_len() {
		let world = World::new(4, 4, 8);
		assert_eq!(world.tiles_cache_len(), 4 * 4 * 8);
		assert!(!world.tiles_cache_ptr().is_null());
	}

	#[test]
	fn world_tiles_mut_allows_modification() {
		let mut world = World::new(4, 4, 8);
		world.tiles_mut()[0] = Tile::Stone;
		assert_eq!(world.get(0, 0, 0), Tile::Stone);
	}

	#[test]
	fn water_mass_set_and_get() {
		let mut w = World::new(4, 4, 4);
		w.set_water_mass(1, 1, 1, 100);
		assert_eq!(w.water_mass(1, 1, 1), 100);
		assert_eq!(w.water_mass(0, 0, 0), 0);
	}

	#[test]
	fn record_flow_updates_deltas() {
		let mut w = World::new(4, 4, 4);
		w.set_water_mass(0, 0, 0, 200);
		let from = w.index(0, 0, 0);
		let to = w.index(1, 0, 0);
		w.record_flow(from, to, 50, 0);
		assert_eq!(w.mass_delta_ref()[from], -50);
		assert_eq!(w.mass_delta_ref()[to], 50);
	}

	#[test]
	fn apply_deltas_clamps_and_resets() {
		let mut w = World::new(4, 4, 4);
		w.set_water_mass(0, 0, 0, 200);
		let idx = w.index(0, 0, 0);
		w.mass_delta[idx] = 100; // would exceed 255
		w.apply_water_deltas();
		assert_eq!(w.water_mass(0, 0, 0), 255); // clamped
		assert_eq!(w.mass_delta_ref()[idx], 0); // reset
	}

	#[test]
	fn soil_moisture_set_and_get() {
		let mut w = World::new(4, 4, 4);
		w.set(1, 1, 1, Tile::Dirt);
		w.set_soil_moisture(1, 1, 1, 50);
		assert_eq!(w.soil_moisture(1, 1, 1), 50);
		assert_eq!(w.soil_moisture(0, 0, 0), 0);
	}

	#[test]
	fn apply_moisture_deltas_clamps_to_capacity() {
		let mut w = World::new(4, 4, 4);
		w.set(0, 0, 0, Tile::Sand); // capacity = 48
		w.soil_moisture[0] = 40;
		w.moisture_delta[0] = 100; // would exceed 48
		w.apply_moisture_deltas();
		assert_eq!(w.soil_moisture(0, 0, 0), 48); // clamped
		assert_eq!(w.moisture_delta[0], 0); // reset
	}

	#[test]
	fn apply_moisture_deltas_clamps_to_zero() {
		let mut w = World::new(4, 4, 4);
		w.set(0, 0, 0, Tile::Dirt);
		w.soil_moisture[0] = 10;
		w.moisture_delta[0] = -50;
		w.apply_moisture_deltas();
		assert_eq!(w.soil_moisture(0, 0, 0), 0);
	}

	#[test]
	fn world_new_has_empty_weather_state() {
		let w = World::new(4, 4, 4);
		assert_eq!(w.atmospheric_moisture, 0);
		assert!(w.clouds.is_empty());
		assert_eq!(w.clouds_count(), 0);
	}

	#[test]
	fn sync_cloud_buffer_exports_data() {
		let mut w = World::new(4, 4, 4);
		w.clouds.push(Cloud {
			x: 1.5, y: 2.5, dx: 0.3, dy: 0.0,
			water: 8000, radius: 2.5,
		});
		w.sync_cloud_buffer();
		assert_eq!(w.cloud_buffer_len(), 4);
		assert_eq!(w.clouds_count(), 1);
	}
}
