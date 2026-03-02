# Weather System (Rain & Water Cycle) Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 증발된 물이 대기 수분으로 축적되고, 이동하는 비구름이 생성되어 비를 뿌리는 완전한 물 순환 시스템을 구현한다.

**Architecture:** 글로벌 `atmospheric_moisture: u32` + `Vec<Cloud>` 방식. 기존 증발 패스에서 대기 수분을 채우고, 임계점 도달 시 맵 가장자리에서 구름 생성. 구름은 매 tick 이동하며 반경 내 랜덤 셀에 빗방울 투하. WASM으로 구름 데이터를 프론트엔드에 전달하여 캔버스에 오버레이 렌더링.

**Tech Stack:** Rust (core), wasm-bindgen, React + Canvas 2D

---

### Task 1: Add Cloud struct and weather fields to World

**Files:**
- Modify: `core/src/world/mod.rs:5-18` (World struct)
- Modify: `core/src/world/mod.rs:21-36` (World::new)

**Context:** World 구조체에 대기 수분, 구름 목록, 구름 WASM 버퍼 필드를 추가한다. Cloud는 별도 모듈이 아니라 world/mod.rs에 정의한다 (weather.rs에서 사용하므로 pub(crate)).

**Step 1: Cloud 구조체와 상수를 World 파일 상단에 추가**

`core/src/world/mod.rs` 파일 상단 (`pub mod gravity;` 아래)에 추가:

```rust
pub(crate) const CLOUD_THRESHOLD: u32 = 10000;
pub(crate) const CLOUD_WATER: u32 = 8000;
pub(crate) const DROPS_PER_TICK: u32 = 5;
pub(crate) const RAIN_MASS_PER_DROP: u8 = 5;
pub(crate) const CLOUD_SPEED: f32 = 0.3;
pub(crate) const MAX_CLOUDS: usize = 3;

#[derive(Clone, Debug)]
pub(crate) struct Cloud {
    pub x: f32,
    pub y: f32,
    pub dx: f32,
    pub dy: f32,
    pub water: u32,
    pub radius: f32,
}
```

**Step 2: World 구조체에 필드 추가**

`World` struct (line 5-18)에 `sources` 필드 뒤에 추가:

```rust
pub(crate) atmospheric_moisture: u32,
pub(crate) clouds: Vec<Cloud>,
cloud_buffer: Vec<f32>,
```

**Step 3: World::new에서 초기화**

`World::new` (line 21-36) `sources: Vec::new()` 뒤에 추가:

```rust
atmospheric_moisture: 0,
clouds: Vec::new(),
cloud_buffer: Vec::new(),
```

**Step 4: 구름 데이터 WASM export용 접근자 추가**

`World` impl 블록, `water_outflow` 메서드 뒤에 추가:

```rust
pub fn clouds_count(&self) -> usize {
    self.clouds.len()
}

pub fn sync_cloud_buffer(&mut self) {
    self.cloud_buffer.clear();
    for c in &self.clouds {
        self.cloud_buffer.push(c.x);
        self.cloud_buffer.push(c.y);
        self.cloud_buffer.push(c.radius);
        self.cloud_buffer.push(c.water as f32);
    }
}

pub fn cloud_buffer_ptr(&self) -> *const f32 {
    self.cloud_buffer.as_ptr()
}

pub fn cloud_buffer_len(&self) -> usize {
    self.cloud_buffer.len()
}
```

**Step 5: 테스트 작성**

기존 tests 모듈 내에 추가:

```rust
#[test]
fn world_new_has_empty_weather_state() {
    let w = World::new(4, 4, 4);
    assert_eq!(w.atmospheric_moisture, 0);
    assert!(w.clouds.is_empty());
    assert_eq!(w.clouds_count(), 0);
}

#[test]
fn sync_cloud_buffer_exports_data() {
    let mut w = World::new(4, 4, 4);
    w.clouds.push(Cloud {
        x: 1.5, y: 2.5, dx: 0.3, dy: 0.0,
        water: 8000, radius: 2.5,
    });
    w.sync_cloud_buffer();
    assert_eq!(w.cloud_buffer_len(), 4); // x, y, radius, water
    assert_eq!(w.clouds_count(), 1);
}
```

**Step 6: 테스트 실행**

Run: `cd core && cargo test -- --lib`
Expected: ALL PASS

**Step 7: 커밋**

```bash
git add core/src/world/mod.rs
git commit -m "feat(weather): add Cloud struct and weather fields to World"
```

---

### Task 2: Modify evaporation to feed atmospheric moisture

**Files:**
- Modify: `core/src/water/mass_evaporation.rs:15-47` (pass_evaporation)

**Context:** 현재 `pass_evaporation`은 증발된 물을 그냥 버린다. 이를 `world.atmospheric_moisture`에 축적시킨다.

**Step 1: pass_evaporation에서 증발량을 atmospheric_moisture에 추가**

`core/src/water/mass_evaporation.rs` line 37-38:

현재:
```rust
let evap = mass.min(10);
world.water_mass[idx] = mass - evap;
```

변경:
```rust
let evap = mass.min(10);
world.water_mass[idx] = mass - evap;
world.atmospheric_moisture += evap as u32;
```

**Step 2: 테스트 추가**

기존 tests 모듈에 추가:

```rust
#[test]
fn evaporation_feeds_atmospheric_moisture() {
    let mut w = World::new(4, 4, 4);
    w.set(1, 1, 0, crate::tile::Tile::Stone);
    w.set_water_mass(1, 1, 1, 100);
    let initial_moisture = w.atmospheric_moisture;
    for _ in 0..200 {
        pass_evaporation(&mut w);
    }
    assert!(
        w.atmospheric_moisture > initial_moisture,
        "Atmospheric moisture should increase from evaporation"
    );
}
```

**Step 3: 테스트 실행**

Run: `cd core && cargo test water::mass_evaporation -- --lib`
Expected: ALL PASS (3 tests)

**Step 4: 커밋**

```bash
git add core/src/water/mass_evaporation.rs
git commit -m "feat(weather): feed evaporated water into atmospheric moisture"
```

---

### Task 3: Create weather.rs with cloud spawn and rain logic

**Files:**
- Create: `core/src/water/weather.rs`
- Modify: `core/src/water/mod.rs:1-3` (add module declaration)

**Context:** 핵심 날씨 로직. 두 개의 public 함수: `pass_cloud_spawn` (대기 수분 → 구름 생성), `pass_cloud_update` (구름 이동 + 비).

**Step 1: weather.rs 생성 — 상수/import/해시 함수**

```rust
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
```

**Step 2: pass_cloud_spawn 구현**

```rust
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
        0 => (0.0, along * d, CLOUD_SPEED, 0.0),                    // left → right
        1 => (w - 1.0, along * d, -CLOUD_SPEED, 0.0),               // right → left
        2 => (along * w, 0.0, 0.0, CLOUD_SPEED),                    // top → bottom
        _ => (along * w, d - 1.0, 0.0, -CLOUD_SPEED),               // bottom → top
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
```

**Step 3: pass_cloud_update 구현 (이동 + 비)**

```rust
/// Find the topmost exposed z for a column (x, y).
/// "Exposed" means no solid tile or water above it.
fn find_top_exposed_z(world: &World, x: usize, y: usize) -> usize {
    let h = world.height();
    for z in (0..h).rev() {
        if world.get(x, y, z).is_solid() || world.water_mass(x, y, z) > 0 {
            // The cell above this is the exposed air cell, or this z if it's the top
            return if z + 1 < h { z + 1 } else { z };
        }
    }
    0 // empty column, rain hits z=0
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

        // Check out of bounds → remove
        let margin = cloud.radius;
        if cloud.x < -margin || cloud.x >= w as f32 + margin
            || cloud.y < -margin || cloud.y >= d as f32 + margin
        {
            world.clouds.remove(i);
            continue;
        }

        // Rain: drop DROPS_PER_TICK random drops within radius
        let drops = DROPS_PER_TICK.min(cloud.water / RAIN_MASS_PER_DROP as u32);
        let cx = cloud.x;
        let cy = cloud.y;
        let r = cloud.radius;

        for drop_i in 0..drops {
            let hash = simple_hash(seed.wrapping_add(drop_i as u64), i as u64);
            // Random offset within radius (simple square approximation)
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

        // Remove if depleted
        if cloud.water == 0 {
            world.clouds.remove(i);
            continue;
        }

        i += 1;
    }
}
```

**Step 4: mod.rs에 모듈 선언 추가**

`core/src/water/mod.rs` line 3 뒤에:

```rust
pub mod weather;
```

**Step 5: 테스트 작성**

`weather.rs` 하단에 추가:

```rust
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
        // Floor so rain lands on z=1
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
        // Some water should have been added somewhere at z=1
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
        // Run enough ticks to push cloud off map
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
        // Very little water — will deplete quickly
        w.clouds.push(Cloud {
            x: 4.0, y: 4.0, dx: 0.0, dy: 0.0,
            water: 10, radius: 2.5,
        });
        pass_cloud_update(&mut w);
        assert!(w.clouds.is_empty(), "Cloud with 10 water should deplete in one tick");
    }
}
```

**Step 6: 테스트 실행**

Run: `cd core && cargo test -- --lib`
Expected: ALL PASS

**Step 7: 커밋**

```bash
git add core/src/water/weather.rs core/src/water/mod.rs
git commit -m "feat(weather): add cloud spawn and rain logic"
```

---

### Task 4: Wire up weather passes in tick()

**Files:**
- Modify: `core/src/water/mod.rs:7-28` (tick function)

**Context:** tick()에 weather 패스를 추가하고, sync_cloud_buffer를 호출한다.

**Step 1: tick()에 weather 패스 추가**

현재 tick() (line 7-28):

```rust
pub fn tick(world: &mut World) {
    crate::world::gravity::pass_gravity(world);
    flow::pass_flow(world);

    let sources: Vec<_> = world.sources().to_vec();
    for &(sx, sy, sz) in &sources {
        let idx = world.index(sx, sy, sz);
        world.water_mass[idx] = world.water_mass[idx].saturating_add(50);
    }

    mass_erosion::pass_erosion(world);
    mass_evaporation::pass_evaporation(world);

    world.sync_tiles_cache();
}
```

변경:

```rust
pub fn tick(world: &mut World) {
    crate::world::gravity::pass_gravity(world);
    flow::pass_flow(world);

    let sources: Vec<_> = world.sources().to_vec();
    for &(sx, sy, sz) in &sources {
        let idx = world.index(sx, sy, sz);
        world.water_mass[idx] = world.water_mass[idx].saturating_add(50);
    }

    mass_erosion::pass_erosion(world);
    mass_evaporation::pass_evaporation(world);

    // Weather: cloud movement/rain, then spawn check
    weather::pass_cloud_update(world);
    weather::pass_cloud_spawn(world);

    world.sync_cloud_buffer();
    world.sync_tiles_cache();
}
```

**Step 2: 통합 테스트 추가**

기존 tests 모듈에 추가:

```rust
#[test]
fn tick_weather_cycle_produces_rain() {
    let mut world = World::new(8, 8, 8);
    for x in 0..8 {
        for y in 0..8 {
            world.set(x, y, 0, Tile::Stone);
        }
    }
    // Manually set high atmospheric moisture to trigger cloud
    world.atmospheric_moisture = 10000;

    for _ in 0..50 {
        tick(&mut world);
    }

    // Some water should exist from rain
    let total: u32 = (0..8)
        .flat_map(|x| (0..8).flat_map(move |y| (0..8).map(move |z| (x, y, z))))
        .map(|(x, y, z)| world.water_mass(x, y, z) as u32)
        .sum();
    assert!(total > 0, "Rain should have added water: {}", total);
}
```

**Step 3: 테스트 실행**

Run: `cd core && cargo test -- --lib`
Expected: ALL PASS

**Step 4: 커밋**

```bash
git add core/src/water/mod.rs
git commit -m "feat(weather): wire up cloud passes in tick function"
```

---

### Task 5: Add WASM exports for cloud data

**Files:**
- Modify: `core/src/lib.rs:70-78` (after water exports)

**Context:** 프론트엔드에서 구름 위치/상태를 읽을 수 있도록 WASM 함수를 추가한다.

**Step 1: WASM export 함수 추가**

`core/src/lib.rs`, `world_water_len` 함수 뒤 (line 78 이후)에:

```rust
#[wasm_bindgen]
pub fn world_clouds_ptr() -> *const f32 {
    with_world(|w| w.cloud_buffer_ptr())
}

#[wasm_bindgen]
pub fn world_clouds_len() -> usize {
    with_world(|w| w.cloud_buffer_len())
}

#[wasm_bindgen]
pub fn world_clouds_count() -> usize {
    with_world(|w| w.clouds_count())
}

#[wasm_bindgen]
pub fn world_atmospheric_moisture() -> u32 {
    with_world(|w| w.atmospheric_moisture)
}
```

**Step 2: CLI 시뮬레이션 테스트에 날씨 정보 출력 추가**

`core/src/lib.rs` `cli_simulation_debug` 테스트의 tick 루프 안에 (line 152, `println!` 뒤) 추가:

```rust
println!(
    "  atmos_moisture: {}, clouds: {}",
    count_water_mass(&world), // reuse or add atmos
    world.clouds.len() // need to access
);
```

실제로는 `world`가 이미 `&World`가 아니라 `&mut World`이므로 직접 접근 가능. 출력 포맷을 수정:

기존 line 152:
```rust
println!("\n=== TICK {} | water mass: {} ===", t, count_water_mass(&world));
```

변경:
```rust
println!(
    "\n=== TICK {} | water mass: {} | atmos: {} | clouds: {} ===",
    t, count_water_mass(&world), world.atmospheric_moisture, world.clouds.len()
);
```

**Step 3: WASM 빌드 확인**

Run: `cd core && wasm-pack build --target web --out-dir build`
Expected: 빌드 성공

**Step 4: 커밋**

```bash
git add core/src/lib.rs
git commit -m "feat(weather): add WASM exports for cloud and atmospheric data"
```

---

### Task 6: Update frontend to render clouds

**Files:**
- Modify: `src/pages/game/ui/GamePage.tsx:9-19` (imports)
- Modify: `src/pages/game/ui/GamePage.tsx:51-68` (tick effect)
- Modify: `src/entities/tile/model/world-data.ts` (add cloud data)
- Modify: `src/features/terrain-renderer/ui/IsometricCanvas.tsx` (draw clouds)

**Context:** 프론트엔드에서 구름 데이터를 WASM에서 읽고, 캔버스에 반투명 타원 오버레이로 렌더링한다.

**Step 1: WorldData에 구름 데이터 추가**

`src/entities/tile/model/world-data.ts`:

```typescript
import type { TileTypeValue } from "./tile-type";
import { TileType } from "./tile-type";

const TYPE_MASK = 0x07;

export interface CloudData {
	x: number;
	y: number;
	radius: number;
	water: number;
}

export class WorldData {
	readonly width: number;
	readonly depth: number;
	readonly height: number;
	private tiles: Uint8Array;
	private water: Uint8Array;
	private _clouds: CloudData[] = [];
	private _atmosphericMoisture = 0;

	constructor(
		width: number,
		depth: number,
		height: number,
		tiles: Uint8Array,
		water: Uint8Array,
	) {
		this.width = width;
		this.depth = depth;
		this.height = height;
		this.tiles = new Uint8Array(tiles);
		this.water = new Uint8Array(water);
	}

	private index(x: number, y: number, z: number): number {
		return x + y * this.width + z * this.width * this.depth;
	}

	getTile(x: number, y: number, z: number): TileTypeValue {
		return (this.tiles[this.index(x, y, z)] & TYPE_MASK) as TileTypeValue;
	}

	getWaterMass(x: number, y: number, z: number): number {
		return this.water[this.index(x, y, z)];
	}

	get clouds(): readonly CloudData[] {
		return this._clouds;
	}

	get atmosphericMoisture(): number {
		return this._atmosphericMoisture;
	}

	updateTiles(tiles: Uint8Array, water: Uint8Array): void {
		this.tiles = new Uint8Array(tiles);
		this.water = new Uint8Array(water);
	}

	updateClouds(cloudBuffer: Float32Array, count: number, moisture: number): void {
		this._clouds = [];
		for (let i = 0; i < count; i++) {
			const offset = i * 4;
			this._clouds.push({
				x: cloudBuffer[offset],
				y: cloudBuffer[offset + 1],
				radius: cloudBuffer[offset + 2],
				water: cloudBuffer[offset + 3],
			});
		}
		this._atmosphericMoisture = moisture;
	}

	getTopZ(x: number, y: number): number {
		for (let z = this.height - 1; z >= 0; z--) {
			if (
				this.getTile(x, y, z) !== TileType.Air ||
				this.getWaterMass(x, y, z) > 0
			) {
				return z;
			}
		}
		return 0;
	}
}
```

**Step 2: entities/tile/index.ts에서 CloudData export 추가**

`src/entities/tile/index.ts`에 `CloudData` re-export가 필요하면 추가:

```typescript
export type { CloudData } from "./model/world-data";
```

**Step 3: GamePage.tsx에 구름 WASM import 추가**

`src/pages/game/ui/GamePage.tsx` imports (line 9-19)에 추가:

```typescript
import initGameCore, {
    create_world,
    tick_water,
    world_depth,
    world_height,
    world_tiles_len,
    world_tiles_ptr,
    world_water_len,
    world_water_ptr,
    world_width,
    world_clouds_ptr,
    world_clouds_len,
    world_clouds_count,
    world_atmospheric_moisture,
} from "../../../../core/build/game_core";
```

**Step 4: GamePage.tsx tick effect에서 구름 데이터 전달**

tick effect (line 51-68)의 `setInterval` 콜백에서 `world.updateTiles(tiles, water)` 뒤에 추가:

```typescript
const cloudsCount = world_clouds_count();
const cloudsPtr = world_clouds_ptr();
const cloudsLen = world_clouds_len();
const cloudBuffer = new Float32Array(
    wasmOutput.memory.buffer,
    cloudsPtr,
    cloudsLen,
);
const moisture = world_atmospheric_moisture();
world.updateClouds(cloudBuffer, cloudsCount, moisture);
```

**Step 5: IsometricCanvas.tsx에 구름 오버레이 렌더링 추가**

`src/features/terrain-renderer/ui/IsometricCanvas.tsx`의 render 함수에서 `ctx.restore()` 직전 (line 208 전)에 구름 그리기 코드 추가:

```typescript
// Draw cloud shadows
for (const cloud of world.clouds) {
    const cloudSx = toScreenX(cloud.x, cloud.y);
    const cloudSy = toScreenY(cloud.x, cloud.y, 0);
    const radiusPx = cloud.radius * TILE_WIDTH;
    const radiusPy = cloud.radius * TILE_HEIGHT * 0.5;

    ctx.fillStyle = "rgba(100, 100, 120, 0.25)";
    ctx.beginPath();
    ctx.ellipse(
        cloudSx,
        cloudSy - TILE_DEPTH * 2,
        radiusPx,
        radiusPy,
        0,
        0,
        Math.PI * 2,
    );
    ctx.fill();
}
```

**Step 6: TypeScript 체크 + lint**

Run: `npx tsc -b && yarn lint`
Expected: 0 errors

**Step 7: WASM 빌드**

Run: `cd core && wasm-pack build --target web --out-dir build`
Expected: 빌드 성공

**Step 8: 커밋**

```bash
git add src/entities/tile/model/world-data.ts src/entities/tile/index.ts src/pages/game/ui/GamePage.tsx src/features/terrain-renderer/ui/IsometricCanvas.tsx
git commit -m "feat(frontend): render cloud shadows and atmospheric moisture"
```

---

### Task 7: CLI simulation verification

**Files:**
- None (verification only)

**Step 1: CLI 시뮬레이션 실행**

Run: `cd core && cargo test cli_simulation_debug -- --nocapture`

확인 사항:
- `atmos` 값이 tick이 진행됨에 따라 증가하는지
- `clouds` 값이 0에서 시작해 atmospheric_moisture가 threshold에 도달하면 1+로 증가하는지
- 구름 생성 후 water mass가 비로 인해 추가 증가하는지
- 구름이 일정 시간 후 소멸하는지 (물 소진 or 맵 밖)

**Step 2: 전체 테스트 실행**

Run: `cd core && cargo test -- --lib`
Expected: ALL PASS
