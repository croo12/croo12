use std::sync::atomic::{AtomicU64, Ordering};
use crate::world::{
	World, Cloud, CLOUD_THRESHOLD, CLOUD_WATER, DROPS_PER_TICK,
	RAIN_MASS_PER_DROP, CLOUD_SPEED, MAX_CLOUDS,
};

static WEATHER_TICK: AtomicU64 = AtomicU64::new(0);

fn simple_hash(a: u64, b: u64) -> u64 {
	let mut h = a;
	h = h.wrapping_mul(6364136223846793005).wrapping_add(b);
	h ^ (h >> 33)
}

pub fn pass_cloud_spawn(world: &mut World) {
	if world.atmospheric_moisture < CLOUD_THRESHOLD {
		return;
	}
	if world.clouds.len() >= MAX_CLOUDS {
		return;
	}

	world.atmospheric_moisture -= CLOUD_WATER;

	let seed = WEATHER_TICK.load(Ordering::Relaxed);
	let w = world.width() as f32;
	let d = world.depth() as f32;

	// Determine entry edge (0=left, 1=right, 2=top, 3=bottom)
	let edge = (simple_hash(seed, world.clouds.len() as u64) % 4) as u8;
	let along = (simple_hash(seed, world.clouds.len() as u64 + 100) % 1000) as f32 / 1000.0;

	let (x, y, dx, dy) = match edge {
		0 => (0.0, along * d, CLOUD_SPEED, 0.0),
		1 => (w - 1.0, along * d, -CLOUD_SPEED, 0.0),
		2 => (along * w, 0.0, 0.0, CLOUD_SPEED),
		_ => (along * w, d - 1.0, 0.0, -CLOUD_SPEED),
	};

	// Add slight diagonal drift
	let drift = ((simple_hash(seed, world.clouds.len() as u64 + 200) % 100) as f32 - 50.0) / 500.0;
	let (dx, dy) = if dx.abs() > dy.abs() {
		(dx, drift)
	} else {
		(drift, dy)
	};

	world.clouds.push(Cloud {
		x, y, dx, dy,
		water: CLOUD_WATER,
		radius: 2.5,
	});
}

/// Find the topmost exposed z for a column (x, y).
fn find_top_exposed_z(world: &World, x: usize, y: usize) -> usize {
	let h = world.height();
	for z in (0..h).rev() {
		if world.get(x, y, z).is_solid() || world.water_mass(x, y, z) > 0 {
			return if z + 1 < h { z + 1 } else { z };
		}
	}
	0
}

pub fn pass_cloud_update(world: &mut World) {
	let seed = WEATHER_TICK.fetch_add(1, Ordering::Relaxed);
	let w = world.width();
	let d = world.depth();

	let mut i = 0;
	while i < world.clouds.len() {
		let cloud = &mut world.clouds[i];

		// Move
		cloud.x += cloud.dx;
		cloud.y += cloud.dy;

		// Check out of bounds
		let margin = cloud.radius;
		if cloud.x < -margin || cloud.x >= w as f32 + margin
			|| cloud.y < -margin || cloud.y >= d as f32 + margin
		{
			world.clouds.remove(i);
			continue;
		}

		// Rain
		let drops = DROPS_PER_TICK.min(cloud.water / RAIN_MASS_PER_DROP as u32);
		let cx = cloud.x;
		let cy = cloud.y;
		let r = cloud.radius;

		for drop_i in 0..drops {
			let hash = simple_hash(seed.wrapping_add(drop_i as u64), i as u64);
			let ox = ((hash % 1000) as f32 / 1000.0 - 0.5) * 2.0 * r;
			let oy = (((hash >> 16) % 1000) as f32 / 1000.0 - 0.5) * 2.0 * r;
			let tx = (cx + ox).round() as isize;
			let ty = (cy + oy).round() as isize;

			if tx < 0 || tx >= w as isize || ty < 0 || ty >= d as isize {
				continue;
			}

			let tx = tx as usize;
			let ty = ty as usize;
			let tz = find_top_exposed_z(world, tx, ty);
			let idx = world.index(tx, ty, tz);
			world.water_mass[idx] = world.water_mass[idx].saturating_add(RAIN_MASS_PER_DROP);
		}

		let consumed = drops * RAIN_MASS_PER_DROP as u32;
		let cloud = &mut world.clouds[i];
		cloud.water = cloud.water.saturating_sub(consumed);

		if cloud.water == 0 {
			world.clouds.remove(i);
			continue;
		}

		i += 1;
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tile::Tile;
	use crate::world::World;

	#[test]
	fn cloud_spawn_requires_threshold() {
		let mut w = World::new(8, 8, 4);
		w.atmospheric_moisture = CLOUD_THRESHOLD - 1;
		pass_cloud_spawn(&mut w);
		assert!(w.clouds.is_empty(), "Should not spawn below threshold");
	}

	#[test]
	fn cloud_spawn_at_threshold() {
		let mut w = World::new(8, 8, 4);
		w.atmospheric_moisture = CLOUD_THRESHOLD;
		pass_cloud_spawn(&mut w);
		assert_eq!(w.clouds.len(), 1, "Should spawn one cloud");
		assert_eq!(
			w.atmospheric_moisture,
			CLOUD_THRESHOLD - CLOUD_WATER,
			"Should deduct CLOUD_WATER"
		);
	}

	#[test]
	fn cloud_spawn_respects_max() {
		let mut w = World::new(8, 8, 4);
		w.atmospheric_moisture = CLOUD_THRESHOLD * 10;
		for _ in 0..MAX_CLOUDS + 2 {
			pass_cloud_spawn(&mut w);
		}
		assert_eq!(w.clouds.len(), MAX_CLOUDS, "Should not exceed MAX_CLOUDS");
	}

	#[test]
	fn cloud_update_moves_cloud() {
		let mut w = World::new(8, 8, 4);
		w.clouds.push(Cloud {
			x: 4.0, y: 4.0, dx: 0.3, dy: 0.0,
			water: 8000, radius: 2.5,
		});
		let old_x = w.clouds[0].x;
		pass_cloud_update(&mut w);
		assert!(w.clouds[0].x > old_x, "Cloud should move right");
	}

	#[test]
	fn cloud_update_adds_water_mass() {
		let mut w = World::new(8, 8, 4);
		for x in 0..8 {
			for y in 0..8 {
				w.set(x, y, 0, Tile::Stone);
			}
		}
		w.clouds.push(Cloud {
			x: 4.0, y: 4.0, dx: 0.0, dy: 0.0,
			water: 8000, radius: 2.5,
		});
		pass_cloud_update(&mut w);
		let total: u32 = (0..8)
			.flat_map(|x| (0..8).map(move |y| (x, y)))
			.map(|(x, y)| w.water_mass(x, y, 1) as u32)
			.sum();
		assert!(total > 0, "Rain should add water mass: {}", total);
	}

	#[test]
	fn cloud_removed_when_out_of_bounds() {
		let mut w = World::new(8, 8, 4);
		w.clouds.push(Cloud {
			x: 7.5, y: 4.0, dx: CLOUD_SPEED, dy: 0.0,
			water: 8000, radius: 2.5,
		});
		for _ in 0..100 {
			pass_cloud_update(&mut w);
		}
		assert!(w.clouds.is_empty(), "Cloud should be removed when out of bounds");
	}

	#[test]
	fn cloud_removed_when_water_depleted() {
		let mut w = World::new(8, 8, 4);
		for x in 0..8 {
			for y in 0..8 {
				w.set(x, y, 0, Tile::Stone);
			}
		}
		w.clouds.push(Cloud {
			x: 4.0, y: 4.0, dx: 0.0, dy: 0.0,
			water: 4, radius: 2.5,
		});
		pass_cloud_update(&mut w);
		assert!(w.clouds.is_empty(), "Cloud with minimal water should deplete in one tick");
	}
}
