# Water Simulation Improvements Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Tile 구조를 struct로 확장하고, 경사 기반 분배 + 수력침식을 추가하여 자연스러운 강 흐름 구현

**Architecture:** Tile을 u8에서 struct로 변환하고, JS 공유는 packed u16 cache로 처리. WaterSimulator trait의 terrain 파라미터를 &[Tile]로 변경. cellular.rs에 available_depth 가중치 + erosion/deposition 패스 추가.

**Tech Stack:** Rust (wasm-bindgen), TypeScript (React)

---

### Task 1: Tile struct 정의

**Files:**
- Modify: `core/src/tile.rs`

**Step 1: Write the failing test**

`core/src/tile.rs` 하단에 테스트 추가:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_default_is_air_level_8() {
        let tile = Tile::new(TileType::Air);
        assert_eq!(tile.tile_type, TileType::Air);
        assert_eq!(tile.level, 8);
        assert_eq!(tile.moisture, 0);
        assert_eq!(tile.variant, 0);
    }

    #[test]
    fn tile_is_solid() {
        assert!(!Tile::new(TileType::Air).is_solid());
        assert!(Tile::new(TileType::Stone).is_solid());
        assert!(Tile::new(TileType::Grass).is_solid());
        assert!(Tile::new(TileType::Dirt).is_solid());
        assert!(Tile::new(TileType::Sand).is_solid());
        assert!(!Tile::new(TileType::Water).is_solid());
    }

    #[test]
    fn tile_is_erodible() {
        assert!(!Tile::new(TileType::Air).is_erodible());
        assert!(!Tile::new(TileType::Stone).is_erodible());
        assert!(Tile::new(TileType::Grass).is_erodible());
        assert!(Tile::new(TileType::Dirt).is_erodible());
        assert!(Tile::new(TileType::Sand).is_erodible());
        assert!(!Tile::new(TileType::Water).is_erodible());
    }

    #[test]
    fn tile_pack_roundtrip() {
        let tile = Tile {
            tile_type: TileType::Grass,
            level: 5,
            moisture: 3,
            variant: 2,
        };
        let packed = tile.pack();
        // type(4bit) | level(4bit) | variant(2bit) | reserved(6bit)
        let unpacked_type = (packed >> 12) & 0x0F;
        let unpacked_level = (packed >> 8) & 0x0F;
        assert_eq!(unpacked_type, TileType::Grass as u16);
        assert_eq!(unpacked_level, 5);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cd core && cargo test tile::tests -- --nocapture`
Expected: FAIL — `Tile` struct not defined

**Step 3: Write minimal implementation**

`core/src/tile.rs`를 다음으로 교체:

```rust
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TileType {
    Air = 0,
    Grass = 1,
    Dirt = 2,
    Stone = 3,
    Water = 4,
    Sand = 5,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Tile {
    pub tile_type: TileType,
    pub level: u8,      // 0-8, 타일 충전량 (침식 시 점진적 감소)
    pub moisture: u8,   // 0-7, 습도
    pub variant: u8,    // 0-3, 시각적 변형
}

impl Tile {
    pub fn new(tile_type: TileType) -> Self {
        Self {
            tile_type,
            level: 8,
            moisture: 0,
            variant: 0,
        }
    }

    pub fn is_solid(&self) -> bool {
        self.tile_type != TileType::Air && self.tile_type != TileType::Water
    }

    pub fn is_erodible(&self) -> bool {
        matches!(self.tile_type, TileType::Grass | TileType::Dirt | TileType::Sand)
    }

    /// JS 공유용 packed u16: type(4bit) | level(4bit) | variant(2bit) | reserved(6bit)
    pub fn pack(&self) -> u16 {
        ((self.tile_type as u16) << 12)
            | ((self.level as u16 & 0x0F) << 8)
            | ((self.variant as u16 & 0x03) << 6)
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cd core && cargo test tile::tests -- --nocapture`
Expected: PASS — 4 tests pass

**Step 5: Commit**

```bash
git add core/src/tile.rs
git commit -m "feat(tile): add Tile struct with level, moisture, variant and pack method"
```

---

### Task 2: World를 Vec<Tile>로 전환 + tiles_cache

**Files:**
- Modify: `core/src/world.rs`

**Step 1: Write the failing test**

`core/src/world.rs`의 tests 모듈에 추가:

```rust
#[test]
fn world_get_tile_returns_tile_struct() {
    let mut world = World::new(4, 4, 4, CellularWaterSimulator::new());
    world.set_tile(1, 1, 1, TileType::Stone);
    let tile = world.get_tile(1, 1, 1);
    assert_eq!(tile.tile_type, TileType::Stone);
    assert_eq!(tile.level, 8);
}

#[test]
fn world_tiles_cache_ptr_len() {
    let world = World::new(4, 4, 4, CellularWaterSimulator::new());
    assert_eq!(world.tiles_cache_len(), 4 * 4 * 4);
    assert!(!world.tiles_cache_ptr().is_null());
}

#[test]
fn world_sync_tiles_cache_packs() {
    let mut world = World::new(2, 2, 2, CellularWaterSimulator::new());
    world.set_tile(0, 0, 0, TileType::Grass);
    world.sync_tiles_cache();
    // packed: Grass(1) << 12 | 8 << 8 = 0x1800
    let cache = world.tiles_cache();
    assert_eq!(cache[0], 0x1800);
}
```

**Step 2: Run test to verify it fails**

Run: `cd core && cargo test world::tests -- --nocapture`
Expected: FAIL — `get_tile` returns `u8`, not `Tile`

**Step 3: Write minimal implementation**

`core/src/world.rs`의 주요 변경:

```rust
use crate::tile::{Tile, TileType};
use crate::water::{WaterSimulator, WaterState};

pub struct World<S: WaterSimulator> {
    width: usize,
    depth: usize,
    height: usize,
    tiles: Vec<Tile>,
    tiles_cache: Vec<u16>,
    water: WaterState,
    simulator: S,
}

impl<S: WaterSimulator> World<S> {
    pub fn new(width: usize, depth: usize, height: usize, simulator: S) -> Self {
        let size = width * depth * height;
        let tiles = vec![Tile::new(TileType::Air); size];
        let tiles_cache = vec![0u16; size];
        let water = WaterState::new(width, depth, height);
        Self { width, depth, height, tiles, tiles_cache, water, simulator }
    }

    // ... (기존 width/depth/height/index 동일)

    pub fn set_tile(&mut self, x: usize, y: usize, z: usize, tile: TileType) {
        let idx = self.index(x, y, z);
        self.tiles[idx] = Tile::new(tile);
    }

    pub fn get_tile(&self, x: usize, y: usize, z: usize) -> Tile {
        self.tiles[self.index(x, y, z)]
    }

    pub fn tiles(&self) -> &[Tile] {
        &self.tiles
    }

    pub fn tiles_mut(&mut self) -> &mut [Tile] {
        &mut self.tiles
    }

    pub fn sync_tiles_cache(&mut self) {
        for (i, tile) in self.tiles.iter().enumerate() {
            self.tiles_cache[i] = tile.pack();
        }
    }

    pub fn tiles_cache(&self) -> &[u16] {
        &self.tiles_cache
    }

    pub fn tiles_cache_ptr(&self) -> *const u16 {
        self.tiles_cache.as_ptr()
    }

    pub fn tiles_cache_len(&self) -> usize {
        self.tiles_cache.len()
    }

    // tiles_ptr, tiles_len 제거 (기존 u8 포인터 방식 → tiles_cache_ptr로 대체)

    pub fn tick_water(&mut self) {
        self.simulator.tick(&mut self.water, &self.tiles);
        self.water.sync_levels_cache();
    }
    // ... (나머지 water delegation 동일)
}
```

**Step 4: Run test to verify it passes**

Run: `cd core && cargo test world::tests -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add core/src/world.rs
git commit -m "refactor(world): convert tiles from Vec<u8> to Vec<Tile> with packed u16 cache"
```

---

### Task 3: WaterSimulator trait + CellularWaterSimulator 시그니처 변경

**Files:**
- Modify: `core/src/water/mod.rs`
- Modify: `core/src/water/cellular.rs`

**Step 1: 기존 테스트가 컴파일되도록 trait 시그니처 변경**

`core/src/water/mod.rs`에서 trait 변경:

```rust
use crate::tile::Tile;

pub trait WaterSimulator {
    fn tick(&mut self, state: &mut WaterState, terrain: &[Tile]);
    fn place_water(&mut self, state: &mut WaterState, x: usize, y: usize, z: usize, level: u8);
    fn remove_water(&mut self, state: &mut WaterState, x: usize, y: usize, z: usize);
}
```

**Step 2: CellularWaterSimulator 구현 변경**

`core/src/water/cellular.rs`에서:

```rust
use crate::tile::Tile;

impl CellularWaterSimulator {
    fn is_solid(terrain: &[Tile], idx: usize) -> bool {
        terrain[idx].is_solid()
    }
}

impl WaterSimulator for CellularWaterSimulator {
    fn tick(&mut self, state: &mut WaterState, terrain: &[Tile]) {
        // 기존 로직 동일, is_solid 호출은 이미 Self::is_solid 사용 중
        // terrain 타입만 &[u8] → &[Tile]로 변경
        // ...
    }
    // ...
}
```

**Step 3: 테스트 헬퍼도 변경**

`core/src/water/cellular.rs`의 tests에서:

```rust
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
```

**Step 4: Run all tests**

Run: `cd core && cargo test -- --nocapture`
Expected: 모든 기존 테스트 PASS (시그니처만 변경, 로직 동일)

**Step 5: Commit**

```bash
git add core/src/water/mod.rs core/src/water/cellular.rs
git commit -m "refactor(water): change WaterSimulator terrain param from &[u8] to &[Tile]"
```

---

### Task 4: ASCII 렌더러 Tile struct 대응

**Files:**
- Modify: `core/src/render/ascii.rs`

**Step 1: format_cell 시그니처 변경 + 테스트 수정**

`format_cell`의 첫 번째 파라미터를 `u8` → `Tile`로 변경:

```rust
fn format_cell(tile: Tile, water: WaterCell) -> String {
    if water.level > 0 {
        let prefix = if water.is_source { '*' } else { '~' };
        return format!("{}{} ", prefix, water.level);
    }
    match tile.tile_type {
        TileType::Grass => " G ".to_string(),
        TileType::Dirt => " D ".to_string(),
        TileType::Stone => " # ".to_string(),
        TileType::Sand => " S ".to_string(),
        _ => " . ".to_string(),
    }
}
```

render_top_down/render_side에서 `world.get_tile(x, y, z)`가 이제 `Tile`을 반환하므로 직접 전달.

**Step 2: 테스트 수정**

기존 테스트들의 `format_cell(TileType::X as u8, ...)` → `format_cell(Tile::new(TileType::X), ...)`.

**Step 3: Run tests**

Run: `cd core && cargo test render -- --nocapture`
Expected: PASS

**Step 4: Commit**

```bash
git add core/src/render/ascii.rs
git commit -m "refactor(render): update ASCII renderer to use Tile struct"
```

---

### Task 5: terrain.rs Tile struct 사용

**Files:**
- Modify: `core/src/terrain.rs`

**Step 1: set_tile 호출은 이미 TileType을 받으므로 변경 최소**

`generate_terrain`에서 `world.set_tile(x, y, z, tile)`은 여전히 `TileType`을 받으므로 변경 없음.
`world.get_tile(x, y, z)`의 반환값 변경 확인 (테스트에서 `TileType::Water as u8` 비교 → `tile.tile_type == TileType::Water`).

**Step 2: 테스트 수정**

```rust
if world.water().get(x, y, z).level > 0
    || world.get_tile(x, y, z).tile_type == TileType::Water
```

**Step 3: Run tests**

Run: `cd core && cargo test terrain -- --nocapture`
Expected: PASS

**Step 4: Commit**

```bash
git add core/src/terrain.rs
git commit -m "refactor(terrain): update terrain generation to use Tile struct"
```

---

### Task 6: WASM exports 변경 (tiles_cache u16)

**Files:**
- Modify: `core/src/lib.rs`

**Step 1: tiles_ptr → tiles_cache_ptr 변경**

```rust
#[wasm_bindgen]
pub fn world_tiles_ptr() -> *const u16 {
    with_world(|w| w.tiles_cache_ptr(), std::ptr::null())
}

#[wasm_bindgen]
pub fn world_tiles_len() -> usize {
    with_world(|w| w.tiles_cache_len(), 0)
}
```

`create_world`에서 `sync_tiles_cache` 호출 추가:

```rust
#[wasm_bindgen]
pub fn create_world(width: usize, depth: usize, height: usize, seed: u32) {
    let simulator = CellularWaterSimulator::new();
    let mut w = World::new(width, depth, height, simulator);
    terrain::generate_terrain(&mut w, seed);
    w.sync_tiles_cache();
    unsafe {
        *WORLD.0.get() = Some(w);
    }
}
```

**Step 2: Run tests**

Run: `cd core && cargo test -- --nocapture`
Expected: PASS

**Step 3: Commit**

```bash
git add core/src/lib.rs
git commit -m "refactor(wasm): expose tiles as packed u16 cache instead of raw u8 array"
```

---

### Task 7: TS WorldData u16 대응

**Files:**
- Modify: `src/entities/tile/model/world-data.ts`
- Modify: `src/pages/game/ui/GamePage.tsx`

**Step 1: WorldData를 Uint16Array로 변경**

```typescript
// world-data.ts
const TYPE_SHIFT = 12;
const TYPE_MASK = 0x0F;
const LEVEL_SHIFT = 8;
const LEVEL_MASK = 0x0F;

export class WorldData {
    readonly width: number;
    readonly depth: number;
    readonly height: number;
    private readonly tiles: Uint16Array;

    constructor(width: number, depth: number, height: number, tiles: Uint16Array) {
        this.width = width;
        this.depth = depth;
        this.height = height;
        this.tiles = new Uint16Array(tiles);
    }

    getTile(x: number, y: number, z: number): TileTypeValue {
        const packed = this.tiles[x + y * this.width + z * this.width * this.depth];
        return ((packed >> TYPE_SHIFT) & TYPE_MASK) as TileTypeValue;
    }

    getTileLevel(x: number, y: number, z: number): number {
        const packed = this.tiles[x + y * this.width + z * this.width * this.depth];
        return (packed >> LEVEL_SHIFT) & LEVEL_MASK;
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

**Step 2: GamePage.tsx에서 Uint16Array 사용**

```typescript
const tiles = new Uint16Array(wasmOutput.memory.buffer, ptr, len);
```

**Step 3: entities/tile/index.ts에서 export 확인**

WorldData의 `getTileLevel`은 외부에서 사용할 수 있도록 export는 이미 class에서 public.

**Step 4: Run typecheck + lint**

Run: `npx tsc -b && yarn lint`
Expected: PASS

**Step 5: Commit**

```bash
git add src/entities/tile/model/world-data.ts src/pages/game/ui/GamePage.tsx
git commit -m "refactor(ts): update WorldData to read packed u16 tile cache"
```

---

### Task 8: IsometricCanvas에서 tile level 사용

**Files:**
- Modify: `src/features/terrain-renderer/ui/IsometricCanvas.tsx`

**Step 1: 고체 타일 렌더링에서 level 사용**

기존: `drawTile(ctx, sx, sy, tile, 8)` (항상 level 8)
변경: `drawTile(ctx, sx, sy, tile, world.getTileLevel(x, y, z))`

```typescript
// 고체 타일
if (tile !== TileType.Air && tile !== TileType.Water) {
    drawTile(ctx, sx, sy, tile, world.getTileLevel(x, y, z));
}
```

**Step 2: Run typecheck + lint**

Run: `npx tsc -b && yarn lint`
Expected: PASS

**Step 3: Commit**

```bash
git add src/features/terrain-renderer/ui/IsometricCanvas.tsx
git commit -m "feat(renderer): render solid tiles with actual tile level"
```

---

### Task 9: WASM 빌드 + 중간 검증

**Step 1: WASM 빌드**

Run: `cd core && wasm-pack build --target web --out-dir build`
Expected: 빌드 성공

**Step 2: 개발 서버 확인**

Run: `yarn dev`
Expected: 브라우저에서 지형 렌더링 정상 동작 (기존과 동일하게 보임 — 모든 타일이 level=8)

**Step 3: Commit** (필요시)

빌드 산출물이 이미 .gitignore에 있으면 스킵.

---

### Task 10: WaterCell에 sediment 필드 추가

**Files:**
- Modify: `core/src/water/mod.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn water_cell_sediment_default_zero() {
    assert_eq!(WaterCell::EMPTY.sediment, 0);
}

#[test]
fn water_cell_pack_unpack_with_sediment() {
    let cell = WaterCell {
        level: 5,
        is_source: false,
        sediment: 3,
    };
    let packed = cell.pack();
    let unpacked = WaterCell::unpack(packed);
    assert_eq!(unpacked.level, 5);
    assert_eq!(unpacked.sediment, 0); // sediment는 pack에 포함하지 않음 (Rust 내부용)
}
```

**Step 2: Run test to verify it fails**

Run: `cd core && cargo test water::tests -- --nocapture`
Expected: FAIL — `sediment` field not found

**Step 3: Write minimal implementation**

WaterCell에 sediment 추가:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaterCell {
    pub level: u8,
    pub is_source: bool,
    pub sediment: u8,    // 0-8, 운반 중인 퇴적물
}

impl WaterCell {
    pub const EMPTY: Self = Self {
        level: 0,
        is_source: false,
        sediment: 0,
    };
    // pack/unpack은 sediment를 포함하지 않음 (JS에 노출 불필요)
}
```

모든 WaterCell 리터럴 생성 부분에 `sediment: 0` 추가.

**Step 4: Run all tests**

Run: `cd core && cargo test -- --nocapture`
Expected: 모든 테스트 PASS

**Step 5: Commit**

```bash
git add core/src/water/mod.rs core/src/water/cellular.rs core/src/render/ascii.rs
git commit -m "feat(water): add sediment field to WaterCell"
```

---

### Task 11: 경사 기반 분배 (Gradient-Based Spread)

**Files:**
- Modify: `core/src/water/cellular.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn water_prefers_lower_terrain() {
    // 시나리오: 바닥에 계단 지형, 물은 낮은 쪽으로 더 많이 흘러야 함
    let (w, d, h) = (5, 1, 5);
    let mut state = WaterState::new(w, d, h);
    let mut terrain = make_terrain(w, d, h);

    // 계단 바닥: x=0 z=3, x=1 z=2, x=2 z=1, x=3 z=0, x=4 z=0
    for x in 0..w {
        let floor_z = if x <= 2 { 3 - x } else { 0 };
        for z in 0..=floor_z {
            set_terrain(&mut terrain, w, d, x, 0, z, TileType::Stone);
        }
    }

    // x=0, z=4에 물 배치 (계단 꼭대기)
    state.set(0, 0, 4, WaterCell { level: 8, is_source: false, sediment: 0 });

    let mut sim = CellularWaterSimulator::new();
    for _ in 0..10 {
        sim.tick(&mut state, &terrain);
    }

    // 낮은 쪽(x=3,4)에 더 많은 물이 모여야 함
    let low_water: u8 = (3..5).map(|x| {
        (0..h).map(|z| state.get(x, 0, z).level).sum::<u8>()
    }).sum();
    let high_water: u8 = (0..2).map(|x| {
        (0..h).map(|z| state.get(x, 0, z).level).sum::<u8>()
    }).sum();
    assert!(low_water > high_water, "물은 낮은 지형에 더 많아야 함: low={}, high={}", low_water, high_water);
}
```

**Step 2: Run test to verify it fails (or marginally passes)**

Run: `cd core && cargo test water_prefers_lower_terrain -- --nocapture`
Expected: 현재 균등 분배로 인해 FAIL 가능 (10 tick 내에 차이 불확실)

**Step 3: available_depth 함수 구현 + Pass 2 수정**

`core/src/water/cellular.rs`에 추가:

```rust
fn available_depth(
    terrain: &[Tile],
    cells: &[WaterCell],
    w: usize, d: usize,
    nx: usize, ny: usize, z: usize,
) -> usize {
    let mut capacity = 0usize;
    for dz in 0..=z {
        let check_z = z - dz;
        let idx = nx + ny * w + check_z * w * d;
        if terrain[idx].is_solid() { break; }
        capacity += (8 - cells[idx].level) as usize;
    }
    capacity
}
```

Pass 2 수정 — 기존 `n_level < my_level` 차이 비례 대신 available_depth 가중치 사용:

```rust
// Pass 2: Horizontal spread with available_depth weights
for (dx, dy) in neighbors {
    // ... bounds check, solid check 동일 ...
    let n_level = after_gravity[nidx].level;
    if n_level < my_level {
        let depth = Self::available_depth(terrain, &after_gravity, w, d, nx, ny, z);
        let weight = depth.max(1);
        lower_indices[lower_count as usize] = (nidx, weight as u8);
        total_weight += weight;
        lower_count += 1;
    }
}

// 가중치 비례 분배
let total_give = ((my_level as u16) / (lower_count as u16 + 1) as u16) as u8;
// ...분배 로직: weight / total_weight 비례...
```

**Step 4: Run all tests**

Run: `cd core && cargo test -- --nocapture`
Expected: PASS (기존 테스트도 통과해야 함)

**Step 5: Commit**

```bash
git add core/src/water/cellular.rs
git commit -m "feat(water): implement gradient-based spread with available_depth weighting"
```

---

### Task 12: 수력침식 (Erosion Pass)

**Files:**
- Modify: `core/src/water/cellular.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn erosion_reduces_erodible_tile_level() {
    let (w, d, h) = (3, 3, 4);
    let mut state = WaterState::new(w, d, h);
    let mut terrain = make_terrain(w, d, h);

    // z=0에 Dirt 바닥
    for y in 0..d {
        for x in 0..w {
            set_terrain(&mut terrain, w, d, x, y, 0, TileType::Dirt);
        }
    }

    // z=1에 물 배치 (바닥 위)
    state.set(1, 1, 1, WaterCell { level: 8, is_source: false, sediment: 0 });

    let mut sim = CellularWaterSimulator::new();
    sim.tick_with_erosion(&mut state, &mut terrain);

    // Dirt 타일 레벨이 감소해야 함
    let dirt_tile = terrain[1 + 1 * w + 0 * w * d];
    assert!(dirt_tile.level < 8, "침식으로 level이 줄어야 함: {}", dirt_tile.level);
}

#[test]
fn stone_is_not_eroded() {
    let (w, d, h) = (3, 3, 3);
    let mut state = WaterState::new(w, d, h);
    let mut terrain = make_terrain(w, d, h);

    set_terrain(&mut terrain, w, d, 1, 1, 0, TileType::Stone);
    state.set(1, 1, 1, WaterCell { level: 8, is_source: false, sediment: 0 });

    let mut sim = CellularWaterSimulator::new();
    sim.tick_with_erosion(&mut state, &mut terrain);

    assert_eq!(terrain[1 + 1 * w].tile_type, TileType::Stone);
    assert_eq!(terrain[1 + 1 * w].level, 8, "Stone은 침식되면 안 됨");
}

#[test]
fn eroded_tile_becomes_air_at_level_zero() {
    let (w, d, h) = (3, 3, 3);
    let mut state = WaterState::new(w, d, h);
    let mut terrain = make_terrain(w, d, h);

    // level=1인 Sand 배치
    terrain[1 + 1 * w] = Tile { tile_type: TileType::Sand, level: 1, moisture: 0, variant: 0 };
    state.set(1, 1, 1, WaterCell { level: 8, is_source: false, sediment: 0 });

    let mut sim = CellularWaterSimulator::new();
    sim.tick_with_erosion(&mut state, &mut terrain);

    let tile = terrain[1 + 1 * w];
    assert_eq!(tile.tile_type, TileType::Air, "level 0이 되면 Air로 전환");
}
```

**Step 2: Run test to verify it fails**

Run: `cd core && cargo test erosion -- --nocapture`
Expected: FAIL — `tick_with_erosion` not defined

**Step 3: Write erosion implementation**

WaterSimulator trait에 `tick` 시그니처 변경 — terrain을 `&mut [Tile]`로:

```rust
pub trait WaterSimulator {
    fn tick(&mut self, state: &mut WaterState, terrain: &mut [Tile]);
    fn place_water(/* ... */);
    fn remove_water(/* ... */);
}
```

CellularWaterSimulator의 tick에 Pass 3 (Erosion & Deposition) 추가:

```rust
// Pass 3: Erosion & Deposition
for z in 0..h {
    for y in 0..d {
        for x in 0..w {
            let idx = x + y * w + z * w * d;
            let water_level = cells[idx].level;
            if water_level == 0 { continue; }

            // 아래 타일 침식
            if z > 0 {
                let below_idx = x + y * w + (z - 1) * w * d;
                if terrain[below_idx].is_erodible() {
                    let erosion_amount = (water_level / 4).max(1).min(terrain[below_idx].level);
                    terrain[below_idx].level -= erosion_amount;
                    cells[idx].sediment = cells[idx].sediment.saturating_add(erosion_amount);

                    if terrain[below_idx].level == 0 {
                        terrain[below_idx].tile_type = TileType::Air;
                    }
                }
            }

            // 퇴적: 경사가 완만하면 (available_depth 낮음) sediment 방출
            if cells[idx].sediment > 0 {
                let depth = Self::available_depth(terrain, &cells, w, d, x, y, z);
                if depth <= 8 { // 완만한 경사
                    let deposit = cells[idx].sediment.min(2);
                    cells[idx].sediment -= deposit;
                    // 현재 위치 아래에 퇴적
                    if z > 0 {
                        let below_idx = x + y * w + (z - 1) * w * d;
                        if !terrain[below_idx].is_solid() {
                            terrain[below_idx] = Tile {
                                tile_type: TileType::Sand,
                                level: deposit,
                                moisture: 0,
                                variant: 0,
                            };
                        } else {
                            terrain[below_idx].level = (terrain[below_idx].level + deposit).min(8);
                        }
                    }
                }
            }
        }
    }
}
```

NOTE: `tick_with_erosion`은 별도 메서드로 만들지 말고, 기존 `tick`에 통합. 테스트에서는 `tick`을 직접 호출. 테스트 코드를 `sim.tick(&mut state, &mut terrain)`으로 수정.

**Step 4: Run all tests**

Run: `cd core && cargo test -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add core/src/water/mod.rs core/src/water/cellular.rs core/src/world.rs
git commit -m "feat(water): implement hydraulic erosion and deposition in tick Pass 3"
```

---

### Task 13: terrain.rs에서 침식 활용

**Files:**
- Modify: `core/src/terrain.rs`

**Step 1: generate_rivers에서 tick이 침식도 수행하도록 확인**

`world.tick_water()`가 이미 `simulator.tick(&mut water, &mut tiles)`를 호출하므로, CA 안정화 루프에서 자연스럽게 침식 동작.

`world.rs`의 `tick_water`에서 terrain을 `&mut self.tiles`로 전달:

```rust
pub fn tick_water(&mut self) {
    self.simulator.tick(&mut self.water, &mut self.tiles);
    self.water.sync_levels_cache();
}
```

**Step 2: sync_tiles_cache 호출 추가**

`generate_terrain` 또는 `create_world`에서 침식 후 tiles_cache 동기화 필요.
이미 Task 6에서 `create_world` 끝에 `w.sync_tiles_cache()` 추가했으므로 OK.
단, `tick_water` 끝에도 추가:

```rust
pub fn tick_water(&mut self) {
    self.simulator.tick(&mut self.water, &mut self.tiles);
    self.water.sync_levels_cache();
    self.sync_tiles_cache();
}
```

**Step 3: Run all tests**

Run: `cd core && cargo test -- --nocapture`
Expected: PASS

**Step 4: ASCII 렌더러로 10 tick 시각화 테스트 (수동 확인)**

```rust
#[test]
fn inspect_erosion_10_ticks() {
    let mut world = World::new(16, 16, 16, CellularWaterSimulator::new());
    generate_terrain(&mut world, 42);
    let renderer = AsciiRenderer::side_view(8);
    println!("=== Initial ===\n{}", renderer.render(&world));
    for t in 1..=10 {
        world.tick_water();
        if t % 5 == 0 {
            println!("=== Tick {} ===\n{}", t, renderer.render(&world));
        }
    }
}
```

Run: `cd core && cargo test inspect_erosion_10_ticks -- --nocapture --ignored`
(이 테스트는 `#[ignore]`로 마크하여 정규 테스트에 포함되지 않게 함)

**Step 5: 시각화 테스트 제거 후 Commit**

```bash
git add core/src/world.rs core/src/terrain.rs
git commit -m "feat(terrain): integrate erosion into CA stabilization and tick_water"
```

---

### Task 14: WASM 빌드 + 최종 검증

**Step 1: 전체 테스트**

Run: `cd core && cargo test -- --nocapture`
Expected: 모든 테스트 PASS

**Step 2: WASM 빌드**

Run: `cd core && wasm-pack build --target web --out-dir build`
Expected: 빌드 성공

**Step 3: TypeScript typecheck + lint**

Run: `npx tsc -b && yarn lint`
Expected: PASS

**Step 4: 개발 서버 확인**

Run: `yarn dev`
Expected: 브라우저에서 지형 + 물 렌더링 정상 동작, 침식된 타일이 낮은 level로 표시됨

**Step 5: Commit**

빌드 산출물이 변경되었으면:

```bash
git add core/build/
git commit -m "chore: rebuild WASM with water simulation improvements"
```
