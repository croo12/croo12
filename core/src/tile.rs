#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tile {
	Air,
	Grass,
	Dirt,
	Stone,
	Sand,
}

impl Tile {
	pub fn is_solid(&self) -> bool {
		matches!(self, Self::Grass | Self::Dirt | Self::Stone | Self::Sand)
	}

	pub fn is_erodible(&self) -> bool {
		matches!(self, Self::Grass | Self::Dirt | Self::Sand)
	}

	pub fn is_air(&self) -> bool {
		matches!(self, Self::Air)
	}

	pub fn falls(&self) -> bool {
		matches!(self, Self::Grass | Self::Dirt | Self::Sand)
	}

	/// Maximum soil moisture capacity (0-255)
	pub fn moisture_capacity(&self) -> u8 {
		match self {
			Self::Sand => 48,
			Self::Grass => 160,
			Self::Dirt => 128,
			Self::Air | Self::Stone => 0,
		}
	}

	/// Absorption rate per tick (surface water → soil moisture)
	pub fn absorb_rate(&self) -> u8 {
		match self {
			Self::Sand => 8,
			Self::Grass => 5,
			Self::Dirt => 2,
			Self::Air | Self::Stone => 0,
		}
	}

	/// Permeability for underground flow speed
	pub fn permeability(&self) -> u8 {
		match self {
			Self::Sand => 6,
			Self::Grass => 3,
			Self::Dirt => 1,
			Self::Air | Self::Stone => 0,
		}
	}

	/// Opacity for visibility scoring. 10 = fully opaque, 0 = transparent.
	pub fn opacity(&self) -> u8 {
		match self {
			Self::Air => 0,
			_ => 10,
		}
	}

	fn type_id(&self) -> u8 {
		match self {
			Self::Air => 0,
			Self::Grass => 1,
			Self::Dirt => 2,
			Self::Stone => 3,
			Self::Sand => 4,
		}
	}

	/// Pack for WASM export: u8
	/// Bits 0-2: tile_type (0-4)
	pub fn pack(&self) -> u8 {
		self.type_id() & 0x07
	}

	pub fn unpack(packed: u8) -> Self {
		let type_id = packed & 0x07;
		match type_id {
			0 => Self::Air,
			1 => Self::Grass,
			2 => Self::Dirt,
			3 => Self::Stone,
			4 => Self::Sand,
			_ => Self::Air,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn tile_is_solid() {
		assert!(!Tile::Air.is_solid());
		assert!(Tile::Grass.is_solid());
		assert!(Tile::Dirt.is_solid());
		assert!(Tile::Stone.is_solid());
		assert!(Tile::Sand.is_solid());
	}

	#[test]
	fn tile_is_erodible() {
		assert!(!Tile::Air.is_erodible());
		assert!(Tile::Grass.is_erodible());
		assert!(Tile::Dirt.is_erodible());
		assert!(!Tile::Stone.is_erodible());
		assert!(Tile::Sand.is_erodible());
	}

	#[test]
	fn tile_pack_roundtrip_solid() {
		assert_eq!(Tile::Air.pack(), 0);
		assert_eq!(Tile::unpack(Tile::Grass.pack()), Tile::Grass);
		assert_eq!(Tile::unpack(Tile::Stone.pack()), Tile::Stone);
	}

	#[test]
	fn tile_opacity() {
		assert_eq!(Tile::Air.opacity(), 0);
		assert_eq!(Tile::Grass.opacity(), 10);
	}

	#[test]
	fn tile_moisture_capacity() {
		assert_eq!(Tile::Sand.moisture_capacity(), 48);
		assert_eq!(Tile::Grass.moisture_capacity(), 160);
		assert_eq!(Tile::Dirt.moisture_capacity(), 128);
		assert_eq!(Tile::Stone.moisture_capacity(), 0);
		assert_eq!(Tile::Air.moisture_capacity(), 0);
	}

	#[test]
	fn tile_absorb_rate() {
		assert_eq!(Tile::Sand.absorb_rate(), 8);
		assert_eq!(Tile::Grass.absorb_rate(), 5);
		assert_eq!(Tile::Dirt.absorb_rate(), 2);
		assert_eq!(Tile::Stone.absorb_rate(), 0);
	}

	#[test]
	fn tile_permeability() {
		assert_eq!(Tile::Sand.permeability(), 6);
		assert_eq!(Tile::Grass.permeability(), 3);
		assert_eq!(Tile::Dirt.permeability(), 1);
		assert_eq!(Tile::Stone.permeability(), 0);
	}
}
