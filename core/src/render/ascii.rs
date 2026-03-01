use crate::tile::Tile;
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

	pub fn render(&self, world: &World) -> String {
		match self.slice {
			SliceAxis::TopDown(z) => self.render_top_down(world, z),
			SliceAxis::Side(y) => self.render_side(world, y),
		}
	}

	fn render_top_down(&self, world: &World, z: usize) -> String {
		let w = world.width();
		let d = world.depth();
		assert!(z < world.height(), "z={} out of bounds (height={})", z, world.height());
		let mut out = format!("z={} ({}x{}):\n", z, w, d);
		for y in 0..d {
			for x in 0..w {
				out.push_str(&format_cell(world.get(x, y, z), world.water_mass(x, y, z)));
			}
			out.push('\n');
		}
		out
	}

	fn render_side(&self, world: &World, y: usize) -> String {
		let w = world.width();
		let h = world.height();
		assert!(y < world.depth(), "y={} out of bounds (depth={})", y, world.depth());
		let mut out = format!("y={} (x->, z^, {}x{}):\n", y, w, h);
		for z in (0..h).rev() {
			for x in 0..w {
				out.push_str(&format_cell(world.get(x, y, z), world.water_mass(x, y, z)));
			}
			out.push('\n');
		}
		out
	}
}

fn format_cell(tile: Tile, water_mass: u8) -> String {
	if water_mass > 0 {
		return format!("{:>3}", water_mass);
	}
	match tile {
		Tile::Air => " . ".to_string(),
		Tile::Grass => " G ".to_string(),
		Tile::Dirt => " D ".to_string(),
		Tile::Stone => " # ".to_string(),
		Tile::Sand => " S ".to_string(),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tile::Tile;
	use crate::world::World;

	#[test]
	fn format_cell_tiles() {
		assert_eq!(format_cell(Tile::Air, 0), " . ");
		assert_eq!(format_cell(Tile::Stone, 0), " # ");
		assert_eq!(format_cell(Tile::Grass, 0), " G ");
		assert_eq!(format_cell(Tile::Dirt, 0), " D ");
		assert_eq!(format_cell(Tile::Sand, 0), " S ");
	}

	#[test]
	fn format_cell_water_mass() {
		assert_eq!(format_cell(Tile::Air, 100), "100");
		assert_eq!(format_cell(Tile::Grass, 0), " G ");
		assert_eq!(format_cell(Tile::Air, 0), " . ");
	}

	#[test]
	fn render_top_down_empty() {
		let world = World::new(3, 3, 2);
		let r = AsciiRenderer::top_down(0);
		let out = r.render(&world);
		assert_eq!(out, "z=0 (3x3):\n .  .  . \n .  .  . \n .  .  . \n");
	}

	#[test]
	fn render_side_with_tiles() {
		let mut world = World::new(3, 3, 3);
		world.set(0, 0, 0, Tile::Stone);
		world.set(1, 0, 0, Tile::Stone);
		world.set(2, 0, 0, Tile::Stone);
		world.set_water_mass(1, 0, 1, 100);
		let r = AsciiRenderer::side_view(0);
		let out = r.render(&world);
		let expected = "y=0 (x->, z^, 3x3):\n .  .  . \n . 100 . \n #  #  # \n";
		assert_eq!(out, expected);
	}
}
