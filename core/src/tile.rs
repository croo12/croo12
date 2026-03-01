#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FlowDir {
	None = 0,
	Down = 1,
	North = 2,
	South = 3,
	East = 4,
	West = 5,
}

impl FlowDir {
	pub fn from_u8(v: u8) -> Self {
		match v {
			0 => Self::None,
			1 => Self::Down,
			2 => Self::North,
			3 => Self::South,
			4 => Self::East,
			5 => Self::West,
			_ => Self::None,
		}
	}

	pub fn to_u8(self) -> u8 {
		self as u8
	}

}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tile {
	Air,
	Grass,
	Dirt,
	Stone,
	Sand,
	Water {
		is_source: bool,
		sediment: u8,
		velocity: u8,
		direction: FlowDir,
	},
}

impl Tile {
	pub fn water_default() -> Self {
		Self::Water {
			is_source: false,
			sediment: 0,
			velocity: 0,
			direction: FlowDir::None,
		}
	}

	pub fn water_source() -> Self {
		Self::Water {
			is_source: true,
			sediment: 0,
			velocity: 0,
			direction: FlowDir::None,
		}
	}

	pub fn is_solid(&self) -> bool {
		matches!(self, Self::Grass | Self::Dirt | Self::Stone | Self::Sand)
	}

	pub fn is_erodible(&self) -> bool {
		matches!(self, Self::Grass | Self::Dirt | Self::Sand)
	}

	pub fn is_water(&self) -> bool {
		matches!(self, Self::Water { .. })
	}

	pub fn is_air(&self) -> bool {
		matches!(self, Self::Air)
	}

	pub fn falls(&self) -> bool {
		matches!(self, Self::Grass | Self::Dirt | Self::Sand | Self::Water { .. })
	}

	/// Opacity for visibility scoring. 10 = fully opaque, 0 = transparent.
	pub fn opacity(&self) -> u8 {
		match self {
			Self::Air => 0,
			Self::Water { .. } => 3,
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
			Self::Water { .. } => 5,
		}
	}

	/// Pack for WASM export: u8
	/// Bits 0-2: tile_type (0-5)
	/// Bits 3-5: direction (0-5), Water only
	/// Bit 6: is_source, Water only
	/// Bit 7: unused
	pub fn pack(&self) -> u8 {
		let type_bits = self.type_id() & 0x07;
		match self {
			Self::Water {
				is_source,
				direction,
				..
			} => {
				let dir_bits = (direction.to_u8() & 0x07) << 3;
				let src_bit = if *is_source { 1 << 6 } else { 0 };
				type_bits | dir_bits | src_bit
			}
			_ => type_bits,
		}
	}

	pub fn unpack(packed: u8) -> Self {
		let type_id = packed & 0x07;
		match type_id {
			0 => Self::Air,
			1 => Self::Grass,
			2 => Self::Dirt,
			3 => Self::Stone,
			4 => Self::Sand,
			5 => {
				let dir = FlowDir::from_u8((packed >> 3) & 0x07);
				let is_source = (packed & (1 << 6)) != 0;
				Self::Water {
					is_source,
					sediment: 0,
					velocity: 0,
					direction: dir,
				}
			}
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
		assert!(!Tile::Water {
			is_source: false,
			sediment: 0,
			velocity: 0,
			direction: FlowDir::None
		}
		.is_solid());
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
	fn tile_pack_roundtrip_water() {
		let w = Tile::Water {
			is_source: true,
			sediment: 5,
			velocity: 3,
			direction: FlowDir::East,
		};
		let packed = w.pack();
		let unpacked = Tile::unpack(packed);
		// Water unpacking only preserves is_source and direction (rendering fields)
		// sediment/velocity are internal-only
		match unpacked {
			Tile::Water {
				is_source,
				direction,
				..
			} => {
				assert!(is_source);
				assert_eq!(direction, FlowDir::East);
			}
			_ => panic!("expected Water"),
		}
	}

	#[test]
	fn tile_opacity() {
		assert_eq!(Tile::Air.opacity(), 0);
		assert_eq!(Tile::Grass.opacity(), 10);
		assert_eq!(
			Tile::Water {
				is_source: false,
				sediment: 0,
				velocity: 0,
				direction: FlowDir::None
			}
			.opacity(),
			3
		);
	}

	#[test]
	fn flow_dir_all_variants() {
		assert_eq!(FlowDir::from_u8(0), FlowDir::None);
		assert_eq!(FlowDir::from_u8(1), FlowDir::Down);
		assert_eq!(FlowDir::from_u8(5), FlowDir::West);
		assert_eq!(FlowDir::from_u8(6), FlowDir::None); // out of range
	}
}
