use super::{WaterCell, WaterSimulator, WaterState};
use crate::tile::TileType;

pub struct CellularWaterSimulator;

impl CellularWaterSimulator {
	pub fn new() -> Self {
		Self
	}

	fn is_solid(terrain: &[u8], idx: usize) -> bool {
		let tile = terrain[idx];
		tile != TileType::Air as u8 && tile != TileType::Water as u8
	}
}

impl WaterSimulator for CellularWaterSimulator {
	fn tick(&mut self, state: &mut WaterState, terrain: &[u8]) {
		let w = state.width();
		let d = state.depth();
		let h = state.height();
		let original = state.snapshot_cells();
		let mut cells = original.clone();

		// Pass 1: Gravity — snapshot 기반, 한 tick에 한 층만 이동
		for z in (1..h).rev() {
			for y in 0..d {
				for x in 0..w {
					let idx = x + y * w + z * w * d;
					let src_level = original[idx].level;
					if src_level == 0 {
						continue;
					}

					let below_idx = x + y * w + (z - 1) * w * d;
					if Self::is_solid(terrain, below_idx) {
						continue;
					}

					// below의 현재 누적량 확인 (다른 셀에서 이미 떨어진 물 고려)
					if cells[below_idx].level >= 8 {
						continue;
					}

					let space = 8 - cells[below_idx].level;
					let transfer = src_level.min(space);
					cells[below_idx].level += transfer;
					cells[idx].level = cells[idx].level.saturating_sub(transfer);
				}
			}
		}

		// Pass 2: Horizontal spread — snapshot 기반 delta 누적
		// 아래가 비어있으면 수평 분배 스킵 (중력 우선)
		let after_gravity = cells.clone();
		let mut deltas = vec![0i16; w * d * h];

		for z in 0..h {
			for y in 0..d {
				for x in 0..w {
					let idx = x + y * w + z * w * d;
					let my_level = after_gravity[idx].level;
					if my_level <= 1 {
						continue;
					}

					// 아래가 비어있으면 중력이 우선 — 수평 분배 스킵
					if z > 0 {
						let below_idx = x + y * w + (z - 1) * w * d;
						if !Self::is_solid(terrain, below_idx)
							&& after_gravity[below_idx].level < 8
						{
							continue;
						}
					}

					let neighbors: [(isize, isize); 4] =
						[(-1, 0), (1, 0), (0, -1), (0, 1)];
					let mut lower_count: u8 = 0;
					let mut total_diff: u16 = 0;
					let mut lower_indices = [(0usize, 0u8); 4];

					for (dx, dy) in neighbors {
						let nx = x as isize + dx;
						let ny = y as isize + dy;
						if nx < 0 || nx >= w as isize || ny < 0 || ny >= d as isize {
							continue;
						}
						let nidx = nx as usize + ny as usize * w + z * w * d;
						if Self::is_solid(terrain, nidx) {
							continue;
						}
						let n_level = after_gravity[nidx].level;
						if n_level < my_level {
							let diff = my_level - n_level;
							lower_indices[lower_count as usize] = (nidx, diff);
							total_diff += diff as u16;
							lower_count += 1;
						}
					}

					if lower_count == 0 {
						continue;
					}

					// 총 유출량: 각 이웃으로의 차이 합 / (이웃수 + 1), 최대 my_level - 1
					let total_give =
						((total_diff / (lower_count as u16 + 1)) as u8).min(my_level - 1);
					if total_give == 0 {
						continue;
					}

					deltas[idx] -= total_give as i16;

					// 차이 비례로 분배
					let mut distributed: u8 = 0;
					for i in 0..lower_count as usize {
						let (nidx, diff) = lower_indices[i];
						let share = if total_diff > 0 {
							((total_give as u16 * diff as u16) / total_diff) as u8
						} else {
							0
						};
						let share = share.min(total_give - distributed);
						deltas[nidx] += share as i16;
						distributed += share;
					}
					// 나머지 1단위는 첫 이웃에게
					if distributed < total_give {
						deltas[lower_indices[0].0] += (total_give - distributed) as i16;
					}
				}
			}
		}

		// Apply deltas
		for (i, cell) in cells.iter_mut().enumerate() {
			let new_level = (after_gravity[i].level as i16 + deltas[i]).clamp(0, 8) as u8;
			cell.level = new_level;
		}

		// Pass 3: Source replenishment
		for (i, cell) in cells.iter_mut().enumerate() {
			if original[i].is_source {
				cell.level = 8;
				cell.is_source = true;
			}
		}

		state.apply_cells(cells);
	}

	fn place_water(&mut self, state: &mut WaterState, x: usize, y: usize, z: usize, level: u8) {
		state.set(
			x,
			y,
			z,
			WaterCell {
				level,
				is_source: false,
			},
		);
	}

	fn remove_water(&mut self, state: &mut WaterState, x: usize, y: usize, z: usize) {
		state.set(x, y, z, WaterCell::EMPTY);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tile::TileType;

	fn make_terrain(w: usize, d: usize, h: usize) -> Vec<u8> {
		vec![TileType::Air as u8; w * d * h]
	}

	fn set_terrain(
		terrain: &mut [u8],
		w: usize,
		d: usize,
		x: usize,
		y: usize,
		z: usize,
		tile: TileType,
	) {
		terrain[x + y * w + z * w * d] = tile as u8;
	}

	#[test]
	fn water_falls_down() {
		let (w, d, h) = (4, 4, 4);
		let mut state = WaterState::new(w, d, h);
		let terrain = make_terrain(w, d, h);
		let mut sim = CellularWaterSimulator::new();

		state.set(
			1,
			1,
			3,
			WaterCell {
				level: 8,
				is_source: false,
			},
		);
		sim.tick(&mut state, &terrain);

		assert_eq!(state.get(1, 1, 3).level, 0, "원래 위치는 비어야 함");
		assert_eq!(state.get(1, 1, 2).level, 8, "아래로 이동해야 함");
	}

	#[test]
	fn water_stops_on_solid() {
		let (w, d, h) = (4, 4, 4);
		let mut state = WaterState::new(w, d, h);
		let mut terrain = make_terrain(w, d, h);
		let mut sim = CellularWaterSimulator::new();

		set_terrain(&mut terrain, w, d, 1, 1, 0, TileType::Stone);
		state.set(
			1,
			1,
			1,
			WaterCell {
				level: 8,
				is_source: false,
			},
		);
		sim.tick(&mut state, &terrain);

		// 고체 아래로 물이 빠지지 않아야 함
		assert_eq!(state.get(1, 1, 0).level, 0, "고체 블록에는 물이 없어야 함");
		// z=1에 물이 남아 있어야 함 (수평 분배로 줄어들 수 있음)
		assert!(state.get(1, 1, 1).level > 0, "고체 위에 물이 남아야 함");
	}

	#[test]
	fn water_spreads_horizontally() {
		let (w, d, h) = (4, 4, 4);
		let mut state = WaterState::new(w, d, h);
		let mut terrain = make_terrain(w, d, h);
		let mut sim = CellularWaterSimulator::new();

		// 바닥을 전부 고체로
		for y in 0..d {
			for x in 0..w {
				set_terrain(&mut terrain, w, d, x, y, 0, TileType::Stone);
			}
		}
		state.set(
			2,
			2,
			1,
			WaterCell {
				level: 8,
				is_source: false,
			},
		);

		sim.tick(&mut state, &terrain);

		// 중앙 level 감소, 이웃 중 일부에 물 분배
		let center = state.get(2, 2, 1).level;
		let neighbors_total: u8 = [(1, 2), (3, 2), (2, 1), (2, 3)]
			.iter()
			.map(|&(nx, ny)| state.get(nx, ny, 1).level)
			.sum();
		assert!(center < 8, "중앙은 분배 후 줄어야 함");
		assert!(neighbors_total > 0, "이웃에 물이 분배되어야 함");
	}

	#[test]
	fn source_replenishes_level() {
		let (w, d, h) = (4, 4, 4);
		let mut state = WaterState::new(w, d, h);
		let mut terrain = make_terrain(w, d, h);
		let mut sim = CellularWaterSimulator::new();

		for y in 0..d {
			for x in 0..w {
				set_terrain(&mut terrain, w, d, x, y, 0, TileType::Stone);
			}
		}
		state.set(
			2,
			2,
			1,
			WaterCell {
				level: 8,
				is_source: true,
			},
		);

		sim.tick(&mut state, &terrain);

		assert_eq!(state.get(2, 2, 1).level, 8, "수원은 level 유지");
		assert!(state.get(2, 2, 1).is_source, "수원 플래그 유지");
	}
}
