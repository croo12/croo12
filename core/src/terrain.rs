use noise::{Fbm, MultiFractal, NoiseFn, Perlin};

use crate::tile::TileType;
use crate::water::WaterSimulator;
use crate::world::World;

const WATER_LEVEL: usize = 8;
const NOISE_SCALE: f64 = 0.03;
const RIVER_SOURCE_COUNT: usize = 3;

pub fn generate_terrain<S: WaterSimulator>(world: &mut World<S>, seed: u32) {
	let fbm = Fbm::<Perlin>::new(seed)
		.set_octaves(6)
		.set_frequency(NOISE_SCALE);

	let width = world.width();
	let depth = world.depth();
	let height = world.height();

	let mut surface_map = vec![0usize; width * depth];

	for y in 0..depth {
		for x in 0..width {
			let noise_val = fbm.get([x as f64, y as f64]);
			let normalized = ((noise_val + 1.0) / 2.0).clamp(0.0, 1.0);
			let surface_z = (normalized * (height as f64 * 0.6)) as usize + 2;
			let surface_z = surface_z.min(height - 1);
			surface_map[x + y * width] = surface_z;

			for z in 0..=surface_z {
				let tile = if z < surface_z.saturating_sub(3) {
					TileType::Stone
				} else if z < surface_z {
					TileType::Dirt
				} else if surface_z <= WATER_LEVEL {
					TileType::Sand
				} else {
					TileType::Grass
				};
				world.set_tile(x, y, z, tile);
			}

			// 해수면 물 채우기
			if surface_z < WATER_LEVEL {
				for z in (surface_z + 1)..=WATER_LEVEL {
					world.set_tile(x, y, z, TileType::Water);
					world.place_water(x, y, z, 8);
				}
			}
		}
	}

	// 강 생성: 높은 지점에서 수원 배치 → CA 안정화
	generate_rivers(world, &surface_map, width, depth, seed);
}

fn generate_rivers<S: WaterSimulator>(
	world: &mut World<S>,
	surface_map: &[usize],
	width: usize,
	depth: usize,
	seed: u32,
) {
	// 높은 지점 찾기 (상위 25%)
	let mut heights: Vec<(usize, usize, usize)> = Vec::new();
	for y in 0..depth {
		for x in 0..width {
			let sz = surface_map[x + y * width];
			heights.push((x, y, sz));
		}
	}
	heights.sort_by(|a, b| b.2.cmp(&a.2));

	let threshold = heights.len() / 4;
	let candidates = &heights[..threshold.max(1)];

	// seed 기반으로 수원 선택 (간단한 해싱)
	let mut sources = Vec::new();
	for i in 0..RIVER_SOURCE_COUNT.min(candidates.len()) {
		let pick =
			((seed as usize).wrapping_mul(31).wrapping_add(i * 97)) % candidates.len();
		let (x, y, sz) = candidates[pick];
		let source_z = (sz + 1).min(world.height() - 1);
		sources.push((x, y, source_z));
	}

	// 수원 배치 (is_source=true)
	for &(x, y, z) in &sources {
		world.place_water_source(x, y, z, 8);
	}

	// CA 안정화
	let stabilization_ticks = width * depth / 4;
	for _ in 0..stabilization_ticks {
		world.tick_water();
	}

	// 수원 플래그 제거
	world.clear_water_sources();
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::water::cellular::CellularWaterSimulator;

	#[test]
	fn generate_terrain_produces_water_in_low_areas() {
		let mut world = World::new(32, 32, 16, CellularWaterSimulator::new());
		generate_terrain(&mut world, 42);

		// WATER_LEVEL 이하에 물이 존재해야 함
		let mut has_water = false;
		for y in 0..32 {
			for x in 0..32 {
				for z in 0..=WATER_LEVEL {
					if world.water().get(x, y, z).level > 0
						|| world.get_tile(x, y, z).tile_type == TileType::Water
					{
						has_water = true;
					}
				}
			}
		}
		assert!(has_water, "낮은 지대에 물이 있어야 함");
	}

	#[test]
	fn generate_terrain_rivers_flow_downhill() {
		let mut world = World::new(32, 32, 16, CellularWaterSimulator::new());
		generate_terrain(&mut world, 42);

		// 강 수원 위치(높은 곳)에서 아래쪽으로 물이 있어야 함
		let mut high_water = 0;
		let mut low_water = 0;
		for y in 0..32 {
			for x in 0..32 {
				for z in 0..16 {
					if world.water().get(x, y, z).level > 0 {
						if z > WATER_LEVEL {
							high_water += 1;
						} else {
							low_water += 1;
						}
					}
				}
			}
		}
		assert!(low_water >= high_water, "물은 낮은 곳에 더 많아야 함");
	}
}
