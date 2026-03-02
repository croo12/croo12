# Groundwater System Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 토양 수분 흡수, 지하수 흐름, 샘물 배출, 수분 기반 침식 배율을 구현하여 완전한 수문학적 순환을 완성한다.

**Architecture:** 기존 mass-based cellular automata 패턴을 확장. `soil_moisture: Vec<u8>` 레이어 추가, delta 기반 지하수 흐름, WASM export로 프론트엔드 렌더링.

**Tech Stack:** Rust (core), wasm-bindgen, TypeScript/React (frontend)

**Design Doc:** `docs/plans/2026-03-02-groundwater-system-design.md`

---

### Task 1: Tile 토양 특성 메서드 추가

**Files:**
- Modify: `core/src/tile.rs`

**Step 1: 기존 테스트 통과 확인**

Run: `cd /home/croo12/croo12/core && cargo test tile::tests`
Expected: PASS

**Step 2: Tile에 토양 특성 메서드 추가**

`core/src/tile.rs`의 `impl Tile` 블록에 추가:

```rust
/// Maximum soil moisture capacity (0-255)
pub fn moisture_capacity(&self) -> u8 {
    match self {
        Self::Sand => 48,
        Self::Grass => 160,
        Self::Dirt => 128,
        Self::Air | Self::Stone => 0,
    }
}

/// Absorption rate per tick (surface water → soil moisture)
pub fn absorb_rate(&self) -> u8 {
    match self {
        Self::Sand => 8,
        Self::Grass => 5,
        Self::Dirt => 2,
        Self::Air | Self::Stone => 0,
    }
}

/// Permeability for underground flow speed
pub fn permeability(&self) -> u8 {
    match self {
        Self::Sand => 6,
        Self::Grass => 3,
        Self::Dirt => 1,
        Self::Air | Self::Stone => 0,
    }
}
```

**Step 3: 테스트 추가**

`core/src/tile.rs`의 `mod tests` 블록에 추가:

```rust
#[test]
fn tile_moisture_capacity() {
    assert_eq!(Tile::Sand.moisture_capacity(), 48);
    assert_eq!(Tile::Grass.moisture_capacity(), 160);
    assert_eq!(Tile::Dirt.moisture_capacity(), 128);
    assert_eq!(Tile::Stone.moisture_capacity(), 0);
    assert_eq!(Tile::Air.moisture_capacity(), 0);
}

#[test]
fn tile_absorb_rate() {
    assert_eq!(Tile::Sand.absorb_rate(), 8);
    assert_eq!(Tile::Grass.absorb_rate(), 5);
    assert_eq!(Tile::Dirt.absorb_rate(), 2);
    assert_eq!(Tile::Stone.absorb_rate(), 0);
}

#[test]
fn tile_permeability() {
    assert_eq!(Tile::Sand.permeability(), 6);
    assert_eq!(Tile::Grass.permeability(), 3);
    assert_eq!(Tile::Dirt.permeability(), 1);
    assert_eq!(Tile::Stone.permeability(), 0);
}
```

**Step 4: 테스트 실행**

Run: `cd /home/croo12/croo12/core && cargo test tile::tests`
Expected: ALL PASS

**Step 5: 커밋**

```bash
git add core/src/tile.rs
git commit -m "feat(tile): add moisture_capacity, absorb_rate, permeability methods"
```

---

### Task 2: World 구조체에 soil_moisture 레이어 추가

**Files:**
- Modify: `core/src/world/mod.rs`

**Step 1: World 구조체에 필드 추가**

`core/src/world/mod.rs`의 `World` 구조체에 추가:

```rust
pub(crate) soil_moisture: Vec<u8>,
pub(crate) moisture_delta: Vec<i16>,
```

**Step 2: `World::new`에서 초기화**

`new()` 함수의 `Self { ... }` 블록에 추가:

```rust
soil_moisture: vec![0u8; size],
moisture_delta: vec![0i16; size],
```

**Step 3: 접근자 메서드 추가**

`impl World` 블록의 water 접근자 아래에 추가:

```rust
// --- Soil moisture accessors ---

pub fn soil_moisture(&self, x: usize, y: usize, z: usize) -> u8 {
    self.soil_moisture[self.index(x, y, z)]
}

pub fn set_soil_moisture(&mut self, x: usize, y: usize, z: usize, moisture: u8) {
    let idx = self.index(x, y, z);
    self.soil_moisture[idx] = moisture;
}

pub fn soil_moisture_ptr(&self) -> *const u8 {
    self.soil_moisture.as_ptr()
}

pub fn soil_moisture_len(&self) -> usize {
    self.soil_moisture.len()
}

pub fn apply_moisture_deltas(&mut self) {
    for i in 0..self.soil_moisture.len() {
        if self.moisture_delta[i] != 0 {
            let tile = self.tiles[i];
            let cap = tile.moisture_capacity() as i16;
            let new_val = (self.soil_moisture[i] as i16 + self.moisture_delta[i]).clamp(0, cap);
            self.soil_moisture[i] = new_val as u8;
            self.moisture_delta[i] = 0;
        }
    }
}
```

**Step 4: 테스트 추가**

`core/src/world/mod.rs`의 `mod tests` 블록에 추가:

```rust
#[test]
fn soil_moisture_set_and_get() {
    let mut w = World::new(4, 4, 4);
    w.set(1, 1, 1, Tile::Dirt);
    w.set_soil_moisture(1, 1, 1, 50);
    assert_eq!(w.soil_moisture(1, 1, 1), 50);
    assert_eq!(w.soil_moisture(0, 0, 0), 0);
}

#[test]
fn apply_moisture_deltas_clamps_to_capacity() {
    let mut w = World::new(4, 4, 4);
    w.set(0, 0, 0, Tile::Sand); // capacity = 48
    w.soil_moisture[0] = 40;
    w.moisture_delta[0] = 100; // would exceed 48
    w.apply_moisture_deltas();
    assert_eq!(w.soil_moisture(0, 0, 0), 48); // clamped to capacity
    assert_eq!(w.moisture_delta[0], 0); // reset
}

#[test]
fn apply_moisture_deltas_clamps_to_zero() {
    let mut w = World::new(4, 4, 4);
    w.set(0, 0, 0, Tile::Dirt);
    w.soil_moisture[0] = 10;
    w.moisture_delta[0] = -50; // would go below 0
    w.apply_moisture_deltas();
    assert_eq!(w.soil_moisture(0, 0, 0), 0); // clamped to 0
}
```

**Step 5: 테스트 실행**

Run: `cd /home/croo12/croo12/core && cargo test world::tests`
Expected: ALL PASS

**Step 6: 커밋**

```bash
git add core/src/world/mod.rs
git commit -m "feat(world): add soil_moisture and moisture_delta layers"
```

---

### Task 3: pass_groundwater 구현 (흡수 + 지하 흐름 + 배출)

**Files:**
- Create: `core/src/water/groundwater.rs`
- Modify: `core/src/water/mod.rs`

**Step 1: groundwater.rs 파일 생성 — 흡수 (Absorption)**

`core/src/water/groundwater.rs` 생성:

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use crate::world::World;

static GW_TICK: AtomicU64 = AtomicU64::new(0);

pub fn pass_groundwater(world: &mut World) {
    let _seed = GW_TICK.fetch_add(1, Ordering::Relaxed);
    let w = world.width();
    let d = world.depth();
    let h = world.height();

    // Phase A: Absorption (surface water → soil moisture)
    for z in 1..h {
        for y in 0..d {
            for x in 0..w {
                let idx = world.index(x, y, z);
                let water = world.water_mass[idx];
                if water == 0 {
                    continue;
                }
                // Water must be in Air cell
                if world.get(x, y, z).is_solid() {
                    continue;
                }
                // Check solid below
                if z == 0 {
                    continue;
                }
                let below_idx = world.index(x, y, z - 1);
                let below_tile = world.get(x, y, z - 1);
                if !below_tile.is_solid() {
                    continue;
                }
                let cap = below_tile.moisture_capacity();
                if cap == 0 {
                    continue;
                }
                let current = world.soil_moisture[below_idx];
                let remaining_cap = cap.saturating_sub(current);
                if remaining_cap == 0 {
                    continue;
                }
                let rate = below_tile.absorb_rate();
                let transfer = rate.min(remaining_cap).min(water);
                if transfer == 0 {
                    continue;
                }
                world.water_mass[idx] -= transfer;
                world.soil_moisture[below_idx] += transfer;
            }
        }
    }

    // Phase B: Underground gravity (top → down through soil)
    for z in (1..h).rev() {
        for y in 0..d {
            for x in 0..w {
                let idx = world.index(x, y, z);
                let tile = world.get(x, y, z);
                if tile.moisture_capacity() == 0 {
                    continue;
                }
                let moisture = world.soil_moisture[idx];
                if moisture == 0 {
                    continue;
                }
                let below_idx = world.index(x, y, z - 1);
                let below_tile = world.get(x, y, z - 1);
                let below_cap = below_tile.moisture_capacity();
                if below_cap == 0 {
                    continue;
                }
                let below_current = (world.soil_moisture[below_idx] as i16
                    + world.moisture_delta[below_idx])
                    .max(0) as u8;
                let below_remaining = below_cap.saturating_sub(below_current);
                if below_remaining == 0 {
                    continue;
                }
                let perm = tile.permeability().min(below_tile.permeability());
                let transfer = perm.min(below_remaining).min(moisture);
                if transfer == 0 {
                    continue;
                }
                world.moisture_delta[idx] -= transfer as i16;
                world.moisture_delta[below_idx] += transfer as i16;
            }
        }
    }

    // Phase C: Horizontal pressure equalization
    let dir_offsets: [(isize, isize); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

    for z in 0..h {
        for y in 0..d {
            for x in 0..w {
                let idx = world.index(x, y, z);
                let tile = world.get(x, y, z);
                let cap = tile.moisture_capacity();
                if cap == 0 {
                    continue;
                }
                let moisture =
                    (world.soil_moisture[idx] as i16 + world.moisture_delta[idx]).max(0) as u8;
                if moisture == 0 {
                    continue;
                }

                // Budget: moisture / 8 (slow lateral spread)
                let budget = (moisture as u16) / 8;
                if budget == 0 {
                    continue;
                }

                let mut targets: [(usize, u8); 4] = [(0, 0); 4];
                let mut total_diff: u16 = 0;
                let mut count = 0u8;

                for (i, &(dx, dy)) in dir_offsets.iter().enumerate() {
                    let nx = x as isize + dx;
                    let ny = y as isize + dy;
                    if nx < 0 || nx >= w as isize || ny < 0 || ny >= d as isize {
                        continue;
                    }
                    let nx = nx as usize;
                    let ny = ny as usize;
                    let n_tile = world.get(nx, ny, z);
                    let n_cap = n_tile.moisture_capacity();
                    if n_cap == 0 {
                        continue;
                    }
                    let n_idx = world.index(nx, ny, z);
                    let n_moisture = (world.soil_moisture[n_idx] as i16
                        + world.moisture_delta[n_idx])
                        .max(0) as u8;
                    if n_moisture >= moisture {
                        continue;
                    }
                    let diff = moisture - n_moisture;
                    targets[i] = (n_idx, diff);
                    total_diff += diff as u16;
                    count += 1;
                }

                if count == 0 || total_diff == 0 {
                    continue;
                }

                for &(n_idx, diff) in targets.iter() {
                    if diff == 0 {
                        continue;
                    }
                    let transfer =
                        ((budget as u32 * diff as u32) / total_diff as u32) as i16;
                    if transfer == 0 {
                        continue;
                    }
                    world.moisture_delta[idx] -= transfer;
                    world.moisture_delta[n_idx] += transfer;
                }
            }
        }
    }

    // Phase D: Seepage (soil moisture → surface water at Air boundaries)
    for z in 0..h {
        for y in 0..d {
            for x in 0..w {
                let idx = world.index(x, y, z);
                let tile = world.get(x, y, z);
                let cap = tile.moisture_capacity();
                if cap == 0 {
                    continue;
                }
                let moisture =
                    (world.soil_moisture[idx] as i16 + world.moisture_delta[idx]).max(0) as u8;
                let threshold = cap / 2;
                if moisture <= threshold {
                    continue;
                }

                // Check adjacent Air cells (6 directions: 4 horizontal + up + down not needed for seepage typically)
                // Seepage to side Air cells and above Air cell
                let seep_targets: Vec<usize> = {
                    let mut targets = Vec::new();
                    // Horizontal
                    for &(dx, dy) in &dir_offsets {
                        let nx = x as isize + dx;
                        let ny = y as isize + dy;
                        if nx < 0 || nx >= w as isize || ny < 0 || ny >= d as isize {
                            continue;
                        }
                        let n_idx = world.index(nx as usize, ny as usize, z);
                        if world.get(nx as usize, ny as usize, z).is_air() {
                            targets.push(n_idx);
                        }
                    }
                    // Above
                    if z + 1 < h && world.get(x, y, z + 1).is_air() {
                        targets.push(world.index(x, y, z + 1));
                    }
                    targets
                };

                if seep_targets.is_empty() {
                    continue;
                }

                let available = moisture - threshold;
                let seep_per_target = (available.min(2) as u16 / seep_targets.len() as u16).max(1) as u8;

                for &target_idx in &seep_targets {
                    let seep = seep_per_target.min(available);
                    if seep == 0 {
                        break;
                    }
                    world.moisture_delta[idx] -= seep as i16;
                    world.water_mass[target_idx] = world.water_mass[target_idx].saturating_add(seep);
                    break; // Only seep to first available target per tick (gradual)
                }
            }
        }
    }

    // Phase E: Apply moisture deltas
    world.apply_moisture_deltas();
}
```

**Step 2: mod.rs에 groundwater 모듈 등록 및 tick 순서 수정**

`core/src/water/mod.rs` 수정:

모듈 선언 추가:
```rust
pub mod groundwater;
```

`tick()` 함수에서 `mass_erosion::pass_erosion` 호출 전에 추가:
```rust
// Groundwater (absorption + underground flow + seepage)
groundwater::pass_groundwater(world);
```

**Step 3: 기본 테스트 작성**

`core/src/water/groundwater.rs` 하단에 테스트 모듈:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tile::Tile;
    use crate::world::World;

    #[test]
    fn absorption_moves_surface_water_to_soil() {
        let mut w = World::new(4, 4, 4);
        w.set(1, 1, 0, Tile::Dirt); // capacity=128, rate=2
        w.set_water_mass(1, 1, 1, 100); // water above dirt
        pass_groundwater(&mut w);
        assert!(w.soil_moisture(1, 1, 0) > 0, "Dirt should absorb water");
        assert!(w.water_mass(1, 1, 1) < 100, "Surface water should decrease");
    }

    #[test]
    fn absorption_rate_varies_by_tile() {
        let mut w = World::new(4, 4, 4);
        w.set(0, 0, 0, Tile::Sand);  // rate=8
        w.set(1, 0, 0, Tile::Dirt);  // rate=2
        w.set_water_mass(0, 0, 1, 100);
        w.set_water_mass(1, 0, 1, 100);
        pass_groundwater(&mut w);
        let sand_absorbed = w.soil_moisture(0, 0, 0);
        let dirt_absorbed = w.soil_moisture(1, 0, 0);
        assert!(sand_absorbed > dirt_absorbed,
            "Sand ({}) should absorb faster than Dirt ({})", sand_absorbed, dirt_absorbed);
    }

    #[test]
    fn stone_does_not_absorb() {
        let mut w = World::new(4, 4, 4);
        w.set(1, 1, 0, Tile::Stone);
        w.set_water_mass(1, 1, 1, 100);
        pass_groundwater(&mut w);
        assert_eq!(w.soil_moisture(1, 1, 0), 0);
        assert_eq!(w.water_mass(1, 1, 1), 100);
    }

    #[test]
    fn absorption_respects_capacity() {
        let mut w = World::new(4, 4, 4);
        w.set(1, 1, 0, Tile::Sand); // capacity=48
        w.set_soil_moisture(1, 1, 0, 45); // nearly full
        w.set_water_mass(1, 1, 1, 100);
        pass_groundwater(&mut w);
        assert!(w.soil_moisture(1, 1, 0) <= 48, "Should not exceed capacity");
    }

    #[test]
    fn gravity_moves_moisture_down() {
        let mut w = World::new(4, 4, 4);
        w.set(1, 1, 2, Tile::Dirt);
        w.set(1, 1, 1, Tile::Dirt);
        w.set_soil_moisture(1, 1, 2, 50);
        pass_groundwater(&mut w);
        assert!(w.soil_moisture(1, 1, 1) > 0, "Moisture should flow down");
    }

    #[test]
    fn stone_blocks_underground_flow() {
        let mut w = World::new(4, 4, 4);
        w.set(1, 1, 2, Tile::Dirt);
        w.set(1, 1, 1, Tile::Stone); // impermeable barrier
        w.set(1, 1, 0, Tile::Dirt);
        w.set_soil_moisture(1, 1, 2, 50);
        pass_groundwater(&mut w);
        assert_eq!(w.soil_moisture(1, 1, 0), 0, "Stone should block flow");
    }

    #[test]
    fn seepage_creates_surface_water() {
        let mut w = World::new(5, 5, 4);
        // Solid wall with saturated dirt adjacent to air
        for x in 0..5 { for y in 0..5 { w.set(x, y, 0, Tile::Stone); } }
        w.set(2, 2, 1, Tile::Dirt); // capacity=128, threshold=64
        w.set_soil_moisture(2, 2, 1, 100); // above threshold
        // (1,2,1) is Air → seepage target
        pass_groundwater(&mut w);
        // Check if any adjacent Air cell got water
        let has_seepage = w.water_mass(1, 2, 1) > 0
            || w.water_mass(3, 2, 1) > 0
            || w.water_mass(2, 1, 1) > 0
            || w.water_mass(2, 3, 1) > 0
            || w.water_mass(2, 2, 2) > 0;
        assert!(has_seepage, "Moisture above threshold adjacent to Air should seep");
    }

    #[test]
    fn seepage_requires_threshold() {
        let mut w = World::new(5, 5, 4);
        for x in 0..5 { for y in 0..5 { w.set(x, y, 0, Tile::Stone); } }
        w.set(2, 2, 1, Tile::Dirt); // threshold = 128/2 = 64
        w.set_soil_moisture(2, 2, 1, 30); // below threshold
        pass_groundwater(&mut w);
        let total_water: u16 = (0..5).flat_map(|x| (0..5).flat_map(move |y| (0..4).map(move |z| (x,y,z))))
            .map(|(x,y,z)| w.water_mass(x,y,z) as u16).sum();
        assert_eq!(total_water, 0, "Below threshold should not seep");
    }
}
```

**Step 4: 테스트 실행**

Run: `cd /home/croo12/croo12/core && cargo test groundwater::tests`
Expected: ALL PASS

**Step 5: 전체 테스트 실행**

Run: `cd /home/croo12/croo12/core && cargo test`
Expected: ALL PASS (기존 테스트도 깨지지 않아야 함)

**Step 6: 커밋**

```bash
git add core/src/water/groundwater.rs core/src/water/mod.rs
git commit -m "feat(water): add groundwater system with absorption, flow, and seepage"
```

---

### Task 4: 침식에 수분 배율 적용 (3단계 모델)

**Files:**
- Modify: `core/src/water/mass_erosion.rs`

**Step 1: erosion_multiplier 함수 추가**

`core/src/water/mass_erosion.rs` 상단 (pass_erosion 위)에 추가:

```rust
fn erosion_multiplier(moisture: u8, capacity: u8) -> f64 {
    if capacity == 0 || moisture == 0 {
        return 1.0;
    }
    let ratio = moisture as f64 / capacity as f64;
    if ratio < 0.8 {
        0.4 // Damp: surface tension binds soil
    } else {
        1.8 // Saturated: liquefaction
    }
}
```

**Step 2: pass_erosion의 침식 확률에 배율 적용**

`pass_erosion` 함수의 침식 로직에서, `let chance = ...` 줄 뒤에 수분 배율 적용:

기존:
```rust
let chance = (pressure * 5 + (flow as u64) / 5).min(80);
```

변경:
```rust
let base_chance = (pressure * 5 + (flow as u64) / 5).min(80);
let below_idx = world.index(x, y, z - 1);
let below_moisture = world.soil_moisture[below_idx];
let below_cap = world.get(x, y, z - 1).moisture_capacity();
let multiplier = erosion_multiplier(below_moisture, below_cap);
let chance = ((base_chance as f64 * multiplier) as u64).min(95);
```

**Step 3: 테스트 추가**

`core/src/water/mass_erosion.rs`의 `mod tests`에 추가:

```rust
#[test]
fn damp_soil_resists_erosion() {
    // Wet dirt should be harder to erode than dry dirt
    let mut eroded_dry = 0u32;
    let mut eroded_wet = 0u32;
    for _ in 0..100 {
        // Dry dirt
        let mut w = World::new(4, 4, 4);
        w.set(1, 1, 0, Tile::Dirt);
        w.set_water_mass(1, 1, 1, 200);
        let idx = w.index(1, 1, 1);
        w.water_outflow[idx] = 500;
        pass_erosion(&mut w);
        if w.get(1, 1, 0) == Tile::Air { eroded_dry += 1; }

        // Wet dirt (50% moisture = damp zone)
        let mut w2 = World::new(4, 4, 4);
        w2.set(1, 1, 0, Tile::Dirt);
        w2.set_soil_moisture(1, 1, 0, 64); // 50% of 128
        w2.set_water_mass(1, 1, 1, 200);
        let idx2 = w2.index(1, 1, 1);
        w2.water_outflow[idx2] = 500;
        pass_erosion(&mut w2);
        if w2.get(1, 1, 0) == Tile::Air { eroded_wet += 1; }
    }
    assert!(eroded_wet < eroded_dry,
        "Damp soil ({}) should erode less than dry ({})", eroded_wet, eroded_dry);
}

#[test]
fn saturated_soil_erodes_faster() {
    let mult_dry = erosion_multiplier(0, 128);
    let mult_damp = erosion_multiplier(64, 128);
    let mult_sat = erosion_multiplier(120, 128);
    assert!((mult_dry - 1.0).abs() < 0.01);
    assert!(mult_damp < 1.0);
    assert!(mult_sat > 1.0);
}
```

**Step 4: 테스트 실행**

Run: `cd /home/croo12/croo12/core && cargo test mass_erosion::tests`
Expected: ALL PASS

**Step 5: 커밋**

```bash
git add core/src/water/mass_erosion.rs
git commit -m "feat(erosion): apply 3-stage moisture multiplier (dry/damp/saturated)"
```

---

### Task 5: WASM Export 추가

**Files:**
- Modify: `core/src/lib.rs`

**Step 1: WASM 바인딩 함수 추가**

`core/src/lib.rs`에 기존 water ptr/len 함수들 아래에 추가:

```rust
#[wasm_bindgen]
pub fn world_moisture_ptr() -> *const u8 {
    with_world(|w| w.soil_moisture_ptr())
}

#[wasm_bindgen]
pub fn world_moisture_len() -> usize {
    with_world(|w| w.soil_moisture_len())
}
```

**Step 2: 테스트 실행**

Run: `cd /home/croo12/croo12/core && cargo test`
Expected: ALL PASS

**Step 3: WASM 빌드**

Run: `cd /home/croo12/croo12/core && wasm-pack build --target web --out-dir build/game_core`
Expected: BUILD SUCCESS

**Step 4: 커밋**

```bash
git add core/src/lib.rs core/build/
git commit -m "feat(wasm): export soil moisture pointer for frontend rendering"
```

---

### Task 6: Frontend — WorldData에 soil moisture 통합

**Files:**
- Modify: `src/entities/tile/model/world-data.ts`
- Modify: `src/pages/game/ui/GamePage.tsx`

**Step 1: WorldData에 moisture 필드 추가**

`src/entities/tile/model/world-data.ts` 수정:

constructor에 moisture 매개변수 추가:
```typescript
private moisture: Uint8Array;

constructor(
    width: number,
    depth: number,
    height: number,
    tiles: Uint8Array,
    water: Uint8Array,
    moisture: Uint8Array,
) {
    // ... 기존 코드 ...
    this.moisture = new Uint8Array(moisture);
}
```

updateTiles에 moisture 매개변수 추가:
```typescript
updateTiles(tiles: Uint8Array, water: Uint8Array, moisture: Uint8Array): void {
    this.tiles = new Uint8Array(tiles);
    this.water = new Uint8Array(water);
    this.moisture = new Uint8Array(moisture);
}
```

접근자 추가:
```typescript
getSoilMoisture(x: number, y: number, z: number): number {
    return this.moisture[this.index(x, y, z)];
}
```

**Step 2: GamePage.tsx에서 moisture 데이터 전달**

`src/pages/game/ui/GamePage.tsx` 수정:

import에 추가:
```typescript
import { world_moisture_ptr, world_moisture_len } from "../../../../core/build/game_core";
```

world 생성 시 moisture 추가:
```typescript
const moisturePtr = world_moisture_ptr();
const moistureLen = world_moisture_len();
const moisture = new Uint8Array(wasmOutput.memory.buffer, moisturePtr, moistureLen);
return new WorldData(w, d, h, tiles, water, moisture);
```

tick interval에서 moisture 업데이트:
```typescript
const moisturePtr = world_moisture_ptr();
const moistureLen = world_moisture_len();
const moisture = new Uint8Array(wasmOutput.memory.buffer, moisturePtr, moistureLen);
world.updateTiles(tiles, water, moisture);
```

**Step 3: 타입체크**

Run: `cd /home/croo12/croo12 && npx tsc -b`
Expected: NO ERRORS

**Step 4: 커밋**

```bash
git add src/entities/tile/model/world-data.ts src/pages/game/ui/GamePage.tsx
git commit -m "feat(frontend): integrate soil moisture data from WASM"
```

---

### Task 7: Frontend — 수분에 따른 타일 색상 변화

**Files:**
- Modify: `src/features/terrain-renderer/lib/tile-palette.ts`
- Modify: `src/features/terrain-renderer/ui/IsometricCanvas.tsx`

**Step 1: tile-palette에 수분 색상 조절 함수 추가**

`src/features/terrain-renderer/lib/tile-palette.ts`에 추가:

```typescript
const MOISTURE_CAPACITIES: Record<number, number> = {
    [TileType.Sand]: 48,
    [TileType.Grass]: 160,
    [TileType.Dirt]: 128,
};

const hexToRgb = (hex: string): [number, number, number] => {
    const r = parseInt(hex.slice(1, 3), 16);
    const g = parseInt(hex.slice(3, 5), 16);
    const b = parseInt(hex.slice(5, 7), 16);
    return [r, g, b];
};

export const getMoistureTileFaces = (
    tileType: number,
    moisture: number,
): TileFaces | undefined => {
    const base = palette[tileType];
    if (!base) return undefined;

    const capacity = MOISTURE_CAPACITIES[tileType] ?? 0;
    if (capacity === 0 || moisture === 0) return base;

    const ratio = Math.min(moisture / capacity, 1.0);
    const darkening = 1 - ratio * 0.35;

    const darken = (hex: string): string => {
        const [r, g, b] = hexToRgb(hex);
        const nr = Math.round(r * darkening);
        const ng = Math.round(g * darkening);
        const nb = Math.round(b * darkening);
        return `rgb(${nr},${ng},${nb})`;
    };

    return {
        top: darken(base.top),
        left: darken(base.left),
        right: darken(base.right),
    };
};
```

**Step 2: IsometricCanvas에서 수분 기반 색상 적용**

`src/features/terrain-renderer/ui/IsometricCanvas.tsx` 수정:

import 추가:
```typescript
import { getTileFaces, getMoistureTileFaces } from "../lib/tile-palette";
```

drawTile 호출 부분 수정 (render 함수 내부):

기존:
```typescript
if (tileType !== TileType.Air) {
    drawTile(ctx, sx, sy, tileType, getTileOpacity(tileType));
}
```

변경:
```typescript
if (tileType !== TileType.Air) {
    const moisture = world.getSoilMoisture(x, y, z);
    drawTileWithMoisture(ctx, sx, sy, tileType, moisture, getTileOpacity(tileType));
}
```

drawTileWithMoisture 함수 추가 (drawTile 함수 아래):
```typescript
const drawTileWithMoisture = (
    ctx: CanvasRenderingContext2D,
    sx: number,
    sy: number,
    tileType: number,
    moisture: number,
    alpha: number,
): void => {
    const faces = moisture > 0
        ? getMoistureTileFaces(tileType, moisture)
        : getTileFaces(tileType);
    if (!faces) return;

    const hw = TILE_WIDTH / 2;
    const hh = TILE_HEIGHT / 2;

    ctx.globalAlpha = alpha;

    ctx.fillStyle = faces.top;
    ctx.beginPath();
    ctx.moveTo(sx, sy - hh);
    ctx.lineTo(sx + hw, sy);
    ctx.lineTo(sx, sy + hh);
    ctx.lineTo(sx - hw, sy);
    ctx.closePath();
    ctx.fill();

    ctx.fillStyle = faces.left;
    ctx.beginPath();
    ctx.moveTo(sx - hw, sy);
    ctx.lineTo(sx, sy + hh);
    ctx.lineTo(sx, sy + hh + TILE_DEPTH);
    ctx.lineTo(sx - hw, sy + TILE_DEPTH);
    ctx.closePath();
    ctx.fill();

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
```

**Step 3: 타입체크 및 린트**

Run: `cd /home/croo12/croo12 && npx tsc -b && yarn lint`
Expected: NO ERRORS

**Step 4: 커밋**

```bash
git add src/features/terrain-renderer/
git commit -m "feat(renderer): darken tile colors based on soil moisture level"
```

---

### Task 8: CLI 디버그 테스트 업데이트 및 통합 검증

**Files:**
- Modify: `core/src/lib.rs`

**Step 1: cli_simulation_debug에 지하수 통계 추가**

`core/src/lib.rs`의 `cli_simulation_debug` 테스트에 헬퍼 추가:

```rust
let count_total_moisture = |w: &World| -> u32 {
    let mut total = 0u32;
    for z in 0..w.height() {
        for y in 0..w.depth() {
            for x in 0..w.width() {
                total += w.soil_moisture(x, y, z) as u32;
            }
        }
    }
    total
};

let count_seepage_cells = |w: &World| -> usize {
    let mut count = 0;
    for z in 0..w.height() {
        for y in 0..w.depth() {
            for x in 0..w.width() {
                let tile = w.get(x, y, z);
                let cap = tile.moisture_capacity();
                if cap > 0 && w.soil_moisture(x, y, z) > cap / 2 {
                    count += 1;
                }
            }
        }
    }
    count
};
```

출력에 추가 (기존 println 블록에):
```rust
println!(
    "    moisture: {} | seepage_ready: {}",
    count_total_moisture(&world),
    count_seepage_cells(&world)
);
```

**Step 2: 1000틱 시뮬레이션 실행**

Run: `cd /home/croo12/croo12/core && cargo test cli_simulation_debug -- --nocapture 2>&1 | tail -80`
Expected: 수분 축적과 배출 관련 통계가 출력되어야 함

**Step 3: 전체 테스트 통과 확인**

Run: `cd /home/croo12/croo12/core && cargo test`
Expected: ALL PASS

**Step 4: 커밋**

```bash
git add core/src/lib.rs
git commit -m "feat(debug): add groundwater statistics to CLI simulation output"
```

---

### Task 9: 스케일 문서화 (CLAUDE.md 업데이트)

**Files:**
- Modify: `/home/croo12/croo12/CLAUDE.md`

**Step 1: CLAUDE.md에 세계관 스케일 섹션 추가**

`## Architecture` 섹션 아래에 추가:

```markdown
## Simulation Scale

이중 시간대 모델(Dual Timescale Model)을 사용하는 자연환경 시뮬레이터.

- **공간**: 1 블록 = 10m × 10m × 10m (1,000m³)
- **물리적 시간**: 1 tick ≈ 1.5초 (자유낙하 10m 기준, 유체 역학)
- **지질학적 시간**: 1 tick ≈ 수 개월 (침식/퇴적/지하수, 타임랩스 가속)
- **mass 1단위** ≈ 4,000L (4톤)
- 유체 역학(물 흐름)은 물리적 시간 기준으로 사실적이며, 지질학적 현상은 수억 배 가속된 타임랩스로 동작한다.
```

**Step 2: 커밋**

```bash
git add CLAUDE.md
git commit -m "docs: add simulation scale documentation (dual timescale model)"
```

---

### Task 10: WASM 최종 빌드 및 브라우저 검증

**Step 1: WASM 빌드**

Run: `cd /home/croo12/croo12/core && wasm-pack build --target web --out-dir build/game_core`
Expected: BUILD SUCCESS

**Step 2: 개발 서버 실행**

Run: `cd /home/croo12/croo12 && yarn dev`
Expected: 개발 서버 기동, 브라우저에서 확인

**Step 3: 시각적 확인 사항**

- 비가 내리는 지역의 흙이 서서히 어두워지는지
- 건조한 흙은 밝은 색을 유지하는지
- 절벽/계곡 바닥에서 물이 솟아나는지 (시간이 걸릴 수 있음)
- 기존 물 흐름/침식/증발이 정상 동작하는지

**Step 4: 최종 커밋**

```bash
git add core/build/
git commit -m "chore: rebuild WASM with groundwater system"
```
