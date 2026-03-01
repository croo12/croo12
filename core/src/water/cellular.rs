use super::{WaterCell, WaterSimulator, WaterState};
use crate::tile::{Tile, TileType};

pub struct CellularWaterSimulator;

impl CellularWaterSimulator {
	pub fn new() -> Self {
		Self
	}

	fn is_solid(terrain: &[Tile], idx: usize) -> bool {
		terrain[idx].is_solid()
	}

	fn available_depth(
		terrain: &[Tile],
		cells: &[WaterCell],
		w: usize,
		d: usize,
		nx: usize,
		ny: usize,
		z: usize,
	) -> usize {
		let mut capacity = 0usize;
		for dz in 0..=z {
			let check_z = z - dz;
			let idx = nx + ny * w + check_z * w * d;
			if terrain[idx].is_solid() {
				break;
			}
			capacity += (8 - cells[idx].level) as usize;
		}
		capacity
	}
}

impl WaterSimulator for CellularWaterSimulator {
	fn tick(&mut self, state: &mut WaterState, terrain: &mut [Tile]) {
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
					let mut total_weight: usize = 0;
					let mut lower_indices = [(0usize, 0u8); 4];

					for (dx, dy) in neighbors {
						let nx = x as isize + dx;
						let ny = y as isize + dy;
						if nx < 0 || nx >= w as isize || ny < 0 || ny >= d as isize {
							continue;
						}
						let nx = nx as usize;
						let ny = ny as usize;
						let nidx = nx + ny * w + z * w * d;
						if Self::is_solid(terrain, nidx) {
							continue;
						}
						let n_level = after_gravity[nidx].level;
						if n_level < my_level {
							let weight = Self::available_depth(terrain, &after_gravity, w, d, nx, ny, z);
							let weight_u8 = weight.min(u8::MAX as usize) as u8;
							lower_indices[lower_count as usize] = (nidx, weight_u8);
							total_weight += weight;
							lower_count += 1;
						}
					}

					if lower_count == 0 {
						continue;
					}

					// 총 유출량: 기존 level 차이 기반 로직 유지
					// Compute total_diff for existing total_give formula
					let mut total_diff: u16 = 0;
					for i in 0..lower_count as usize {
						let (nidx, _) = lower_indices[i];
						let n_level = after_gravity[nidx].level;
						let diff = my_level - n_level;
						total_diff += diff as u16;
					}

					let total_give =
						((total_diff / (lower_count as u16 + 1)) as u8).min(my_level - 1);
					if total_give == 0 {
						continue;
					}

					deltas[idx] -= total_give as i16;

					// available_depth 비례로 분배
					let mut distributed: u8 = 0;
					for i in 0..lower_count as usize {
						let (nidx, weight) = lower_indices[i];
						let share = if total_weight > 0 {
							((total_give as usize * weight as usize) / total_weight).min(u8::MAX as usize) as u8
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

		// Pass 3: Erosion & Deposition
		// Sub-pass 3a: Erosion — track which terrain indices were eroded this tick
		let mut eroded = vec![false; w * d * h];
		for z in 0..h {
			for y in 0..d {
				for x in 0..w {
					let idx = x + y * w + z * w * d;
					let water_level = cells[idx].level;
					if water_level == 0 {
						continue;
					}

					// Erode tile below
					if z > 0 {
						let below_idx = x + y * w + (z - 1) * w * d;
						if terrain[below_idx].is_erodible() && terrain[below_idx].level > 0 {
							let erosion_amount =
								(water_level / 4).max(1).min(terrain[below_idx].level);
							terrain[below_idx].level -= erosion_amount;
							cells[idx].sediment =
								cells[idx].sediment.saturating_add(erosion_amount);
							eroded[below_idx] = true;

							if terrain[below_idx].level == 0 {
								terrain[below_idx].tile_type = TileType::Air;
							}
						}
					}
				}
			}
		}

		// Sub-pass 3b: Deposition — only deposit on tiles that were NOT eroded this tick
		for z in 0..h {
			for y in 0..d {
				for x in 0..w {
					let idx = x + y * w + z * w * d;
					if cells[idx].sediment > 0 {
						let depth =
							Self::available_depth(terrain, &cells, w, d, x, y, z);
						if depth <= 8 {
							let deposit = cells[idx].sediment.min(2);
							cells[idx].sediment -= deposit;
							if z > 0 {
								let below_idx = x + y * w + (z - 1) * w * d;
								if !terrain[below_idx].is_solid() {
									terrain[below_idx] = Tile {
										tile_type: TileType::Sand,
										level: deposit,
										moisture: 0,
										variant: 0,
									};
								} else if !eroded[below_idx] {
									terrain[below_idx].level =
										(terrain[below_idx].level + deposit).min(8);
								}
							}
						}
					}
				}
			}
		}

		// Pass 4: Source replenishment
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
				sediment: 0,
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

	fn make_terrain(w: usize, d: usize, h: usize) -> Vec<Tile> {
		vec![Tile::new(TileType::Air); w * d * h]
	}

	fn set_terrain(
		terrain: &mut [Tile],
		w: usize,
		d: usize,
		x: usize,
		y: usize,
		z: usize,
		tile: TileType,
	) {
		terrain[x + y * w + z * w * d] = Tile::new(tile);
	}

	#[test]
	fn water_falls_down() {
		let (w, d, h) = (4, 4, 4);
		let mut state = WaterState::new(w, d, h);
		let mut terrain = make_terrain(w, d, h);
		let mut sim = CellularWaterSimulator::new();

		state.set(
			1,
			1,
			3,
			WaterCell {
				level: 8,
				is_source: false,
				sediment: 0,
			},
		);
		sim.tick(&mut state, &mut terrain);

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
				sediment: 0,
			},
		);
		sim.tick(&mut state, &mut terrain);

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
				sediment: 0,
			},
		);

		sim.tick(&mut state, &mut terrain);

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
				sediment: 0,
			},
		);

		sim.tick(&mut state, &mut terrain);

		assert_eq!(state.get(2, 2, 1).level, 8, "수원은 level 유지");
		assert!(state.get(2, 2, 1).is_source, "수원 플래그 유지");
	}

	#[test]
	fn water_prefers_lower_terrain() {
		let (w, d, h) = (5, 1, 5);
		let mut state = WaterState::new(w, d, h);
		let mut terrain = make_terrain(w, d, h);

		// Staircase: x=0 floor=3, x=1 floor=2, x=2 floor=1, x=3 floor=0, x=4 floor=0
		for x in 0..w {
			let floor_z = if x <= 2 { 3 - x } else { 0 };
			for z in 0..=floor_z {
				set_terrain(&mut terrain, w, d, x, 0, z, TileType::Stone);
			}
		}

		// Water at top of stairs
		state.set(0, 0, 4, WaterCell { level: 8, is_source: false, sediment: 0 });

		let mut sim = CellularWaterSimulator::new();
		for _ in 0..10 {
			sim.tick(&mut state, &mut terrain);
		}

		let low_water: u8 = (3..5).map(|x| {
			(0..h).map(|z| state.get(x, 0, z).level).sum::<u8>()
		}).sum();
		let high_water: u8 = (0..2).map(|x| {
			(0..h).map(|z| state.get(x, 0, z).level).sum::<u8>()
		}).sum();
		assert!(low_water > high_water, "water should pool at lower terrain: low={}, high={}", low_water, high_water);
	}

	#[test]
	fn erosion_reduces_erodible_tile_level() {
		let (w, d, h) = (3, 3, 4);
		let mut state = WaterState::new(w, d, h);
		let mut terrain = make_terrain(w, d, h);

		for y in 0..d {
			for x in 0..w {
				set_terrain(&mut terrain, w, d, x, y, 0, TileType::Dirt);
			}
		}
		state.set(1, 1, 1, WaterCell { level: 8, is_source: false, sediment: 0 });

		let mut sim = CellularWaterSimulator::new();
		sim.tick(&mut state, &mut terrain);

		let dirt_tile = terrain[1 + 1 * w + 0 * w * d];
		assert!(dirt_tile.level < 8, "erosion should reduce level: {}", dirt_tile.level);
	}

	#[test]
	fn stone_is_not_eroded() {
		let (w, d, h) = (3, 3, 3);
		let mut state = WaterState::new(w, d, h);
		let mut terrain = make_terrain(w, d, h);

		set_terrain(&mut terrain, w, d, 1, 1, 0, TileType::Stone);
		state.set(1, 1, 1, WaterCell { level: 8, is_source: false, sediment: 0 });

		let mut sim = CellularWaterSimulator::new();
		sim.tick(&mut state, &mut terrain);

		assert_eq!(terrain[1 + 1 * w].tile_type, TileType::Stone);
		assert_eq!(terrain[1 + 1 * w].level, 8, "Stone must not erode");
	}
}
