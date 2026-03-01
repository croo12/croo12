use crate::tile::{FlowDir, Tile};
use crate::world::World;

/// Check if a horizontal neighbor is a valid spread target.
/// Returns true if the neighbor at (nx, ny, z) is Air.
fn can_spread_to(world: &World, nx: usize, ny: usize, z: usize) -> bool {
	world.get(nx, ny, z).is_air()
}

/// Check if neighbor has air below (water can fall there).
fn has_drop(world: &World, nx: usize, ny: usize, z: usize) -> bool {
	z > 0 && world.get(nx, ny, z - 1).is_air()
}

/// Pick best horizontal neighbor for spreading water.
/// Priority: neighbors where water can fall > flat neighbors.
/// Among equals, prefer current direction (momentum).
fn pick_target(
	world: &World,
	x: usize,
	y: usize,
	z: usize,
	current_dir: FlowDir,
) -> Option<(FlowDir, usize, usize)> {
	let w = world.width();
	let d = world.depth();

	let neighbors: [(FlowDir, Option<(usize, usize)>); 4] = [
		(
			FlowDir::North,
			if y > 0 { Some((x, y - 1)) } else { None },
		),
		(
			FlowDir::South,
			if y + 1 < d { Some((x, y + 1)) } else { None },
		),
		(
			FlowDir::East,
			if x + 1 < w { Some((x + 1, y)) } else { None },
		),
		(
			FlowDir::West,
			if x > 0 { Some((x - 1, y)) } else { None },
		),
	];

	// Collect valid targets with their properties
	struct Candidate {
		dir: FlowDir,
		nx: usize,
		ny: usize,
		drop: bool,       // has air below
		is_forward: bool,  // matches current direction
	}

	let mut candidates: Vec<Candidate> = Vec::new();

	for (dir, pos) in &neighbors {
		if let Some((nx, ny)) = pos {
			if !can_spread_to(world, *nx, *ny, z) {
				continue;
			}
			candidates.push(Candidate {
				dir: *dir,
				nx: *nx,
				ny: *ny,
				drop: has_drop(world, *nx, *ny, z),
				is_forward: *dir == current_dir,
			});
		}
	}

	if candidates.is_empty() {
		return None;
	}

	match current_dir {
		FlowDir::Down => {
			// Just landed from falling: spread to any neighbor, prefer drops
			candidates.sort_by(|a, b| b.drop.cmp(&a.drop));
		}
		FlowDir::None => {
			// Stagnant: only spread toward drops
			candidates.retain(|c| c.drop);
			if candidates.is_empty() {
				return None;
			}
		}
		_ => {
			// Has horizontal momentum: prefer forward, then drops
			candidates.sort_by(|a, b| {
				b.is_forward
					.cmp(&a.is_forward)
					.then(b.drop.cmp(&a.drop))
			});
			let best = &candidates[0];
			// If no forward and no drop, stop
			if !best.is_forward && !best.drop {
				return None;
			}
		}
	}

	let best = &candidates[0];
	Some((best.dir, best.nx, best.ny))
}

pub fn pass_spread(world: &mut World) {
	let w = world.width();
	let d = world.depth();
	let h = world.height();

	// Collect moves first (snapshot approach)
	struct Move {
		ox: usize,
		oy: usize,
		oz: usize,
		nx: usize,
		ny: usize,
		nz: usize,
		new_water: Tile,
	}

	let mut moves: Vec<Move> = Vec::new();

	for z in (0..h).rev() {
		for y in 0..d {
			for x in 0..w {
				let tile = world.get(x, y, z);
				if let Tile::Water {
					is_source,
					sediment,
					velocity,
					direction,
				} = tile
				{
					// Only spread if can't fall (below is not air)
					if z > 0 && world.get(x, y, z - 1).is_air() {
						continue; // gravity handles this
					}

					let has_water_above =
						z + 1 < h && world.get(x, y, z + 1).is_water();

					// Non-source water needs pressure (water above) to spread.
					// A single water block on the ground stays put.
					if !is_source && !has_water_above {
						// No pressure — become stagnant
						if velocity > 0 || direction != FlowDir::None {
							world.set(
								x,
								y,
								z,
								Tile::Water {
									is_source: false,
									sediment,
									velocity: 0,
									direction: FlowDir::None,
								},
							);
						}
						continue;
					}

					// Source or pressured water: spread to any available neighbor
					let dir = FlowDir::Down;

					if let Some((new_dir, nx, ny)) =
						pick_target(world, x, y, z, dir)
					{
						let new_vel = if velocity > 1 { velocity - 1 } else { 1 };
						let new_water = Tile::Water {
							is_source: false,
							sediment,
							velocity: new_vel,
							direction: new_dir,
						};
						moves.push(Move {
							ox: x,
							oy: y,
							oz: z,
							nx,
							ny,
							nz: z,
							new_water,
						});
					} else if !is_source {
						// Stagnant: reset velocity and direction
						if velocity > 0 || direction != FlowDir::None {
							world.set(
								x,
								y,
								z,
								Tile::Water {
									is_source: false,
									sediment,
									velocity: 0,
									direction: FlowDir::None,
								},
							);
						}
					}
				}
			}
		}
	}

	// Apply moves
	for m in moves {
		// Only move if target is still air and source is still water
		if world.get(m.nx, m.ny, m.nz).is_air() && world.get(m.ox, m.oy, m.oz).is_water() {
			let src = world.get(m.ox, m.oy, m.oz);
			if let Tile::Water { is_source, .. } = src {
				world.set(m.nx, m.ny, m.nz, m.new_water);
				if is_source {
					world.set(m.ox, m.oy, m.oz, Tile::water_source());
				} else {
					world.set(m.ox, m.oy, m.oz, Tile::Air);
				}
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tile::{FlowDir, Tile};
	use crate::world::World;

	#[test]
	fn single_water_block_stays_put() {
		// A single water block without pressure should not spread
		let mut world = World::new(4, 4, 4);
		world.set(1, 1, 0, Tile::Stone);
		world.set(1, 1, 1, Tile::water_default());
		pass_spread(&mut world);
		assert!(world.get(1, 1, 1).is_water());
	}

	#[test]
	fn pressured_water_spreads() {
		// 2+ stacked water blocks: bottom one spreads due to pressure
		let mut world = World::new(4, 4, 4);
		world.set(1, 1, 0, Tile::Stone);
		world.set(1, 1, 1, Tile::water_default()); // bottom
		world.set(1, 1, 2, Tile::water_default()); // top (pressure)
		pass_spread(&mut world);
		// Bottom water should have spread to a neighbor
		let has_neighbor_water = world.get(0, 1, 1).is_water()
			|| world.get(2, 1, 1).is_water()
			|| world.get(1, 0, 1).is_water()
			|| world.get(1, 2, 1).is_water();
		assert!(has_neighbor_water);
	}

	#[test]
	fn source_water_emits_to_neighbor() {
		// Source always emits regardless of pressure
		let mut world = World::new(4, 4, 4);
		world.set(1, 1, 0, Tile::Stone);
		world.set(1, 1, 1, Tile::water_source());
		pass_spread(&mut world);
		assert!(world.get(1, 1, 1).is_water());
		if let Tile::Water { is_source, .. } = world.get(1, 1, 1) {
			assert!(is_source);
		}
		let has_neighbor_water = world.get(0, 1, 1).is_water()
			|| world.get(2, 1, 1).is_water()
			|| world.get(1, 0, 1).is_water()
			|| world.get(1, 2, 1).is_water();
		assert!(has_neighbor_water);
	}
}
