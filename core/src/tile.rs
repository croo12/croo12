#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TileType {
	Air = 0,
	Grass = 1,
	Dirt = 2,
	Stone = 3,
	Water = 4,
	Sand = 5,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Tile {
	pub tile_type: TileType,
	pub level: u8,
	pub moisture: u8,
	pub variant: u8,
}

impl Tile {
	pub fn new(tile_type: TileType) -> Self {
		Tile {
			tile_type,
			level: 8,
			moisture: 0,
			variant: 0,
		}
	}

	pub fn is_solid(&self) -> bool {
		!matches!(self.tile_type, TileType::Air | TileType::Water)
	}

	pub fn is_erodible(&self) -> bool {
		matches!(self.tile_type, TileType::Grass | TileType::Dirt | TileType::Sand)
	}

	pub fn pack(&self) -> u16 {
		let t = self.tile_type as u16;
		let l = self.level as u16 & 0x0F;
		let v = self.variant as u16 & 0x03;
		(t << 12) | (l << 8) | (v << 6)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn tile_default_is_air_level_8() {
		let tile = Tile::new(TileType::Air);
		assert_eq!(tile.tile_type, TileType::Air);
		assert_eq!(tile.level, 8);
		assert_eq!(tile.moisture, 0);
		assert_eq!(tile.variant, 0);
	}

	#[test]
	fn tile_is_solid() {
		assert!(!Tile::new(TileType::Air).is_solid());
		assert!(!Tile::new(TileType::Water).is_solid());
		assert!(Tile::new(TileType::Stone).is_solid());
		assert!(Tile::new(TileType::Grass).is_solid());
		assert!(Tile::new(TileType::Dirt).is_solid());
		assert!(Tile::new(TileType::Sand).is_solid());
	}

	#[test]
	fn tile_is_erodible() {
		assert!(Tile::new(TileType::Grass).is_erodible());
		assert!(Tile::new(TileType::Dirt).is_erodible());
		assert!(Tile::new(TileType::Sand).is_erodible());
		assert!(!Tile::new(TileType::Air).is_erodible());
		assert!(!Tile::new(TileType::Water).is_erodible());
		assert!(!Tile::new(TileType::Stone).is_erodible());
	}

	#[test]
	fn tile_pack_roundtrip() {
		let tile = Tile {
			tile_type: TileType::Grass,
			level: 5,
			moisture: 3,
			variant: 2,
		};
		let packed = tile.pack();
		let type_bits = (packed >> 12) & 0x0F;
		let level_bits = (packed >> 8) & 0x0F;
		let variant_bits = (packed >> 6) & 0x03;
		assert_eq!(type_bits, TileType::Grass as u16);
		assert_eq!(level_bits, 5);
		assert_eq!(variant_bits, 2);
	}
}
