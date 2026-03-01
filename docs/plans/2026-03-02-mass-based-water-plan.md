# Mass-Based Water System Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace discrete block water model with mass-based cellular automata for continuous river flow.

**Architecture:** Separate water layer (`Vec<u8>` water_mass) from terrain (`Vec<Tile>`). Delta buffers for bias-free simultaneous flow. 4-phase tick: gravity → spread → pressure → apply.

**Tech Stack:** Rust/WASM, TypeScript/React frontend, cargo test, wasm-pack

**Design doc:** `docs/plans/2026-03-02-mass-based-water-design.md`

---

### Task 1: Add water layer fields to World

**Files:**
- Modify: `core/src/world/mod.rs`

**Step 1: Add water fields to World struct**

Add these fields after `tiles_cache`:

```rust
pub struct World {
    width: usize,
    depth: usize,
    height: usize,
    tiles: Vec<Tile>,
    tiles_cache: Vec<u8>,
    // New water layer fields
    water_mass: Vec<u8>,
    water_sediment: Vec<u8>,
    mass_delta: Vec<i16>,
    sediment_delta: Vec<i16>,
    water_outflow: Vec<u16>,
    sources: Vec<(usize, usize, usize)>,
}
```

**Step 2: Update World::new() to initialize new fields**

```rust
pub fn new(width: usize, depth: usize, height: usize) -> Self {
    let size = width * depth * height;
    Self {
        width,
        depth,
        height,
        tiles: vec![Tile::Air; size],
        tiles_cache: vec![0u8; size],
        water_mass: vec![0u8; size],
        water_sediment: vec![0u8; size],
        mass_delta: vec![0i16; size],
        sediment_delta: vec![0i16; size],
        water_outflow: vec![0u16; size],
        sources: Vec::new(),
    }
}
```

**Step 3: Add water accessor methods**

```rust
pub fn water_mass(&self, x: usize, y: usize, z: usize) -> u8 {
    self.water_mass[self.index(x, y, z)]
}

pub fn set_water_mass(&mut self, x: usize, y: usize, z: usize, mass: u8) {
    let idx = self.index(x, y, z);
    self.water_mass[idx] = mass;
}

pub fn water_mass_ptr(&self) -> *const u8 {
    self.water_mass.as_ptr()
}

pub fn water_mass_len(&self) -> usize {
    self.water_mass.len()
}

pub fn water_sediment(&self, x: usize, y: usize, z: usize) -> u8 {
    self.water_sediment[self.index(x, y, z)]
}

pub fn set_water_sediment(&mut self, x: usize, y: usize, z: usize, sed: u8) {
    let idx = self.index(x, y, z);
    self.water_sediment[idx] = sed;
}
```

**Step 4: Add delta/outflow helpers**

```rust
pub fn mass_delta_ref(&self) -> &[i16] {
    &self.mass_delta
}

pub fn record_flow(&mut self, from: usize, to: usize, amount: u16, sed_amount: i16) {
    self.mass_delta[from] -= amount as i16;
    self.mass_delta[to] += amount as i16;
    self.water_outflow[from] += amount;
    if sed_amount != 0 {
        self.sediment_delta[from] -= sed_amount;
        self.sediment_delta[to] += sed_amount;
    }
}

pub fn apply_water_deltas(&mut self) {
    for i in 0..self.water_mass.len() {
        if self.mass_delta[i] != 0 {
            let new_mass = (self.water_mass[i] as i16 + self.mass_delta[i]).clamp(0, 255);
            self.water_mass[i] = new_mass as u8;
            self.mass_delta[i] = 0;
        }
        if self.sediment_delta[i] != 0 {
            let new_sed = (self.water_sediment[i] as i16 + self.sediment_delta[i]).clamp(0, 255);
            self.water_sediment[i] = new_sed as u8;
            self.sediment_delta[i] = 0;
        }
        self.water_outflow[i] = 0;
    }
}

pub fn water_outflow(&self, idx: usize) -> u16 {
    self.water_outflow[idx]
}
```

**Step 5: Write tests**

```rust
#[test]
fn water_mass_set_and_get() {
    let mut w = World::new(4, 4, 4);
    w.set_water_mass(1, 1, 1, 100);
    assert_eq!(w.water_mass(1, 1, 1), 100);
    assert_eq!(w.water_mass(0, 0, 0), 0);
}

#[test]
fn record_flow_updates_deltas() {
    let mut w = World::new(4, 4, 4);
    w.set_water_mass(0, 0, 0, 200);
    let from = w.index(0, 0, 0);
    let to = w.index(1, 0, 0);
    w.record_flow(from, to, 50, 0);
    assert_eq!(w.mass_delta_ref()[from], -50);
    assert_eq!(w.mass_delta_ref()[to], 50);
}

#[test]
fn apply_deltas_clamps_and_resets() {
    let mut w = World::new(4, 4, 4);
    w.set_water_mass(0, 0, 0, 200);
    let idx = w.index(0, 0, 0);
    w.mass_delta[idx] = 100; // would exceed 255
    w.apply_water_deltas();
    assert_eq!(w.water_mass(0, 0, 0), 255); // clamped
    assert_eq!(w.mass_delta_ref()[idx], 0); // reset
}
```

**Step 6: Run tests**

Run: `cargo test --manifest-path core/Cargo.toml`
Expected: All existing tests pass + new tests pass.

**Step 7: Commit**

```
feat(water): add mass-based water layer fields to World
```

---

### Task 2: Write flow algorithm (gravity + spread + pressure)

**Files:**
- Create: `core/src/water/flow.rs`
- Modify: `core/src/water/mod.rs` (add `pub mod flow;`)

**Step 1: Create flow.rs with Phase 1 (gravity)**

```rust
use crate::world::World;

/// Simple hash for deterministic pseudo-random sediment rounding
fn simple_hash(idx: usize, seed: u64) -> u64 {
    let mut h = seed;
    h = h.wrapping_mul(6364136223846793005).wrapping_add(idx as u64);
    h ^ (h >> 33)
}

/// Transfer sediment proportionally when water flows.
/// Uses probabilistic rounding for small amounts.
fn calc_sediment_transfer(sediment: u8, transfer: u16, mass: u8, idx: usize, seed: u64) -> i16 {
    if sediment == 0 || mass == 0 {
        return 0;
    }
    let exact = (sediment as u32 * transfer as u32) as f64 / mass as f64;
    let base = exact.floor() as i16;
    let frac = exact - exact.floor();
    let roll = (simple_hash(idx, seed) % 1000) as f64 / 1000.0;
    if roll < frac { base + 1 } else { base }
}

static FLOW_TICK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn pass_flow(world: &mut World) {
    let seed = FLOW_TICK.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    let w = world.width();
    let d = world.depth();
    let h = world.height();

    // Phase 1: Gravity (top-down)
    for z in (1..h).rev() {
        for y in 0..d {
            for x in 0..w {
                let idx = world.index(x, y, z);
                let mass = world.water_mass[idx];
                if mass == 0 { continue; }

                let below_idx = world.index(x, y, z - 1);
                if world.get(x, y, z - 1).is_solid() { continue; }

                let below_mass = world.water_mass[below_idx];
                if below_mass >= 255 { continue; }

                let capacity = 255u16.saturating_sub(below_mass as u16);
                let transfer = (mass as u16).min(capacity);
                if transfer == 0 { continue; }

                let sed = world.water_sediment[idx];
                let sed_transfer = calc_sediment_transfer(sed, transfer, mass, idx, seed);
                world.record_flow(idx, below_idx, transfer, sed_transfer);
            }
        }
    }

    // Phase 2: Horizontal spread
    for z in 0..h {
        for y in 0..d {
            for x in 0..w {
                let idx = world.index(x, y, z);
                let mass = world.water_mass[idx];
                if mass == 0 { continue; }

                // Calculate remaining after gravity
                let remaining = (mass as i16 + world.mass_delta[idx]).max(0) as u8;
                if remaining == 0 { continue; }

                // Skip if can still fall (gravity priority)
                if z > 0 {
                    let below_idx = world.index(x, y, z - 1);
                    let below_expected = (world.water_mass[below_idx] as i16
                        + world.mass_delta[below_idx]).min(255).max(0) as u8;
                    if !world.get(x, y, z - 1).is_solid() && below_expected < 255 {
                        continue;
                    }
                }

                // Collect valid neighbors with lower mass
                let neighbors: [(usize, usize); 4] = [
                    (x.wrapping_sub(1), y),
                    (x + 1, y),
                    (x, y.wrapping_sub(1)),
                    (x, y + 1),
                ];

                let mut valid: Vec<usize> = Vec::new();
                for &(nx, ny) in &neighbors {
                    if nx >= w || ny >= d { continue; }
                    if world.get(nx, ny, z).is_solid() { continue; }
                    let n_idx = world.index(nx, ny, z);
                    let n_mass = (world.water_mass[n_idx] as i16
                        + world.mass_delta[n_idx]).max(0) as u8;
                    if n_mass < remaining {
                        valid.push(n_idx);
                    }
                }

                if valid.is_empty() { continue; }

                let divisor = (valid.len() + 1) as u16;
                for &n_idx in &valid {
                    let n_mass = (world.water_mass[n_idx] as i16
                        + world.mass_delta[n_idx]).max(0) as u8;
                    let diff = remaining.saturating_sub(n_mass) as u16;
                    let transfer = diff / divisor;
                    if transfer == 0 { continue; }

                    let sed = world.water_sediment[idx];
                    let sed_transfer = calc_sediment_transfer(sed, transfer, remaining, idx, seed);
                    world.record_flow(idx, n_idx, transfer, sed_transfer);
                }
            }
        }
    }

    // Phase 3: Pressure (bottom-up) - push excess up
    for z in 0..h.saturating_sub(1) {
        for y in 0..d {
            for x in 0..w {
                let idx = world.index(x, y, z);
                let expected = world.water_mass[idx] as i16 + world.mass_delta[idx];
                if expected <= 255 { continue; }

                let excess = (expected - 255) as u16;
                let above_idx = world.index(x, y, z + 1);
                // Only push up if above is not solid
                if world.get(x, y, z + 1).is_solid() { continue; }

                world.mass_delta[idx] -= excess as i16;
                world.mass_delta[above_idx] += excess as i16;
            }
        }
    }

    // Phase 4: Apply deltas
    world.apply_water_deltas();
}
```

**Step 2: Add `pub mod flow;` to `core/src/water/mod.rs`**

Add the line at the top of the file alongside existing modules.

**Step 3: Write tests for flow.rs**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tile::Tile;
    use crate::world::World;

    #[test]
    fn gravity_water_falls_into_air() {
        let mut w = World::new(4, 4, 4);
        w.set(0, 0, 0, Tile::Stone);
        w.set_water_mass(0, 0, 2, 100);
        pass_flow(&mut w);
        assert_eq!(w.water_mass(0, 0, 1), 100);
        assert_eq!(w.water_mass(0, 0, 2), 0);
    }

    #[test]
    fn gravity_fills_below_to_capacity() {
        let mut w = World::new(4, 4, 4);
        w.set(0, 0, 0, Tile::Stone);
        w.set_water_mass(0, 0, 1, 200);
        w.set_water_mass(0, 0, 2, 100);
        pass_flow(&mut w);
        // below was 200, can take 55 more
        assert_eq!(w.water_mass(0, 0, 1), 255);
        assert_eq!(w.water_mass(0, 0, 2), 45);
    }

    #[test]
    fn gravity_stops_on_solid() {
        let mut w = World::new(4, 4, 4);
        w.set(0, 0, 0, Tile::Stone);
        w.set_water_mass(0, 0, 1, 100);
        pass_flow(&mut w);
        assert_eq!(w.water_mass(0, 0, 1), 100); // stays
    }

    #[test]
    fn horizontal_spread_equalizes() {
        let mut w = World::new(4, 4, 4);
        // Solid floor
        for x in 0..4 { for y in 0..4 { w.set(x, y, 0, Tile::Stone); } }
        w.set_water_mass(2, 2, 1, 200);
        pass_flow(&mut w);
        // Water should have spread to neighbors
        let center = w.water_mass(2, 2, 1);
        let total: u16 = (0..4).flat_map(|x| (0..4).map(move |y| (x, y)))
            .map(|(x, y)| w.water_mass(x, y, 1) as u16)
            .sum();
        assert_eq!(total, 200); // mass conserved
        assert!(center < 200); // some spread out
    }

    #[test]
    fn pressure_pushes_up() {
        let mut w = World::new(4, 4, 4);
        // Walled box: solid everywhere except (1,1,1) and (1,1,2)
        for x in 0..4 { for y in 0..4 { for z in 0..4 {
            w.set(x, y, z, Tile::Stone);
        }}}
        w.set(1, 1, 1, Tile::Air);
        w.set(1, 1, 2, Tile::Air);
        // Overfill (1,1,1) via deltas
        w.set_water_mass(1, 1, 1, 200);
        w.mass_delta[w.index(1, 1, 1)] = 100; // would be 300, excess 45
        // Manually run Phase 3 + apply
        pass_flow(&mut w);
        // Some mass should appear at z=2
        assert!(w.water_mass(1, 1, 2) > 0);
        assert!(w.water_mass(1, 1, 1) <= 255);
    }

    #[test]
    fn source_fills_continuously() {
        let mut w = World::new(4, 4, 8);
        for x in 0..4 { for y in 0..4 { w.set(x, y, 0, Tile::Stone); } }
        w.add_source(2, 2, 1);
        // Run many ticks - source should produce water
        for _ in 0..50 {
            pass_flow(&mut w);
            // Source replenishment
            for &(sx, sy, sz) in w.sources().to_vec().iter() {
                let idx = w.index(sx, sy, sz);
                w.water_mass[idx] = w.water_mass[idx].saturating_add(50);
            }
        }
        // Water should have spread away from source
        let total: u32 = (0..4).flat_map(|x| (0..4).flat_map(move |y|
            (0..8).map(move |z| (x, y, z))))
            .map(|(x, y, z)| w.water_mass(x, y, z) as u32)
            .sum();
        assert!(total > 255, "Should have significant water: {}", total);
    }

    #[test]
    fn sediment_moves_with_water() {
        let mut w = World::new(4, 4, 4);
        w.set(0, 0, 0, Tile::Stone);
        w.set_water_mass(0, 0, 2, 200);
        w.set_water_sediment(0, 0, 2, 4);
        pass_flow(&mut w);
        // Water fell to z=1, sediment should follow
        assert!(w.water_sediment(0, 0, 1) > 0);
    }
}
```

**Step 4: Run tests**

Run: `cargo test --manifest-path core/Cargo.toml flow`
Expected: All pass.

**Step 5: Commit**

```
feat(water): add mass-based flow algorithm with gravity, spread, pressure
```

---

### Task 3: Write mass-based erosion, deposition, evaporation

**Files:**
- Create: `core/src/water/mass_erosion.rs`
- Create: `core/src/water/mass_evaporation.rs`

New files alongside old ones (old ones deleted in Task 5).

**Step 1: Create mass_erosion.rs**

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use crate::world::World;

fn simple_hash(x: usize, y: usize, z: usize, seed: u64) -> u64 {
    let mut h = seed;
    h = h.wrapping_mul(6364136223846793005).wrapping_add(x as u64);
    h = h.wrapping_mul(6364136223846793005).wrapping_add(y as u64);
    h = h.wrapping_mul(6364136223846793005).wrapping_add(z as u64);
    h ^ (h >> 33)
}

fn count_water_above(world: &World, x: usize, y: usize, z: usize) -> usize {
    let h = world.height();
    let mut count = 0;
    let mut cz = z + 1;
    while cz < h {
        if world.water_mass(x, y, cz) > 0 { count += 1; cz += 1; }
        else { break; }
    }
    count
}

static EROSION_TICK: AtomicU64 = AtomicU64::new(0);

pub fn pass_erosion(world: &mut World) {
    let seed = EROSION_TICK.fetch_add(1, Ordering::Relaxed);
    let w = world.width();
    let d = world.depth();
    let h = world.height();

    // Erosion: water with flow or pressure erodes erodible tile below
    for z in (1..h).rev() {
        for y in 0..d {
            for x in 0..w {
                let idx = world.index(x, y, z);
                let mass = world.water_mass[idx];
                if mass == 0 { continue; }

                if !world.get(x, y, z - 1).is_erodible() { continue; }

                let flow = world.water_outflow(idx);
                let pressure = count_water_above(world, x, y, z) as u64;
                let chance = (pressure * 5 + (flow as u64) / 10).min(80);
                if chance == 0 { continue; }

                let roll = simple_hash(x, y, z, seed) % 100;
                if roll < chance {
                    use crate::tile::Tile;
                    world.set(x, y, z - 1, Tile::Air);
                    let sed = world.water_sediment[idx];
                    world.water_sediment[idx] = sed.saturating_add(1).min(7);
                }
            }
        }
    }

    // Deposition: slow water with sediment on solid ground deposits Sand
    for z in (1..h).rev() {
        for y in 0..d {
            for x in 0..w {
                let idx = world.index(x, y, z);
                if world.water_sediment[idx] == 0 { continue; }
                if world.water_mass[idx] == 0 { continue; }

                let flow = world.water_outflow(idx);
                if flow > 20 { continue; }

                if !world.get(x, y, z - 1).is_solid() { continue; }

                use crate::tile::Tile;
                // Current tile must be Air (not solid) for Sand placement
                if !world.get(x, y, z).is_air() && !world.get(x, y, z).is_solid() {
                    continue;
                }
                world.set(x, y, z, Tile::Sand);
                world.water_sediment[idx] -= 1;
                // Water at this cell gets displaced up by pressure next tick
            }
        }
    }
}
```

**Step 2: Create mass_evaporation.rs**

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use crate::tile::Tile;
use crate::world::World;

fn simple_hash(x: usize, y: usize, z: usize, seed: u64) -> u64 {
    let mut h = seed;
    h = h.wrapping_mul(6364136223846793005).wrapping_add(x as u64);
    h = h.wrapping_mul(6364136223846793005).wrapping_add(y as u64);
    h = h.wrapping_mul(6364136223846793005).wrapping_add(z as u64);
    h ^ (h >> 33)
}

static EVAP_TICK: AtomicU64 = AtomicU64::new(0);

pub fn pass_evaporation(world: &mut World, sources: &[(usize, usize, usize)]) {
    let seed = EVAP_TICK.fetch_add(1, Ordering::Relaxed);
    let w = world.width();
    let d = world.depth();
    let h = world.height();

    for z in 0..h {
        for y in 0..d {
            for x in 0..w {
                let idx = world.index(x, y, z);
                let mass = world.water_mass[idx];
                if mass == 0 { continue; }

                // Sources don't evaporate
                if sources.iter().any(|&(sx, sy, sz)| sx == x && sy == y && sz == z) {
                    continue;
                }

                // No evaporation if water above
                if z + 1 < h && world.water_mass(x, y, z + 1) > 0 {
                    continue;
                }

                let roll = simple_hash(x, y, z, seed) % 100;
                if roll < 5 {
                    let evap = mass.min(10);
                    world.water_mass[idx] = mass - evap;
                    if world.water_mass[idx] == 0 && world.water_sediment[idx] > 0 {
                        world.set(x, y, z, Tile::Sand);
                        world.water_sediment[idx] = 0;
                    }
                }
            }
        }
    }
}
```

**Step 3: Add `pub mod mass_erosion; pub mod mass_evaporation;` to water/mod.rs**

**Step 4: Write tests for both modules (in each file)**

Include tests similar to existing erosion/evaporation tests but using water_mass.

**Step 5: Run tests**

Run: `cargo test --manifest-path core/Cargo.toml mass_`
Expected: All pass.

**Step 6: Commit**

```
feat(water): add mass-based erosion, deposition, and evaporation
```

---

### Task 4: Wire up new tick function

**Files:**
- Modify: `core/src/water/mod.rs`

**Step 1: Add new tick function using mass-based modules**

Add `tick_mass` alongside existing `tick`:

```rust
pub fn tick_mass(world: &mut World) {
    // Solid gravity (sand, dirt falling) - reuse existing
    crate::world::gravity::pass_gravity(world);

    // Mass-based water flow
    flow::pass_flow(world);

    // Source replenishment
    let sources: Vec<_> = world.sources().to_vec();
    for &(sx, sy, sz) in &sources {
        let idx = world.index(sx, sy, sz);
        world.water_mass[idx] = world.water_mass[idx].saturating_add(50);
    }

    // Erosion & deposition (uses outflow data from flow pass)
    mass_erosion::pass_erosion(world);

    // Evaporation
    mass_evaporation::pass_evaporation(world, &sources);

    world.sync_tiles_cache();
}
```

**Step 2: Write integration test**

```rust
#[test]
fn tick_mass_source_produces_flowing_water() {
    let mut world = World::new(8, 8, 8);
    for x in 0..8 { for y in 0..8 { world.set(x, y, 0, Tile::Stone); } }
    world.add_source(4, 4, 1);
    world.set_water_mass(4, 4, 1, 255);

    for _ in 0..100 {
        tick_mass(&mut world);
    }

    // Water should have spread significantly
    let total: u32 = (0..8).flat_map(|x| (0..8).flat_map(move |y|
        (0..8).map(move |z| (x, y, z))))
        .map(|(x, y, z)| world.water_mass(x, y, z) as u32)
        .sum();
    assert!(total > 500, "Total water should be significant: {}", total);

    // Water should exist at positions away from source
    let distant_water = world.water_mass(0, 4, 1) + world.water_mass(4, 0, 1);
    assert!(distant_water > 0, "Water should reach distant cells");
}
```

**Step 3: Run tests**

Run: `cargo test --manifest-path core/Cargo.toml tick_mass`
Expected: Pass.

**Step 4: Commit**

```
feat(water): wire up mass-based tick function
```

---

### Task 5: Remove Water from Tile, migrate all references

This is the breaking change. Remove `Tile::Water` variant and update everything that referenced it.

**Files:**
- Modify: `core/src/tile.rs`
- Modify: `core/src/world/gravity.rs`
- Modify: `core/src/terrain.rs`
- Modify: `core/src/lib.rs`
- Modify: `core/src/render/ascii.rs`
- Modify: `core/src/water/mod.rs`
- Delete: `core/src/water/spread.rs`
- Delete: `core/src/water/erosion.rs`
- Delete: `core/src/water/evaporation.rs`
- Delete: `core/src/water/source.rs`

**Step 1: Remove Water from Tile enum in tile.rs**

Remove `Water { is_source, sediment, velocity, direction }` variant.
Remove `FlowDir` enum entirely.
Update: `pack()` — remove Water arm. `unpack()` — remove type_id 5.
Remove: `is_water()`, `water_default()`, `water_source()`.
Update: `falls()` — remove Water match. `opacity()` — remove Water.
Update all tests.

**Step 2: Update gravity.rs**

Remove `Tile::Water` match arm. Only handle solid tiles (Grass, Dirt, Sand).
Remove `use crate::tile::FlowDir;`.
Remove water-specific tests (water_falls_to_ground, water_stops_on_solid, water_stops_on_water, source_stays_and_water_falls).

**Step 3: Update terrain.rs**

Remove `Tile::water_source()` placement. Keep source position tracking:
```rust
// Instead of placing water tiles, just record source positions
world.add_source(sx, sy, sz);
// Set initial water mass
world.set_water_mass(sx, sy, sz, 255);
```

**Step 4: Update water/mod.rs**

Remove old modules: `erosion`, `evaporation`, `source`, `spread`.
Remove old `tick()` function.
Rename `tick_mass` → `tick`.

**Step 5: Delete old water module files**

Delete: `spread.rs`, `erosion.rs`, `evaporation.rs`, `source.rs`.

**Step 6: Update lib.rs**

- `tick_water` → call `water::tick()`
- `place_water` → `w.set_water_mass(x, y, z, 255);`
- `place_water_source` → `w.set_water_mass(x, y, z, 255); w.add_source(x, y, z);`
- `remove_water` → `w.set_water_mass(x, y, z, 0);`
- Add: `world_water_ptr`, `world_water_len` WASM exports

**Step 7: Update render/ascii.rs**

Replace `Tile::Water` arm with water_mass check:
```rust
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
```

Update `render_top_down` and `render_side` to pass `world.water_mass(x, y, z)`.

**Step 8: Run all tests**

Run: `cargo test --manifest-path core/Cargo.toml`
Expected: All pass (old water tests removed, new mass-based tests remain).

**Step 9: Commit**

```
refactor(water): remove Tile::Water, migrate to mass-based model
```

---

### Task 6: Build WASM and verify

**Files:**
- None new (already updated in Task 5)

**Step 1: Build WASM**

Run: `cd core && wasm-pack build --target web --out-dir build`
Expected: Build succeeds.

**Step 2: Run CLI simulation**

Update `cli_simulation_debug` test for mass-based rendering, run with `--nocapture`, verify:
- Sources produce water
- Water flows and spreads
- Water mass numbers visible in ASCII output

**Step 3: Commit**

```
chore: rebuild WASM with mass-based water system
```

---

### Task 7: Update frontend for water layer

**Files:**
- Modify: `src/entities/tile/model/tile-type.ts`
- Modify: `src/entities/tile/model/world-data.ts`
- Modify: `src/pages/game/ui/GamePage.tsx`
- Modify: `src/features/terrain-renderer/ui/IsometricCanvas.tsx`
- Modify: `src/features/terrain-renderer/lib/tile-palette.ts`

**Step 1: Update tile-type.ts**

Remove `Water: 5` from TileType. Remove Water from OPACITY record.

**Step 2: Update world-data.ts**

Add water array:
```typescript
export class WorldData {
    private tiles: Uint8Array;
    private water: Uint8Array;

    constructor(w, d, h, tiles: Uint8Array, water: Uint8Array) {
        // ...
        this.water = new Uint8Array(water);
    }

    getWaterMass(x: number, y: number, z: number): number {
        return this.water[this.index(x, y, z)];
    }

    updateTiles(tiles: Uint8Array, water: Uint8Array): void {
        this.tiles = new Uint8Array(tiles);
        this.water = new Uint8Array(water);
    }

    // Update getTopZ to consider water
    getTopZ(x: number, y: number): number {
        for (let z = this.height - 1; z >= 0; z--) {
            if (this.getTile(x, y, z) !== TileType.Air || this.getWaterMass(x, y, z) > 0) {
                return z;
            }
        }
        return 0;
    }
}
```

Remove `getFlowDir()`, `isSource()` (no longer in packed data).

**Step 3: Update GamePage.tsx**

Import `world_water_ptr`, `world_water_len` from WASM.
Pass water array to WorldData constructor and updateTiles.

```typescript
import {
    // ... existing imports
    world_water_ptr,
    world_water_len,
} from "../../../../core/build/game_core";

// In useMemo:
const waterPtr = world_water_ptr();
const waterLen = world_water_len();
const water = new Uint8Array(wasmOutput.memory.buffer, waterPtr, waterLen);
return new WorldData(w, d, h, tiles, water);

// In useEffect interval:
const water = new Uint8Array(wasmOutput.memory.buffer, world_water_ptr(), world_water_len());
world.updateTiles(tiles, water);
```

**Step 4: Update IsometricCanvas.tsx**

After drawing terrain tile, check for water and draw water overlay:

```typescript
// After existing drawTile for terrain:
const waterMass = world.getWaterMass(x, y, z);
if (waterMass > 0) {
    const waterAlpha = 0.3 + (waterMass / 255) * 0.4; // 0.3~0.7
    drawTile(ctx, sx, sy, WATER_TILE_ID, waterAlpha);
}
```

Add water color constant (not in TileType anymore, standalone):
```typescript
const WATER_FACES = {
    top: "rgba(64, 164, 223, 0.7)",
    left: "rgba(48, 130, 190, 0.7)",
    right: "rgba(38, 110, 165, 0.7)",
};
```

**Step 5: Update tile-palette.ts**

Remove Water entry from palette. Add separate water drawing function or constant.

**Step 6: Update getOpacityCutZ**

Consider water mass in opacity calculation:
```typescript
const waterMass = world.getWaterMass(cx, cy, z);
if (waterMass > 0) accum += (waterMass / 255) * 0.3;
```

**Step 7: Run dev server and verify visually**

Run: `yarn dev`
Expected: Water visible, flowing from sources, rendering correctly.

**Step 8: Commit**

```
feat(frontend): update rendering pipeline for mass-based water layer
```

---

## Verification Checklist

After all tasks complete:

1. `cargo test --manifest-path core/Cargo.toml` — all pass
2. `wasm-pack build` — succeeds
3. `npx tsc -b` — no type errors
4. `yarn lint` — no lint errors
5. CLI simulation shows water flowing from sources
6. Browser rendering shows water with variable opacity/height
7. Water forms rivers on slopes (not just puddles)
