# Cell-Based Architecture Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Level 개념을 제거하고 Tile을 enum으로 재설계하여, 바이너리 셀 기반 Falling Sand 스타일 물 시뮬레이션을 구현한다.

**Architecture:** Tile struct → Tile enum 전환, WaterCell/WaterState 제거, Water를 Tile variant로 통합. height 32→128, pack u16→u8. Water simulation을 4개 파일(gravity/spread/erosion/source)로 분리.

**Tech Stack:** Rust (wasm-bindgen), TypeScript, React, Canvas 2D isometric rendering

---

### Task 1: Tile Enum + FlowDir 정의

**Files:**
- Modify: `core/src/tile.rs` (전체 재작성)
- Test: `core/src/tile.rs` (inline tests)

**Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_is_solid() {
        assert!(!Tile::Air.is_solid());
        assert!(Tile::Grass.is_solid());
        assert!(Tile::Dirt.is_solid());
        assert!(Tile::Stone.is_solid());
        assert!(Tile::Sand.is_solid());
        assert!(!Tile::Water { is_source: false, sediment: 0, velocity: 0, direction: FlowDir::None }.is_solid());
    }

    #[test]
    fn tile_is_erodible() {
        assert!(!Tile::Air.is_erodible());
        assert!(Tile::Grass.is_erodible());
        assert!(Tile::Dirt.is_erodible());
        assert!(!Tile::Stone.is_erodible());
        assert!(Tile::Sand.is_erodible());
    }

    #[test]
    fn tile_pack_roundtrip_solid() {
        assert_eq!(Tile::Air.pack(), 0);
        assert_eq!(Tile::unpack(Tile::Grass.pack()), Tile::Grass);
        assert_eq!(Tile::unpack(Tile::Stone.pack()), Tile::Stone);
    }

    #[test]
    fn tile_pack_roundtrip_water() {
        let w = Tile::Water {
            is_source: true,
            sediment: 5,
            velocity: 3,
            direction: FlowDir::East,
        };
        let packed = w.pack();
        let unpacked = Tile::unpack(packed);
        // Water unpacking only preserves is_source and direction (rendering fields)
        // sediment/velocity are internal-only
        match unpacked {
            Tile::Water { is_source, direction, .. } => {
                assert!(is_source);
                assert_eq!(direction, FlowDir::East);
            }
            _ => panic!("expected Water"),
        }
    }

    #[test]
    fn tile_opacity() {
        assert_eq!(Tile::Air.opacity(), 0);
        assert_eq!(Tile::Grass.opacity(), 10);
        assert_eq!(Tile::Water { is_source: false, sediment: 0, velocity: 0, direction: FlowDir::None }.opacity(), 3);
    }

    #[test]
    fn flow_dir_all_variants() {
        assert_eq!(FlowDir::from_u8(0), FlowDir::None);
        assert_eq!(FlowDir::from_u8(1), FlowDir::Down);
        assert_eq!(FlowDir::from_u8(5), FlowDir::West);
        assert_eq!(FlowDir::from_u8(6), FlowDir::None); // out of range
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cd /home/croo12/croo12/core && cargo test tile::tests -- --nocapture`
Expected: FAIL — `Tile` enum, `FlowDir` enum not defined

**Step 3: Write minimal implementation**

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FlowDir {
    None = 0,
    Down = 1,
    North = 2,
    South = 3,
    East = 4,
    West = 5,
}

impl FlowDir {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::None,
            1 => Self::Down,
            2 => Self::North,
            3 => Self::South,
            4 => Self::East,
            5 => Self::West,
            _ => Self::None,
        }
    }

    pub fn to_u8(self) -> u8 {
        self as u8
    }

    pub fn opposite(self) -> Self {
        match self {
            Self::None => Self::None,
            Self::Down => Self::None, // no "Up" direction
            Self::North => Self::South,
            Self::South => Self::North,
            Self::East => Self::West,
            Self::West => Self::East,
        }
    }

    pub fn perpendiculars(self) -> [Self; 2] {
        match self {
            Self::North | Self::South => [Self::East, Self::West],
            Self::East | Self::West => [Self::North, Self::South],
            _ => [Self::North, Self::East],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tile {
    Air,
    Grass,
    Dirt,
    Stone,
    Sand,
    Water {
        is_source: bool,
        sediment: u8,
        velocity: u8,
        direction: FlowDir,
    },
}

impl Tile {
    pub fn water_default() -> Self {
        Self::Water {
            is_source: false,
            sediment: 0,
            velocity: 0,
            direction: FlowDir::None,
        }
    }

    pub fn water_source() -> Self {
        Self::Water {
            is_source: true,
            sediment: 0,
            velocity: 0,
            direction: FlowDir::None,
        }
    }

    pub fn is_solid(&self) -> bool {
        matches!(self, Self::Grass | Self::Dirt | Self::Stone | Self::Sand)
    }

    pub fn is_erodible(&self) -> bool {
        matches!(self, Self::Grass | Self::Dirt | Self::Sand)
    }

    pub fn is_water(&self) -> bool {
        matches!(self, Self::Water { .. })
    }

    pub fn is_air(&self) -> bool {
        matches!(self, Self::Air)
    }

    /// Opacity for visibility scoring. 10 = fully opaque, 0 = transparent.
    pub fn opacity(&self) -> u8 {
        match self {
            Self::Air => 0,
            Self::Water { .. } => 3,
            _ => 10,
        }
    }

    fn type_id(&self) -> u8 {
        match self {
            Self::Air => 0,
            Self::Grass => 1,
            Self::Dirt => 2,
            Self::Stone => 3,
            Self::Sand => 4,
            Self::Water { .. } => 5,
        }
    }

    /// Pack for WASM export: u8
    /// Bits 0-2: tile_type (0-5)
    /// Bits 3-5: direction (0-5), Water only
    /// Bit 6: is_source, Water only
    /// Bit 7: unused
    pub fn pack(&self) -> u8 {
        let type_bits = self.type_id() & 0x07;
        match self {
            Self::Water { is_source, direction, .. } => {
                let dir_bits = (direction.to_u8() & 0x07) << 3;
                let src_bit = if *is_source { 1 << 6 } else { 0 };
                type_bits | dir_bits | src_bit
            }
            _ => type_bits,
        }
    }

    pub fn unpack(packed: u8) -> Self {
        let type_id = packed & 0x07;
        match type_id {
            0 => Self::Air,
            1 => Self::Grass,
            2 => Self::Dirt,
            3 => Self::Stone,
            4 => Self::Sand,
            5 => {
                let dir = FlowDir::from_u8((packed >> 3) & 0x07);
                let is_source = (packed & (1 << 6)) != 0;
                Self::Water {
                    is_source,
                    sediment: 0,
                    velocity: 0,
                    direction: dir,
                }
            }
            _ => Self::Air,
        }
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cd /home/croo12/croo12/core && cargo test tile::tests -- --nocapture`
Expected: All 6 tests PASS

**Step 5: Commit**

```bash
git add core/src/tile.rs
git commit -m "refactor: replace Tile struct with enum, add FlowDir"
```

---

### Task 2: World struct 단순화

**Files:**
- Modify: `core/src/world.rs` (전체 재작성)
- Test: `core/src/world.rs` (inline tests)

**Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tile::{Tile, FlowDir};

    #[test]
    fn world_new_creates_air_grid() {
        let world = World::new(4, 4, 8);
        assert_eq!(world.width(), 4);
        assert_eq!(world.depth(), 4);
        assert_eq!(world.height(), 8);
        assert_eq!(world.get(0, 0, 0), Tile::Air);
    }

    #[test]
    fn world_set_and_get() {
        let mut world = World::new(4, 4, 8);
        world.set(1, 2, 3, Tile::Stone);
        assert_eq!(world.get(1, 2, 3), Tile::Stone);
        assert_eq!(world.get(0, 0, 0), Tile::Air);
    }

    #[test]
    fn world_sync_cache_packs_correctly() {
        let mut world = World::new(4, 4, 8);
        world.set(0, 0, 0, Tile::Grass);
        world.set(1, 0, 0, Tile::Water {
            is_source: true,
            sediment: 0,
            velocity: 0,
            direction: FlowDir::East,
        });
        world.sync_tiles_cache();
        assert_eq!(world.tiles_cache()[0], Tile::Grass.pack());
        assert_eq!(world.tiles_cache()[1] & 0x07, 5); // Water type
        assert_eq!((world.tiles_cache()[1] >> 6) & 1, 1); // is_source
    }

    #[test]
    fn world_tiles_cache_ptr_and_len() {
        let world = World::new(4, 4, 8);
        assert_eq!(world.tiles_cache_len(), 4 * 4 * 8);
        assert!(!world.tiles_cache_ptr().is_null());
    }

    #[test]
    fn world_tiles_mut_allows_modification() {
        let mut world = World::new(4, 4, 8);
        world.tiles_mut()[0] = Tile::Stone;
        assert_eq!(world.get(0, 0, 0), Tile::Stone);
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cd /home/croo12/croo12/core && cargo test world::tests -- --nocapture`
Expected: FAIL — World struct changed

**Step 3: Write minimal implementation**

```rust
use crate::tile::Tile;

pub struct World {
    width: usize,
    depth: usize,
    height: usize,
    tiles: Vec<Tile>,
    tiles_cache: Vec<u8>,
}

impl World {
    pub fn new(width: usize, depth: usize, height: usize) -> Self {
        let size = width * depth * height;
        Self {
            width,
            depth,
            height,
            tiles: vec![Tile::Air; size],
            tiles_cache: vec![0u8; size],
        }
    }

    pub fn width(&self) -> usize { self.width }
    pub fn depth(&self) -> usize { self.depth }
    pub fn height(&self) -> usize { self.height }

    pub fn index(&self, x: usize, y: usize, z: usize) -> usize {
        x + y * self.width + z * self.width * self.depth
    }

    pub fn get(&self, x: usize, y: usize, z: usize) -> Tile {
        self.tiles[self.index(x, y, z)]
    }

    pub fn set(&mut self, x: usize, y: usize, z: usize, tile: Tile) {
        let idx = self.index(x, y, z);
        self.tiles[idx] = tile;
    }

    pub fn tiles(&self) -> &[Tile] { &self.tiles }
    pub fn tiles_mut(&mut self) -> &mut [Tile] { &mut self.tiles }

    pub fn sync_tiles_cache(&mut self) {
        for (i, tile) in self.tiles.iter().enumerate() {
            self.tiles_cache[i] = tile.pack();
        }
    }

    pub fn tiles_cache(&self) -> &[u8] { &self.tiles_cache }
    pub fn tiles_cache_ptr(&self) -> *const u8 { self.tiles_cache.as_ptr() }
    pub fn tiles_cache_len(&self) -> usize { self.tiles_cache.len() }

    pub fn in_bounds(&self, x: usize, y: usize, z: usize) -> bool {
        x < self.width && y < self.depth && z < self.height
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cd /home/croo12/croo12/core && cargo test world::tests -- --nocapture`
Expected: All 5 tests PASS

**Step 5: Commit**

```bash
git add core/src/world.rs
git commit -m "refactor: simplify World struct, remove WaterState and generics"
```

---

### Task 3: Water simulation 분리 — mod.rs + gravity.rs

**Files:**
- Modify: `core/src/water/mod.rs` (전체 재작성)
- Create: `core/src/water/gravity.rs`
- Test: inline tests in both files

**Step 1: Write the failing tests**

`core/src/water/mod.rs` tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tile::{Tile, FlowDir};
    use crate::world::World;

    #[test]
    fn tick_moves_water_down() {
        let mut world = World::new(4, 4, 8);
        world.set(0, 0, 0, Tile::Stone); // ground
        world.set(0, 0, 2, Tile::water_default()); // water at z=2
        tick(&mut world);
        assert!(world.get(0, 0, 1).is_water()); // moved to z=1
        assert!(world.get(0, 0, 2).is_air()); // vacated
    }
}
```

`core/src/water/gravity.rs` tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tile::{Tile, FlowDir};
    use crate::world::World;

    #[test]
    fn water_falls_one_cell() {
        let mut world = World::new(4, 4, 8);
        world.set(0, 0, 3, Tile::water_default());
        pass_gravity(&mut world);
        assert!(world.get(0, 0, 2).is_water());
        assert!(world.get(0, 0, 3).is_air());
    }

    #[test]
    fn water_stops_on_solid() {
        let mut world = World::new(4, 4, 8);
        world.set(0, 0, 0, Tile::Stone);
        world.set(0, 0, 1, Tile::water_default());
        pass_gravity(&mut world);
        assert!(world.get(0, 0, 1).is_water()); // stays
    }

    #[test]
    fn falling_water_accelerates() {
        let mut world = World::new(4, 4, 8);
        world.set(0, 0, 4, Tile::Water {
            is_source: false, sediment: 0, velocity: 2, direction: FlowDir::Down,
        });
        pass_gravity(&mut world);
        match world.get(0, 0, 3) {
            Tile::Water { velocity, direction, .. } => {
                assert_eq!(velocity, 3); // vel + 1
                assert_eq!(direction, FlowDir::Down);
            }
            _ => panic!("expected Water"),
        }
    }

    #[test]
    fn falling_water_max_velocity() {
        let mut world = World::new(4, 4, 8);
        world.set(0, 0, 2, Tile::Water {
            is_source: false, sediment: 0, velocity: 7, direction: FlowDir::Down,
        });
        pass_gravity(&mut world);
        match world.get(0, 0, 1) {
            Tile::Water { velocity, .. } => assert_eq!(velocity, 7), // capped
            _ => panic!("expected Water"),
        }
    }

    #[test]
    fn water_does_not_fall_through_water() {
        let mut world = World::new(4, 4, 8);
        world.set(0, 0, 1, Tile::water_default());
        world.set(0, 0, 2, Tile::water_default());
        pass_gravity(&mut world);
        assert!(world.get(0, 0, 1).is_water());
        assert!(world.get(0, 0, 2).is_water()); // blocked
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cd /home/croo12/croo12/core && cargo test water:: -- --nocapture`
Expected: FAIL — modules not defined

**Step 3: Write minimal implementation**

`core/src/water/gravity.rs`:

```rust
use crate::tile::{Tile, FlowDir};
use crate::world::World;

pub fn pass_gravity(world: &mut World) {
    let w = world.width();
    let d = world.depth();
    let h = world.height();

    // Iterate top-to-bottom so water falls naturally
    for z in (1..h).rev() {
        for y in 0..d {
            for x in 0..w {
                let tile = world.get(x, y, z);
                if let Tile::Water { is_source, sediment, velocity, .. } = tile {
                    let below = world.get(x, y, z - 1);
                    if below.is_air() {
                        let new_vel = (velocity + 1).min(7);
                        world.set(x, y, z - 1, Tile::Water {
                            is_source: false,
                            sediment,
                            velocity: new_vel,
                            direction: FlowDir::Down,
                        });
                        if is_source {
                            // Source stays, reset velocity
                            world.set(x, y, z, Tile::water_source());
                        } else {
                            world.set(x, y, z, Tile::Air);
                        }
                    }
                }
            }
        }
    }
}
```

`core/src/water/mod.rs`:

```rust
pub mod gravity;
pub mod spread;
pub mod erosion;
pub mod source;

use crate::world::World;

pub fn tick(world: &mut World) {
    gravity::pass_gravity(world);
    spread::pass_spread(world);
    erosion::pass_erosion(world);
    source::pass_source(world);
    world.sync_tiles_cache();
}
```

Also create stubs for `spread.rs`, `erosion.rs`, `source.rs`:

`core/src/water/spread.rs`:
```rust
use crate::world::World;
pub fn pass_spread(_world: &mut World) {}
```

`core/src/water/erosion.rs`:
```rust
use crate::world::World;
pub fn pass_erosion(_world: &mut World) {}
```

`core/src/water/source.rs`:
```rust
use crate::world::World;
pub fn pass_source(_world: &mut World) {}
```

**Step 4: Run tests to verify they pass**

Run: `cd /home/croo12/croo12/core && cargo test water:: -- --nocapture`
Expected: All gravity + mod tests PASS

**Step 5: Commit**

```bash
git add core/src/water/
git commit -m "refactor: split water module, implement gravity pass"
```

---

### Task 4: Water spread pass

**Files:**
- Modify: `core/src/water/spread.rs`
- Test: inline tests

**Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tile::{Tile, FlowDir};
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
        world.set(3, 1, 1, Tile::Water {
            is_source: false, sediment: 0, velocity: 3, direction: FlowDir::East,
        });
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
        world.set(3, 3, 1, Tile::Water {
            is_source: false, sediment: 0, velocity: 5, direction: FlowDir::East,
        });
        pass_spread(&mut world);
        assert!(world.get(4, 3, 1).is_water()); // moved East
        assert!(world.get(3, 3, 1).is_air());
    }

    #[test]
    fn stagnant_water_stays() {
        let mut world = World::new(4, 4, 4);
        // Surrounded by solid
        for x in 0..4 { for y in 0..4 { world.set(x, y, 0, Tile::Stone); } }
        world.set(0, 1, 0, Tile::Stone); world.set(2, 1, 0, Tile::Stone);
        world.set(1, 0, 0, Tile::Stone); world.set(1, 2, 0, Tile::Stone);
        world.set(0, 1, 1, Tile::Stone); world.set(2, 1, 1, Tile::Stone);
        world.set(1, 0, 1, Tile::Stone); world.set(1, 2, 1, Tile::Stone);
        world.set(1, 1, 0, Tile::Stone); // floor
        world.set(1, 1, 1, Tile::water_default());
        pass_spread(&mut world);
        assert!(world.get(1, 1, 1).is_water()); // no move
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cd /home/croo12/croo12/core && cargo test water::spread::tests -- --nocapture`
Expected: FAIL — `pass_spread` is stub

**Step 3: Write minimal implementation**

```rust
use crate::tile::{Tile, FlowDir};
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
        (FlowDir::North, if y > 0 { Some((x, y - 1)) } else { None }),
        (FlowDir::South, if y + 1 < d { Some((x, y + 1)) } else { None }),
        (FlowDir::East, if x + 1 < w { Some((x + 1, y)) } else { None }),
        (FlowDir::West, if x > 0 { Some((x - 1, y)) } else { None }),
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
            if !target.is_air() { continue; }
            let depth = scan_depth(world, *nx, *ny, z);
            let priority = if *dir == current_dir {
                0
            } else if *dir == opposite {
                2
            } else {
                1
            };
            candidates.push(Candidate { dir: *dir, nx: *nx, ny: *ny, depth, priority });
        }
    }

    if candidates.is_empty() { return None; }

    // High velocity: only forward
    if velocity >= 4 {
        if let Some(c) = candidates.iter().find(|c| c.priority == 0) {
            return Some((c.dir, c.nx, c.ny));
        }
    }

    // Sort: priority asc, then depth desc
    candidates.sort_by(|a, b| {
        a.priority.cmp(&b.priority).then(b.depth.cmp(&a.depth))
    });

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
                if let Tile::Water { is_source, sediment, velocity, direction } = tile {
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

                    if let Some((new_dir, nx, ny)) = pick_direction(world, x, y, z, dir, velocity) {
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
                            world.set(x, y, z, Tile::Water {
                                is_source,
                                sediment,
                                velocity: 0,
                                direction: FlowDir::None,
                            });
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
```

**Step 4: Run tests to verify they pass**

Run: `cd /home/croo12/croo12/core && cargo test water::spread::tests -- --nocapture`
Expected: All spread tests PASS

**Step 5: Commit**

```bash
git add core/src/water/spread.rs
git commit -m "feat: implement direction-based horizontal water spread"
```

---

### Task 5: Water erosion pass

**Files:**
- Modify: `core/src/water/erosion.rs`
- Test: inline tests

**Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tile::{Tile, FlowDir};
    use crate::world::World;

    #[test]
    fn stagnant_water_does_not_erode() {
        let mut world = World::new(4, 4, 4);
        world.set(1, 1, 0, Tile::Dirt);
        world.set(1, 1, 1, Tile::Water {
            is_source: false, sediment: 0, velocity: 0, direction: FlowDir::None,
        });
        pass_erosion(&mut world);
        assert_eq!(world.get(1, 1, 0), Tile::Dirt); // not eroded
    }

    #[test]
    fn moving_water_can_erode() {
        let mut world = World::new(4, 4, 4);
        world.set(1, 1, 0, Tile::Sand);
        world.set(1, 1, 1, Tile::Water {
            is_source: false, sediment: 0, velocity: 7, direction: FlowDir::East,
        });
        // With velocity=7, erosion chance = 14%. Run many times to check it's possible.
        let mut eroded = false;
        for seed in 0..200u64 {
            let mut w = World::new(4, 4, 4);
            w.set(1, 1, 0, Tile::Sand);
            w.set(1, 1, 1, Tile::Water {
                is_source: false, sediment: 0, velocity: 7, direction: FlowDir::East,
            });
            pass_erosion_with_seed(&mut w, seed);
            if w.get(1, 1, 0).is_air() {
                eroded = true;
                break;
            }
        }
        assert!(eroded, "Should erode at least once in 200 attempts");
    }

    #[test]
    fn stone_never_erodes() {
        let mut world = World::new(4, 4, 4);
        world.set(1, 1, 0, Tile::Stone);
        world.set(1, 1, 1, Tile::Water {
            is_source: false, sediment: 0, velocity: 7, direction: FlowDir::East,
        });
        for seed in 0..100u64 {
            let mut w = World::new(4, 4, 4);
            w.set(1, 1, 0, Tile::Stone);
            w.set(1, 1, 1, Tile::Water {
                is_source: false, sediment: 0, velocity: 7, direction: FlowDir::East,
            });
            pass_erosion_with_seed(&mut w, seed);
            assert_eq!(w.get(1, 1, 0), Tile::Stone);
        }
    }

    #[test]
    fn sediment_deposits_when_stagnant() {
        let mut world = World::new(4, 4, 4);
        world.set(1, 1, 0, Tile::Stone); // floor
        world.set(1, 1, 1, Tile::Water {
            is_source: false, sediment: 3, velocity: 0, direction: FlowDir::None,
        });
        // Adjacent air for deposition
        pass_erosion(&mut world);
        // Check if sediment decreased
        if let Tile::Water { sediment, .. } = world.get(1, 1, 1) {
            // Sediment may have deposited as Sand somewhere adjacent
            assert!(sediment <= 3);
        }
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cd /home/croo12/croo12/core && cargo test water::erosion::tests -- --nocapture`
Expected: FAIL

**Step 3: Write minimal implementation**

```rust
use crate::tile::{Tile, FlowDir};
use crate::world::World;

/// Simple hash for deterministic pseudo-random erosion
fn simple_hash(x: usize, y: usize, z: usize, seed: u64) -> u64 {
    let mut h = seed;
    h = h.wrapping_mul(6364136223846793005).wrapping_add(x as u64);
    h = h.wrapping_mul(6364136223846793005).wrapping_add(y as u64);
    h = h.wrapping_mul(6364136223846793005).wrapping_add(z as u64);
    h ^ (h >> 33)
}

static mut EROSION_TICK: u64 = 0;

pub fn pass_erosion(world: &mut World) {
    let seed = unsafe { EROSION_TICK };
    unsafe { EROSION_TICK = EROSION_TICK.wrapping_add(1); }
    pass_erosion_with_seed(world, seed);
}

pub fn pass_erosion_with_seed(world: &mut World, seed: u64) {
    let w = world.width();
    let d = world.depth();
    let h = world.height();

    // Sub-pass A: Erosion (moving water erodes below)
    for z in 1..h {
        for y in 0..d {
            for x in 0..w {
                let tile = world.get(x, y, z);
                if let Tile::Water { is_source, sediment, velocity, direction } = tile {
                    if velocity == 0 { continue; }

                    let below = world.get(x, y, z - 1);
                    if !below.is_erodible() { continue; }

                    // Erosion chance: velocity * 2 out of 100
                    let chance = (velocity as u64) * 2;
                    let roll = simple_hash(x, y, z, seed) % 100;
                    if roll < chance {
                        world.set(x, y, z - 1, Tile::Air);
                        let new_sed = (sediment + 1).min(7);
                        world.set(x, y, z, Tile::Water {
                            is_source,
                            sediment: new_sed,
                            velocity,
                            direction,
                        });
                    }
                }
            }
        }
    }

    // Sub-pass B: Deposition (stagnant water with sediment deposits Sand)
    let neighbors: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

    for z in 0..h {
        for y in 0..d {
            for x in 0..w {
                let tile = world.get(x, y, z);
                if let Tile::Water { is_source, sediment, velocity, direction } = tile {
                    if velocity != 0 || sediment == 0 { continue; }

                    // Find adjacent Air cell for deposition
                    for (dx, dy) in &neighbors {
                        let nx = x as i32 + dx;
                        let ny = y as i32 + dy;
                        if nx < 0 || nx >= w as i32 || ny < 0 || ny >= d as i32 { continue; }
                        let (nx, ny) = (nx as usize, ny as usize);
                        if world.get(nx, ny, z).is_air() {
                            world.set(nx, ny, z, Tile::Sand);
                            let new_sed = sediment - 1;
                            world.set(x, y, z, Tile::Water {
                                is_source,
                                sediment: new_sed,
                                velocity,
                                direction,
                            });
                            break; // one deposition per tick
                        }
                    }
                }
            }
        }
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cd /home/croo12/croo12/core && cargo test water::erosion::tests -- --nocapture`
Expected: All erosion tests PASS

**Step 5: Commit**

```bash
git add core/src/water/erosion.rs
git commit -m "feat: implement velocity-based erosion and deposition"
```

---

### Task 6: Water source pass

**Files:**
- Modify: `core/src/water/source.rs`
- Modify: `core/src/world.rs` (add source tracking)
- Test: inline tests

**Step 1: Write the failing tests**

`core/src/water/source.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tile::Tile;
    use crate::world::World;

    #[test]
    fn source_replenishes_when_empty() {
        let mut world = World::new(4, 4, 4);
        let sources = vec![(1, 1, 2)];
        // Source position is Air (water moved away)
        pass_source(&mut world, &sources);
        assert!(world.get(1, 1, 2).is_water());
        if let Tile::Water { is_source, .. } = world.get(1, 1, 2) {
            assert!(is_source);
        }
    }

    #[test]
    fn source_does_not_overwrite_solid() {
        let mut world = World::new(4, 4, 4);
        world.set(1, 1, 2, Tile::Stone);
        let sources = vec![(1, 1, 2)];
        pass_source(&mut world, &sources);
        assert_eq!(world.get(1, 1, 2), Tile::Stone); // not overwritten
    }

    #[test]
    fn source_does_not_overwrite_existing_water() {
        let mut world = World::new(4, 4, 4);
        world.set(1, 1, 2, Tile::water_default());
        let sources = vec![(1, 1, 2)];
        pass_source(&mut world, &sources);
        // Should mark as source
        if let Tile::Water { is_source, .. } = world.get(1, 1, 2) {
            assert!(is_source);
        }
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cd /home/croo12/croo12/core && cargo test water::source::tests -- --nocapture`
Expected: FAIL

**Step 3: Write minimal implementation**

`core/src/water/source.rs`:

```rust
use crate::tile::Tile;
use crate::world::World;

pub fn pass_source(world: &mut World, sources: &[(usize, usize, usize)]) {
    for &(x, y, z) in sources {
        let tile = world.get(x, y, z);
        match tile {
            Tile::Air => {
                world.set(x, y, z, Tile::water_source());
            }
            Tile::Water { sediment, velocity, direction, .. } => {
                world.set(x, y, z, Tile::Water {
                    is_source: true,
                    sediment,
                    velocity,
                    direction,
                });
            }
            _ => {} // don't overwrite solid
        }
    }
}
```

Update `core/src/world.rs` — add `sources` field:

```rust
// Add to World struct:
sources: Vec<(usize, usize, usize)>,

// Add to new():
sources: Vec::new(),

// Add methods:
pub fn sources(&self) -> &[(usize, usize, usize)] { &self.sources }
pub fn add_source(&mut self, x: usize, y: usize, z: usize) { self.sources.push((x, y, z)); }
pub fn clear_sources(&mut self) { self.sources.clear(); }
```

Update `core/src/water/mod.rs` tick:

```rust
pub fn tick(world: &mut World) {
    gravity::pass_gravity(world);
    spread::pass_spread(world);
    erosion::pass_erosion(world);
    let sources: Vec<_> = world.sources().to_vec();
    source::pass_source(world, &sources);
    world.sync_tiles_cache();
}
```

**Step 4: Run tests to verify they pass**

Run: `cd /home/croo12/croo12/core && cargo test water::source::tests -- --nocapture`
Expected: All source tests PASS

**Step 5: Commit**

```bash
git add core/src/water/source.rs core/src/water/mod.rs core/src/world.rs
git commit -m "feat: implement water source replenishment pass"
```

---

### Task 7: Terrain generation 업데이트

**Files:**
- Modify: `core/src/terrain.rs` (전체 재작성)
- Test: inline tests

**Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tile::Tile;
    use crate::world::World;

    #[test]
    fn terrain_fills_ground() {
        let mut world = World::new(8, 8, 128);
        generate_terrain(&mut world, 42);
        // Bottom should be Stone
        assert_eq!(world.get(0, 0, 0), Tile::Stone);
    }

    #[test]
    fn terrain_has_air_at_top() {
        let mut world = World::new(8, 8, 128);
        generate_terrain(&mut world, 42);
        assert_eq!(world.get(0, 0, 127), Tile::Air);
    }

    #[test]
    fn terrain_has_grass_surface() {
        let mut world = World::new(8, 8, 128);
        generate_terrain(&mut world, 42);
        let mut found_grass = false;
        for x in 0..8 {
            for y in 0..8 {
                for z in 0..128 {
                    if world.get(x, y, z) == Tile::Grass {
                        found_grass = true;
                    }
                }
            }
        }
        assert!(found_grass);
    }

    #[test]
    fn terrain_has_water_sources() {
        let mut world = World::new(16, 16, 128);
        generate_terrain(&mut world, 42);
        assert!(!world.sources().is_empty());
    }

    #[test]
    fn terrain_is_deterministic() {
        let mut w1 = World::new(8, 8, 128);
        let mut w2 = World::new(8, 8, 128);
        generate_terrain(&mut w1, 99);
        generate_terrain(&mut w2, 99);
        for i in 0..(8*8*128) {
            assert_eq!(w1.tiles()[i], w2.tiles()[i]);
        }
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cd /home/croo12/croo12/core && cargo test terrain::tests -- --nocapture`
Expected: FAIL — function signature changed

**Step 3: Write minimal implementation**

```rust
use noise::{Fbm, MultiFractal, NoiseFn, Perlin};
use crate::tile::Tile;
use crate::world::World;

const WATER_LEVEL: usize = 32;
const SEA_FLOOR: usize = 16;
const NOISE_SCALE: f64 = 0.03;
const RIVER_SOURCE_COUNT: usize = 3;
const DIRT_LAYERS: usize = 3;

pub fn generate_terrain(world: &mut World, seed: u32) {
    let w = world.width();
    let d = world.depth();
    let h = world.height();

    let fbm = Fbm::<Perlin>::new(seed)
        .set_octaves(4)
        .set_frequency(NOISE_SCALE);

    // Generate surface height map
    let mut surface_heights = vec![0usize; w * d];
    for y in 0..d {
        for x in 0..w {
            let val = fbm.get([x as f64, y as f64]);
            let normalized = (val + 1.0) / 2.0; // 0.0..1.0
            let surface = SEA_FLOOR + (normalized * (h - SEA_FLOOR) as f64 * 0.6) as usize;
            surface_heights[x + y * w] = surface.min(h - 1);
        }
    }

    // Fill terrain column by column
    for y in 0..d {
        for x in 0..w {
            let surface = surface_heights[x + y * w];
            for z in 0..h {
                let tile = if z > surface {
                    Tile::Air
                } else if z == surface {
                    if surface <= WATER_LEVEL {
                        Tile::Sand
                    } else {
                        Tile::Grass
                    }
                } else if z > surface.saturating_sub(DIRT_LAYERS) {
                    Tile::Dirt
                } else {
                    Tile::Stone
                };
                world.set(x, y, z, tile);
            }
        }
    }

    // Place water sources at high points
    let mut candidates: Vec<(usize, usize, usize)> = Vec::new();
    for y in 0..d {
        for x in 0..w {
            let s = surface_heights[x + y * w];
            if s > WATER_LEVEL {
                candidates.push((x, y, s + 1));
            }
        }
    }

    // Sort by height descending, pick top N
    candidates.sort_by(|a, b| b.2.cmp(&a.2));

    let count = RIVER_SOURCE_COUNT.min(candidates.len());
    // Deterministic selection using seed
    let step = if candidates.len() > count { candidates.len() / count } else { 1 };
    for i in 0..count {
        let idx = (i * step + seed as usize) % candidates.len();
        let (sx, sy, sz) = candidates[idx];
        if sz < h {
            world.set(sx, sy, sz, Tile::water_source());
            world.add_source(sx, sy, sz);
        }
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cd /home/croo12/croo12/core && cargo test terrain::tests -- --nocapture`
Expected: All 5 tests PASS

**Step 5: Commit**

```bash
git add core/src/terrain.rs
git commit -m "refactor: update terrain gen for 128 height, source-based water"
```

---

### Task 8: WASM exports (lib.rs) 업데이트

**Files:**
- Modify: `core/src/lib.rs` (전체 재작성)
- Test: manual WASM build

**Step 1: Write the implementation**

```rust
mod tile;
mod world;
mod water;
mod terrain;
mod render;

use std::cell::UnsafeCell;
use wasm_bindgen::prelude::*;
use world::World;

struct WorldHolder(UnsafeCell<Option<World>>);
unsafe impl Sync for WorldHolder {}

static WORLD: WorldHolder = WorldHolder(UnsafeCell::new(None));

fn with_world<T>(f: impl FnOnce(&World) -> T) -> T {
    unsafe {
        let world = &*WORLD.0.get();
        f(world.as_ref().expect("World not initialized"))
    }
}

fn with_world_mut<T>(f: impl FnOnce(&mut World) -> T) -> T {
    unsafe {
        let world = &mut *WORLD.0.get();
        f(world.as_mut().expect("World not initialized"))
    }
}

#[wasm_bindgen]
pub fn greet() -> String {
    "Hello from game_core!".to_string()
}

#[wasm_bindgen]
pub fn create_world(width: usize, depth: usize, height: usize, seed: u32) {
    let mut world = World::new(width, depth, height);
    terrain::generate_terrain(&mut world, seed);
    world.sync_tiles_cache();
    unsafe {
        *WORLD.0.get() = Some(world);
    }
}

#[wasm_bindgen]
pub fn world_width() -> usize { with_world(|w| w.width()) }

#[wasm_bindgen]
pub fn world_depth() -> usize { with_world(|w| w.depth()) }

#[wasm_bindgen]
pub fn world_height() -> usize { with_world(|w| w.height()) }

#[wasm_bindgen]
pub fn world_tiles_ptr() -> *const u8 { with_world(|w| w.tiles_cache_ptr()) }

#[wasm_bindgen]
pub fn world_tiles_len() -> usize { with_world(|w| w.tiles_cache_len()) }

#[wasm_bindgen]
pub fn tick_water() {
    with_world_mut(|w| water::tick(w));
}

#[wasm_bindgen]
pub fn place_water(x: usize, y: usize, z: usize) {
    with_world_mut(|w| {
        w.set(x, y, z, tile::Tile::water_default());
        w.sync_tiles_cache();
    });
}

#[wasm_bindgen]
pub fn place_water_source(x: usize, y: usize, z: usize) {
    with_world_mut(|w| {
        w.set(x, y, z, tile::Tile::water_source());
        w.add_source(x, y, z);
        w.sync_tiles_cache();
    });
}

#[wasm_bindgen]
pub fn remove_water(x: usize, y: usize, z: usize) {
    with_world_mut(|w| {
        w.set(x, y, z, tile::Tile::Air);
        w.sync_tiles_cache();
    });
}
```

**Step 2: Run Rust tests**

Run: `cd /home/croo12/croo12/core && cargo test`
Expected: All tests across all modules PASS

**Step 3: Delete old render/ascii.rs (depends on old API)**

Remove render module or update it to use new Tile enum. Simplest: delete `render/ascii.rs` and update `render/mod.rs` to empty module.

```rust
// render/mod.rs — empty for now, can be re-implemented later
```

**Step 4: Build WASM**

Run: `cd /home/croo12/croo12/core && wasm-pack build --target web --out-dir build`
Expected: Build succeeds

**Step 5: Commit**

```bash
git add core/src/lib.rs core/src/render/
git commit -m "refactor: update WASM exports for cell-based architecture"
```

---

### Task 9: TS TileType + WorldData 업데이트

**Files:**
- Modify: `src/entities/tile/model/tile-type.ts`
- Modify: `src/entities/tile/model/world-data.ts`
- Modify: `src/entities/tile/index.ts`
- Delete: `src/entities/water/` (entire directory)

**Step 1: Update tile-type.ts**

```typescript
export const TileType = {
    Air: 0,
    Grass: 1,
    Dirt: 2,
    Stone: 3,
    Sand: 4,
    Water: 5,
} as const;

export type TileTypeValue = (typeof TileType)[keyof typeof TileType];

export const FlowDir = {
    None: 0,
    Down: 1,
    North: 2,
    South: 3,
    East: 4,
    West: 5,
} as const;

export type FlowDirValue = (typeof FlowDir)[keyof typeof FlowDir];

const OPACITY: Record<TileTypeValue, number> = {
    [TileType.Air]: 0.0,
    [TileType.Grass]: 1.0,
    [TileType.Dirt]: 1.0,
    [TileType.Stone]: 1.0,
    [TileType.Sand]: 1.0,
    [TileType.Water]: 0.3,
};

export const getTileOpacity = (tileType: TileTypeValue): number =>
    OPACITY[tileType] ?? 0.0;
```

**Step 2: Update world-data.ts**

```typescript
import type { TileTypeValue, FlowDirValue } from "./tile-type";
import { TileType } from "./tile-type";

const TYPE_MASK = 0x07;
const DIR_SHIFT = 3;
const DIR_MASK = 0x07;
const SOURCE_BIT = 1 << 6;

export class WorldData {
    readonly width: number;
    readonly depth: number;
    readonly height: number;
    private tiles: Uint8Array;

    constructor(
        width: number,
        depth: number,
        height: number,
        tiles: Uint8Array,
    ) {
        this.width = width;
        this.depth = depth;
        this.height = height;
        this.tiles = new Uint8Array(tiles);
    }

    private index(x: number, y: number, z: number): number {
        return x + y * this.width + z * this.width * this.depth;
    }

    getTile(x: number, y: number, z: number): TileTypeValue {
        return (this.tiles[this.index(x, y, z)] & TYPE_MASK) as TileTypeValue;
    }

    getFlowDir(x: number, y: number, z: number): FlowDirValue {
        return ((this.tiles[this.index(x, y, z)] >> DIR_SHIFT) & DIR_MASK) as FlowDirValue;
    }

    isSource(x: number, y: number, z: number): boolean {
        return (this.tiles[this.index(x, y, z)] & SOURCE_BIT) !== 0;
    }

    updateTiles(tiles: Uint8Array): void {
        this.tiles = new Uint8Array(tiles);
    }

    getTopZ(x: number, y: number): number {
        for (let z = this.height - 1; z >= 0; z--) {
            if (this.getTile(x, y, z) !== TileType.Air) {
                return z;
            }
        }
        return 0;
    }
}
```

**Step 3: Update index.ts exports**

```typescript
export { TileType, type TileTypeValue, FlowDir, type FlowDirValue, getTileOpacity } from "./model/tile-type";
export { WorldData } from "./model/world-data";
```

**Step 4: Delete water entity**

```bash
rm -rf src/entities/water/
```

**Step 5: Run typecheck**

Run: `cd /home/croo12/croo12 && npx tsc -b`
Expected: Errors in GamePage.tsx and IsometricCanvas.tsx (expected — will fix in next tasks)

**Step 6: Commit**

```bash
git add src/entities/tile/ && git rm -r src/entities/water/
git commit -m "refactor: update TS tile model for u8 pack, remove water entity"
```

---

### Task 10: GamePage 업데이트

**Files:**
- Modify: `src/pages/game/ui/GamePage.tsx`

**Step 1: Update GamePage**

```typescript
import { useQuery } from "@tanstack/react-query";
import type React from "react";
import { useEffect, useMemo } from "react";
import initGameCore, {
    create_world,
    tick_water,
    world_depth,
    world_height,
    world_tiles_len,
    world_tiles_ptr,
    world_width,
} from "../../../../core/build/game_core";
import { WorldData } from "@/entities/tile";
import { IsometricCanvas } from "@/features/terrain-renderer";
import { createWasmLoader } from "@/shared/wasm";
import { colors, effects, layout, spacing } from "@/shared/theme";
import { Body, Title } from "@/shared/ui";

const CANVAS_WIDTH = 800;
const CANVAS_HEIGHT = 600;
const WORLD_SIZE = 64;
const WORLD_HEIGHT = 128;
const SEED = 1;
const TICK_INTERVAL_MS = 200;

const gameCoreQueryOptions = createWasmLoader("game-core", initGameCore);

export const GamePage: React.FC = () => {
    const { data: wasmOutput, isSuccess } = useQuery(gameCoreQueryOptions);

    const world = useMemo(() => {
        if (!isSuccess || !wasmOutput) return null;

        create_world(WORLD_SIZE, WORLD_SIZE, WORLD_HEIGHT, SEED);

        const ptr = world_tiles_ptr();
        const len = world_tiles_len();
        const w = world_width();
        const d = world_depth();
        const h = world_height();

        const tiles = new Uint8Array(wasmOutput.memory.buffer, ptr, len);
        return new WorldData(w, d, h, tiles);
    }, [isSuccess, wasmOutput]);

    useEffect(() => {
        if (!wasmOutput || !world) return;
        const interval = setInterval(() => {
            tick_water();
            const ptr = world_tiles_ptr();
            const len = world_tiles_len();
            const tiles = new Uint8Array(wasmOutput.memory.buffer, ptr, len);
            world.updateTiles(tiles);
        }, TICK_INTERVAL_MS);
        return () => clearInterval(interval);
    }, [wasmOutput, world]);

    return (
        <div
            style={{
                display: "flex",
                flexDirection: "column",
                alignItems: "center",
            }}
        >
            <div
                id="game-container"
                style={{
                    width: `${CANVAS_WIDTH}px`,
                    height: `${CANVAS_HEIGHT}px`,
                    backgroundColor: "#1a1a2e",
                    border: `2px solid ${colors.border}`,
                    borderRadius: layout.radius,
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                    boxShadow: effects.shadowElevated,
                    marginBottom: spacing.md,
                }}
            >
                {world ? (
                    <IsometricCanvas
                        world={world}
                        width={CANVAS_WIDTH}
                        height={CANVAS_HEIGHT}
                    />
                ) : (
                    <Body>Loading terrain...</Body>
                )}
            </div>

            <div
                className="controls"
                style={{
                    padding: spacing.md,
                    background: colors.bgElevated,
                    borderRadius: layout.radius,
                    width: `${CANVAS_WIDTH}px`,
                    boxSizing: "border-box",
                    textAlign: "center",
                }}
            >
                <Title>Isometric Terrain Sandbox</Title>
                <Body>WASD / Arrow keys to pan, mouse wheel to zoom.</Body>
            </div>
        </div>
    );
};
```

Key changes: Removed all WaterData/water imports. `Uint16Array` → `Uint8Array`. `WORLD_HEIGHT` 32→128. Removed `waterData` prop from `IsometricCanvas`.

**Step 2: Commit**

```bash
git add src/pages/game/ui/GamePage.tsx
git commit -m "refactor: simplify GamePage, remove water layer"
```

---

### Task 11: IsometricCanvas + rendering 업데이트

**Files:**
- Modify: `src/features/terrain-renderer/ui/IsometricCanvas.tsx`
- Modify: `src/features/terrain-renderer/lib/isometric.ts`
- Modify: `src/features/terrain-renderer/lib/tile-palette.ts`
- Modify: `src/features/terrain-renderer/index.ts`

**Step 1: Update isometric.ts**

```typescript
export const TILE_WIDTH = 32;
export const TILE_HEIGHT = 16;
export const TILE_DEPTH = 2; // was 8, now 2 for 128 height

export const toScreenX = (x: number, y: number): number =>
    (x - y) * (TILE_WIDTH / 2);

export const toScreenY = (x: number, y: number, z: number): number =>
    (x + y) * (TILE_HEIGHT / 2) - z * TILE_DEPTH;
```

**Step 2: Update tile-palette.ts** — no structural changes needed (Water already has colors)

**Step 3: Update IsometricCanvas.tsx**

```typescript
import type React from "react";
import { useCallback, useEffect, useRef } from "react";
import { TileType, getTileOpacity, type WorldData } from "@/entities/tile";
import {
    TILE_DEPTH,
    TILE_HEIGHT,
    TILE_WIDTH,
    toScreenX,
    toScreenY,
} from "../lib/isometric";
import { getTileFaces } from "../lib/tile-palette";
import { useCamera } from "./use-camera";

interface IsometricCanvasProps {
    world: WorldData;
    width: number;
    height: number;
}

const drawTile = (
    ctx: CanvasRenderingContext2D,
    sx: number,
    sy: number,
    tileType: number,
    alpha: number,
): void => {
    const faces = getTileFaces(tileType);
    if (!faces) return;

    const hw = TILE_WIDTH / 2;
    const hh = TILE_HEIGHT / 2;

    ctx.globalAlpha = alpha;

    // Top face
    ctx.fillStyle = faces.top;
    ctx.beginPath();
    ctx.moveTo(sx, sy - hh);
    ctx.lineTo(sx + hw, sy);
    ctx.lineTo(sx, sy + hh);
    ctx.lineTo(sx - hw, sy);
    ctx.closePath();
    ctx.fill();

    // Left face
    ctx.fillStyle = faces.left;
    ctx.beginPath();
    ctx.moveTo(sx - hw, sy);
    ctx.lineTo(sx, sy + hh);
    ctx.lineTo(sx, sy + hh + TILE_DEPTH);
    ctx.lineTo(sx - hw, sy + TILE_DEPTH);
    ctx.closePath();
    ctx.fill();

    // Right face
    ctx.fillStyle = faces.right;
    ctx.beginPath();
    ctx.moveTo(sx + hw, sy);
    ctx.lineTo(sx, sy + hh);
    ctx.lineTo(sx, sy + hh + TILE_DEPTH);
    ctx.lineTo(sx + hw, sy + TILE_DEPTH);
    ctx.closePath();
    ctx.fill();

    ctx.globalAlpha = 1.0;
};

export const IsometricCanvas: React.FC<IsometricCanvasProps> = ({
    world,
    width,
    height,
}) => {
    const canvasRef = useRef<HTMLCanvasElement>(null);

    const centerX = world.width / 2;
    const centerY = world.depth / 2;
    const centerZ = world.height * 0.3;
    const initialCamX = toScreenX(centerX, centerY);
    const initialCamY = toScreenY(centerX, centerY, centerZ);

    const { camera, onWheel } = useCamera(initialCamX, initialCamY);
    const cameraRef = useRef(camera);
    cameraRef.current = camera;

    const render = useCallback(() => {
        const canvas = canvasRef.current;
        if (!canvas) return;
        const ctx = canvas.getContext("2d");
        if (!ctx) return;

        const cam = cameraRef.current;
        ctx.clearRect(0, 0, canvas.width, canvas.height);

        ctx.save();
        ctx.translate(canvas.width / 2, canvas.height / 4);
        ctx.scale(cam.zoom, cam.zoom);
        ctx.translate(-cam.x, -cam.y);

        const w = world.width;
        const d = world.depth;

        for (let y = 0; y < d; y++) {
            for (let x = 0; x < w; x++) {
                // Visibility scan: top-down per column
                let accumulated = 0.0;
                for (let z = world.getTopZ(x, y); z >= 0 && accumulated < 1.0; z--) {
                    const tileType = world.getTile(x, y, z);
                    if (tileType === TileType.Air) continue;

                    const opacity = getTileOpacity(tileType);
                    const alpha = Math.max(0, 1.0 - accumulated);
                    const sx = toScreenX(x, y);
                    const sy = toScreenY(x, y, z);

                    drawTile(ctx, sx, sy, tileType, alpha);
                    accumulated += opacity;
                }
            }
        }

        ctx.restore();
    }, [world]);

    useEffect(() => {
        let rafId: number;
        const loop = (): void => {
            render();
            rafId = requestAnimationFrame(loop);
        };
        rafId = requestAnimationFrame(loop);
        return () => cancelAnimationFrame(rafId);
    }, [render]);

    useEffect(() => {
        const canvas = canvasRef.current;
        if (!canvas) return;

        const handleWheel = (e: WheelEvent): void => {
            e.preventDefault();
            onWheel(e);
        };

        canvas.addEventListener("wheel", handleWheel, { passive: false });
        return () => canvas.removeEventListener("wheel", handleWheel);
    }, [onWheel]);

    return (
        <canvas
            ref={canvasRef}
            width={width}
            height={height}
            style={{ display: "block", background: "#1a1a2e" }}
        />
    );
};
```

Key changes: Removed `waterData` prop, removed `isOccluded`, replaced with visibility score column scan, `drawTile` uses alpha instead of level, `TILE_DEPTH` = 2.

**Step 4: Run typecheck + lint**

Run: `cd /home/croo12/croo12 && npx tsc -b && yarn lint`
Expected: PASS

**Step 5: Commit**

```bash
git add src/features/terrain-renderer/
git commit -m "refactor: update renderer for cell-based tiles with visibility scoring"
```

---

### Task 12: Integration test — WASM build + dev server

**Step 1: Build WASM**

Run: `cd /home/croo12/croo12/core && wasm-pack build --target web --out-dir build`
Expected: Build succeeds

**Step 2: Run full Rust test suite**

Run: `cd /home/croo12/croo12/core && cargo test`
Expected: All tests PASS

**Step 3: Run TS typecheck + lint**

Run: `cd /home/croo12/croo12 && npx tsc -b && yarn lint`
Expected: PASS

**Step 4: Start dev server and verify visually**

Run: `cd /home/croo12/croo12 && yarn dev`
Expected: Game page loads, terrain renders with 128 height, water flows from sources

**Step 5: Commit (if any fixes needed)**

```bash
git add -A
git commit -m "fix: integration fixes for cell-based architecture"
```
