pub mod cellular;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaterCell {
	pub level: u8,
	pub is_source: bool,
	pub sediment: u8,
}

impl WaterCell {
	pub const EMPTY: Self = Self {
		level: 0,
		is_source: false,
		sediment: 0,
	};

	pub fn pack(self) -> u8 {
		let source_bit = if self.is_source { 0x80 } else { 0 };
		source_bit | (self.level & 0x7F)
	}

	pub fn unpack(packed: u8) -> Self {
		Self {
			level: packed & 0x7F,
			is_source: packed & 0x80 != 0,
			sediment: 0,
		}
	}
}

pub struct WaterState {
	width: usize,
	depth: usize,
	height: usize,
	cells: Vec<WaterCell>,
	levels_cache: Vec<u8>,
}

impl WaterState {
	pub fn new(width: usize, depth: usize, height: usize) -> Self {
		let size = width * depth * height;
		Self {
			width,
			depth,
			height,
			cells: vec![WaterCell::EMPTY; size],
			levels_cache: vec![0u8; size],
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

	fn index(&self, x: usize, y: usize, z: usize) -> usize {
		x + y * self.width + z * self.width * self.depth
	}

	pub fn get(&self, x: usize, y: usize, z: usize) -> WaterCell {
		self.cells[self.index(x, y, z)]
	}

	pub fn set(&mut self, x: usize, y: usize, z: usize, cell: WaterCell) {
		let idx = self.index(x, y, z);
		self.cells[idx] = cell;
	}

	pub fn sync_levels_cache(&mut self) {
		for (i, cell) in self.cells.iter().enumerate() {
			self.levels_cache[i] = cell.pack();
		}
	}

	pub fn levels(&self) -> &[u8] {
		&self.levels_cache
	}

	pub fn levels_ptr(&self) -> *const u8 {
		self.levels_cache.as_ptr()
	}

	pub fn levels_len(&self) -> usize {
		self.levels_cache.len()
	}

	pub fn snapshot_cells(&self) -> Vec<WaterCell> {
		self.cells.clone()
	}

	pub fn apply_cells(&mut self, cells: Vec<WaterCell>) {
		self.cells = cells;
	}

	pub fn clear_sources(&mut self) {
		for cell in &mut self.cells {
			cell.is_source = false;
		}
	}
}

use crate::tile::Tile;

pub trait WaterSimulator {
	fn tick(&mut self, state: &mut WaterState, terrain: &mut [Tile]);
	fn place_water(&mut self, state: &mut WaterState, x: usize, y: usize, z: usize, level: u8);
	fn remove_water(&mut self, state: &mut WaterState, x: usize, y: usize, z: usize);
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn water_state_new_initializes_empty() {
		let state = WaterState::new(4, 4, 4);
		assert_eq!(state.width(), 4);
		assert_eq!(state.depth(), 4);
		assert_eq!(state.height(), 4);
		assert_eq!(state.get(0, 0, 0).level, 0);
		assert!(!state.get(0, 0, 0).is_source);
	}

	#[test]
	fn water_state_set_and_get() {
		let mut state = WaterState::new(4, 4, 4);
		state.set(
			1,
			2,
			3,
			WaterCell {
				level: 5,
				is_source: true,
				sediment: 0,
			},
		);
		let cell = state.get(1, 2, 3);
		assert_eq!(cell.level, 5);
		assert!(cell.is_source);
	}

	#[test]
	fn water_state_pack_unpack_roundtrip() {
		let cell = WaterCell {
			level: 8,
			is_source: true,
			sediment: 0,
		};
		let packed = cell.pack();
		let unpacked = WaterCell::unpack(packed);
		assert_eq!(unpacked.level, 8);
		assert!(unpacked.is_source);
	}

	#[test]
	fn water_levels_returns_packed_data() {
		let mut state = WaterState::new(2, 2, 2);
		state.set(
			0,
			0,
			0,
			WaterCell {
				level: 4,
				is_source: false,
				sediment: 0,
			},
		);
		state.set(
			1,
			0,
			0,
			WaterCell {
				level: 8,
				is_source: true,
				sediment: 0,
			},
		);
		state.sync_levels_cache();
		let levels = state.levels();
		assert_eq!(levels[0], 4);
		assert_eq!(levels[1], 0x80 | 8);
	}
}
