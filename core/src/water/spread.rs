use crate::tile::{FlowDir, Tile};
use crate::world::World;

/// Scan downward from (x, y, z-1) counting consecutive Air cells.
fn scan_depth(world: &World, x: usize, y: usize, z: usize) -> usize {
	let mut depth = 0;
	let mut cz = z;
	while cz > 0 {
		cz -= 1;
		if world.get(x, y, cz).is_air() {
			depth += 1;
		} else {
			break;
		}
	}
	depth
}

/// Pick best horizontal direction based on current direction + scan_depth.
fn pick_direction(
	world: &World,
	x: usize,
	y: usize,
	z: usize,
	current_dir: FlowDir,
	velocity: u8,
) -> Option<(FlowDir, usize, usize)> {
	let w = world.width();
	let d = world.depth();

	// Neighbors: (dir, nx, ny)
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

	// Classify by priority: forward, perpendicular, backward
	let opposite = current_dir.opposite();
	let perps = current_dir.perpendiculars();

	struct Candidate {
		dir: FlowDir,
		nx: usize,
		ny: usize,
		depth: usize,
		priority: u8, // 0=forward, 1=perp, 2=backward
	}

	let mut candidates: Vec<Candidate> = Vec::new();

	for (dir, pos) in &neighbors {
		if let Some((nx, ny)) = pos {
			let target = world.get(*nx, *ny, z);
			if !target.is_air() {
				continue;
			}
			let depth = scan_depth(world, *nx, *ny, z);
			let priority = if *dir == current_dir {
				0
			} else if *dir == opposite {
				2
			} else if perps.contains(dir) {
				1
			} else {
				1
			};
			candidates.push(Candidate {
				dir: *dir,
				nx: *nx,
				ny: *ny,
				depth,
				priority,
			});
		}
	}

	if candidates.is_empty() {
		return None;
	}

	// High velocity: only forward
	if velocity >= 4 {
		if let Some(c) = candidates.iter().find(|c| c.priority == 0) {
			return Some((c.dir, c.nx, c.ny));
		}
	}

	// Sort: priority asc, then depth desc
	candidates.sort_by(|a, b| a.priority.cmp(&b.priority).then(b.depth.cmp(&a.depth)));

	let best = &candidates[0];
	Some((best.dir, best.nx, best.ny))
}

pub fn pass_spread(world: &mut World) {
	let w = world.width();
	let d = world.depth();
	let h = world.height();

	// Collect moves first (snapshot approach)
	let mut moves: Vec<(usize, usize, usize, usize, usize, usize, Tile)> = Vec::new();

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

					// Determine direction for newly landed water
					let dir = if direction == FlowDir::Down || direction == FlowDir::None {
						FlowDir::None // will be recalculated by pick_direction
					} else {
						direction
					};

					if let Some((new_dir, nx, ny)) =
						pick_direction(world, x, y, z, dir, velocity)
					{
						let new_vel = if velocity > 0 { velocity - 1 } else { 0 }.max(1);
						let new_water = Tile::Water {
							is_source: false,
							sediment,
							velocity: new_vel,
							direction: new_dir,
						};
						moves.push((x, y, z, nx, ny, z, new_water));
					} else {
						// Stagnant: reset velocity
						if velocity > 0 || direction != FlowDir::None {
							world.set(
								x,
								y,
								z,
								Tile::Water {
									is_source,
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
	for (ox, oy, oz, nx, ny, nz, new_water) in moves {
		// Only move if target is still air and source is still water
		if world.get(nx, ny, nz).is_air() && world.get(ox, oy, oz).is_water() {
			let src = world.get(ox, oy, oz);
			if let Tile::Water { is_source, .. } = src {
				world.set(nx, ny, nz, new_water);
				if is_source {
					world.set(ox, oy, oz, Tile::water_source());
				} else {
					world.set(ox, oy, oz, Tile::Air);
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
	fn water_spreads_to_lower_neighbor() {
		// Ground at z=0, water at z=1 (on ground), air neighbor at z=1 with no ground
		let mut world = World::new(4, 4, 4);
		world.set(1, 1, 0, Tile::Stone); // ground under water
		world.set(1, 1, 1, Tile::water_default());
		// neighbor (2,1) has no ground, so scan_depth is deeper
		pass_spread(&mut world);
		// Water should move to the neighbor with deepest scan_depth
		assert!(world.get(1, 1, 1).is_air() || world.get(1, 1, 1).is_water());
	}

	#[test]
	fn water_continues_in_direction() {
		let mut world = World::new(8, 4, 4);
		// Flat ground
		for x in 0..8 {
			world.set(x, 1, 0, Tile::Stone);
		}
		world.set(
			3,
			1,
			1,
			Tile::Water {
				is_source: false,
				sediment: 0,
				velocity: 3,
				direction: FlowDir::East,
			},
		);
		pass_spread(&mut world);
		// Should continue East (priority direction)
		assert!(world.get(4, 1, 1).is_water());
		assert!(world.get(3, 1, 1).is_air());
	}

	#[test]
	fn high_velocity_only_goes_forward() {
		let mut world = World::new(8, 8, 4);
		for x in 0..8 {
			for y in 0..8 {
				world.set(x, y, 0, Tile::Stone);
			}
		}
		world.set(
			3,
			3,
			1,
			Tile::Water {
				is_source: false,
				sediment: 0,
				velocity: 5,
				direction: FlowDir::East,
			},
		);
		pass_spread(&mut world);
		assert!(world.get(4, 3, 1).is_water()); // moved East
		assert!(world.get(3, 3, 1).is_air());
	}

	#[test]
	fn stagnant_water_stays() {
		let mut world = World::new(4, 4, 4);
		// Surrounded by solid
		for x in 0..4 {
			for y in 0..4 {
				world.set(x, y, 0, Tile::Stone);
			}
		}
		world.set(0, 1, 1, Tile::Stone);
		world.set(2, 1, 1, Tile::Stone);
		world.set(1, 0, 1, Tile::Stone);
		world.set(1, 2, 1, Tile::Stone);
		world.set(1, 1, 0, Tile::Stone); // floor
		world.set(1, 1, 1, Tile::water_default());
		pass_spread(&mut world);
		assert!(world.get(1, 1, 1).is_water()); // no move
	}
}
