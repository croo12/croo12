use crate::tile::TileType;
use crate::water::WaterCell;

pub enum SliceAxis {
	TopDown(usize),
	Side(usize),
}

pub struct AsciiRenderer {
	slice: SliceAxis,
}

impl AsciiRenderer {
	pub fn top_down(z: usize) -> Self {
		Self {
			slice: SliceAxis::TopDown(z),
		}
	}

	pub fn side_view(y: usize) -> Self {
		Self {
			slice: SliceAxis::Side(y),
		}
	}
}

fn format_cell(tile: u8, water: WaterCell) -> String {
	if water.level > 0 {
		let prefix = if water.is_source { '*' } else { '~' };
		return format!("{}{} ", prefix, water.level);
	}
	match tile {
		t if t == TileType::Grass as u8 => " G ".to_string(),
		t if t == TileType::Dirt as u8 => " D ".to_string(),
		t if t == TileType::Stone as u8 => " # ".to_string(),
		t if t == TileType::Sand as u8 => " S ".to_string(),
		_ => " . ".to_string(),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn format_cell_air() {
		assert_eq!(format_cell(TileType::Air as u8, WaterCell::EMPTY), " . ");
	}

	#[test]
	fn format_cell_stone() {
		assert_eq!(format_cell(TileType::Stone as u8, WaterCell::EMPTY), " # ");
	}

	#[test]
	fn format_cell_grass() {
		assert_eq!(format_cell(TileType::Grass as u8, WaterCell::EMPTY), " G ");
	}

	#[test]
	fn format_cell_water_level() {
		let cell = WaterCell {
			level: 4,
			is_source: false,
		};
		assert_eq!(format_cell(TileType::Air as u8, cell), "~4 ");
	}

	#[test]
	fn format_cell_water_source() {
		let cell = WaterCell {
			level: 8,
			is_source: true,
		};
		assert_eq!(format_cell(TileType::Air as u8, cell), "*8 ");
	}

	#[test]
	fn format_cell_water_on_solid_shows_water() {
		let cell = WaterCell {
			level: 3,
			is_source: false,
		};
		assert_eq!(format_cell(TileType::Stone as u8, cell), "~3 ");
	}
}
