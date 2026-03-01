use crate::tile::Tile;

pub struct World {
	width: usize,
	depth: usize,
	height: usize,
	tiles: Vec<Tile>,
	tiles_cache: Vec<u8>,
	sources: Vec<(usize, usize, usize)>,
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
			sources: Vec::new(),
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
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tile::{FlowDir, Tile};

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
		world.set(
			1,
			0,
			0,
			Tile::Water {
				is_source: true,
				sediment: 0,
				velocity: 0,
				direction: FlowDir::East,
			},
		);
		world.sync_tiles_cache();
		assert_eq!(world.tiles_cache()[0], Tile::Grass.pack());
		assert_eq!(world.tiles_cache()[1] & 0x07, 5); // Water type
		assert_eq!((world.tiles_cache()[1] >> 6) & 1, 1); // is_source
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
}
