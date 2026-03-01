pub mod ascii;

use crate::water::WaterSimulator;
use crate::world::World;

pub trait WorldRenderer {
	fn render(&self, world: &World<impl WaterSimulator>) -> String;
}
