use crate::render::WorldRenderer;
use crate::tile::TileType;
use crate::water::{WaterCell, WaterSimulator};
use crate::world::World;

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

	fn render_top_down(&self, world: &World<impl WaterSimulator>, z: usize) -> String {
		let w = world.width();
		let d = world.depth();
		assert!(z < world.height(), "z={} out of bounds (height={})", z, world.height());
		let mut out = format!("z={} ({}x{}):\n", z, w, d);
		for y in 0..d {
			for x in 0..w {
				out.push_str(&format_cell(world.get_tile(x, y, z), world.water().get(x, y, z)));
			}
			out.push('\n');
		}
		out
	}

	fn render_side(&self, world: &World<impl WaterSimulator>, y: usize) -> String {
		let w = world.width();
		let h = world.height();
		assert!(y < world.depth(), "y={} out of bounds (depth={})", y, world.depth());
		let mut out = format!("y={} (x->, z^, {}x{}):\n", y, w, h);
		for z in (0..h).rev() {
			for x in 0..w {
				out.push_str(&format_cell(world.get_tile(x, y, z), world.water().get(x, y, z)));
			}
			out.push('\n');
		}
		out
	}
}

impl WorldRenderer for AsciiRenderer {
	fn render(&self, world: &World<impl WaterSimulator>) -> String {
		match self.slice {
			SliceAxis::TopDown(z) => self.render_top_down(world, z),
			SliceAxis::Side(y) => self.render_side(world, y),
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

	use crate::render::WorldRenderer;
	use crate::water::cellular::CellularWaterSimulator;
	use crate::world::World;

	#[test]
	fn render_top_down_empty_world() {
		let world = World::new(3, 3, 2, CellularWaterSimulator::new());
		let renderer = AsciiRenderer::top_down(0);
		let output = renderer.render(&world);
		let expected = "z=0 (3x3):\n .  .  . \n .  .  . \n .  .  . \n";
		assert_eq!(output, expected);
	}

	#[test]
	fn render_top_down_with_tiles_and_water() {
		let mut world = World::new(3, 3, 2, CellularWaterSimulator::new());
		world.set_tile(0, 0, 0, TileType::Stone);
		world.set_tile(1, 0, 0, TileType::Grass);
		world.place_water(2, 0, 0, 5);
		let renderer = AsciiRenderer::top_down(0);
		let output = renderer.render(&world);
		let expected = "z=0 (3x3):\n #  G ~5 \n .  .  . \n .  .  . \n";
		assert_eq!(output, expected);
	}

	#[test]
	fn render_side_view_empty_world() {
		let world = World::new(3, 3, 2, CellularWaterSimulator::new());
		let renderer = AsciiRenderer::side_view(0);
		let output = renderer.render(&world);
		let expected = "y=0 (x->, z^, 3x2):\n .  .  . \n .  .  . \n";
		assert_eq!(output, expected);
	}

	#[test]
	fn render_side_view_with_tiles_and_water() {
		let mut world = World::new(3, 3, 2, CellularWaterSimulator::new());
		world.set_tile(0, 0, 0, TileType::Stone);
		world.set_tile(1, 0, 0, TileType::Stone);
		world.place_water(2, 0, 1, 6);
		let renderer = AsciiRenderer::side_view(0);
		let output = renderer.render(&world);
		// z=1 first (top), z=0 second (bottom)
		let expected = "y=0 (x->, z^, 3x2):\n .  . ~6 \n #  #  . \n";
		assert_eq!(output, expected);
	}

}
