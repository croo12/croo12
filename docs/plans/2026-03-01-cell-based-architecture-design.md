# Cell-Based Architecture Design

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Level 개념을 제거하고 바이너리 셀 기반 아키텍처로 전환하여 물리적으로 일관된 물 시뮬레이션 구현

**Architecture:** Tile을 enum으로 재설계하고, Water를 별도 레이어가 아닌 Tile variant로 통합. 높이 해상도를 4배(128)로 증가시키고, Falling Sand 스타일의 셀 기반 시뮬레이션으로 전환.

**Tech Stack:** Rust (WASM core), TypeScript/React (frontend), Canvas (isometric rendering)

---

## Section 1: Data Model & Module Structure

### Tile Enum

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FlowDir {
    None,
    Down,
    North,
    South,
    East,
    West,
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
        sediment: u8,     // 0~7
        velocity: u8,     // 0~7
        direction: FlowDir,
    },
}
```

- 기존 `Tile` struct (type + level + moisture + variant) 완전 제거
- 기존 `WaterCell`, `WaterState` 완전 제거
- Water는 Tile enum의 variant로 통합

### World

```rust
pub struct World {
    width: usize,
    depth: usize,
    height: usize,       // 128 (기존 32)
    tiles: Vec<Tile>,
    tiles_cache: Vec<u8>, // packed cache for JS (기존 Vec<u16>)
}
```

- `WaterSimulator` trait 제거, generic parameter 제거
- water 관련 메서드는 별도 모듈 함수로 분리

### Water Simulation Module Split

```
water/
  mod.rs          — tick 함수 (4개 pass 순서 호출)
  gravity.rs      — Pass 1: 중력 낙하
  spread.rs       — Pass 2: 수평 분배
  erosion.rs      — Pass 3: 침식/퇴적
  source.rs       — Pass 4: 수원 보충
```

기존 `water/cellular.rs` (~480줄 단일 파일) 분리.

---

## Section 2: Water Simulation Mechanics

### Water Tile 필드

- `velocity`: 0~7, 유속. 높을수록 빠른 흐름.
- `direction`: FlowDir, 현재 흐름 방향.
- `sediment`: 0~7, 운반 중인 퇴적물.
- `is_source`: 수원 여부.

### Pass 1: Gravity (gravity.rs)

- 각 Water 셀에서 바로 아래 확인
- Air → 물 이동, `direction = Down`, `velocity = min(vel + 1, 7)` (가속)
- Solid → 이동 불가, velocity 유지한 채 Pass 2로
- 순회: z 높은 곳부터 (위→아래)
- 한 tick에 1칸만 이동

### Pass 2: Horizontal Spread (spread.rs)

방향에 따른 분배 우선순위:

```
현재 direction = East 일 때:

 우선순위: 정면(East) > 측면(North/South) > 후면(West)
```

1. **정면** (현재 direction과 동일): 가장 먼저 시도. Air이면 즉시 이동.
2. **측면** (±90°): 정면이 막혔을 때. 둘 다 가능하면 `scan_depth` 깊은 쪽.
3. **후면** (반대 방향): 정면+측면 모두 막혔을 때만 허용.
4. **전부 막힘**: `velocity = 0`, `direction = None` (정체)

**Down → 수평 전환 (낙하 후 착지):**
- `direction = Down`인 물이 바닥에 도달하면
- 4방향 `scan_depth` 측정 → 가장 깊은 방향을 새 direction으로 설정
- `velocity = max(1, vel / 2)` (낙하 속도에서 감쇠)

**속도에 따른 집중도:**
- `velocity >= 4`: 정면만 시도 (측면 분배 없음, 강한 흐름)
- `velocity 1~3`: 정면 우선, 측면 허용
- `velocity == 0`: 방향 없음, `scan_depth` 기반으로 재선택

**이동 시:** `direction = 선택된 방향`, `velocity = max(1, vel - 1)` (마찰 감속)

순회: z 높은 곳부터 (위→아래)

### Pass 3: Erosion (erosion.rs)

- **조건**: `velocity > 0` (정체 물 제외)
- **확률**: `velocity * 2`% (vel=1 → 2%, vel=7 → 14%)
- 침식 대상: Dirt/Grass/Sand → Air, `sediment += 1`
- Stone은 침식 불가
- **퇴적 조건**: `velocity == 0 && sediment > 0`
- 퇴적 위치: 인접 Air 셀에 Sand 생성, `sediment -= 1`

### Pass 4: Source Replenishment (source.rs)

- Source 위치 추적: `Vec<(usize, usize, usize)>`
- Source 위치가 Air이면 → `Water { is_source: true, sediment: 0, velocity: 0, direction: None }` 생성

---

## Section 3: Terrain Generation

### 높이 스케일 변경

```
기존: height=32, WATER_LEVEL=8, SEA_FLOOR=4
신규: height=128, WATER_LEVEL=32, SEA_FLOOR=16
```

Perlin noise 출력 범위 동일, 결과값 4x 스케일 적용.

### 컬럼 채우기

```
z=128 ┌─────────┐
      │   Air   │
z=45  │  Grass  │  ← surface_height (1칸)
z=44  │  Dirt   │  ← surface - 1~3 (3칸)
z=41  │  Stone  │  ← 나머지 전부
z=16  │  Stone  │  ← SEA_FLOOR
z=0   └─────────┘
```

- `surface_height = SEA_FLOOR + (noise(x, y) * (height - SEA_FLOOR))`
- `z = 0..SEA_FLOOR`: Stone
- `z = SEA_FLOOR..surface-3`: Stone
- `z = surface-3..surface`: Dirt
- `z = surface`: Grass
- `z > surface`: Air

### 초기 수원 배치

- `WATER_LEVEL(32)` 이상 높이의 특정 지점에 Source Water 배치
- Source 위치는 noise 기반으로 산 정상 근처 선택
- 시뮬레이션 시작 시 Source에서 물이 자연스럽게 흘러내림
- 기존 River 초기 배치 로직 제거 → Source 기반 자연 흐름으로 대체

---

## Section 4: Rendering

### 타일 높이

```
기존: height=32, TILE_DEPTH=8px (level 기반 부분 높이)
신규: height=128, TILE_DEPTH=2px (셀당 고정 높이)
```

시각적 총 높이 동일: 128 * 2px = 256px = 32 * 8px

### 가시성 점수 시스템

```
TileType별 opacity:
  Air   = 0.0  (완전 투명, 건너뜀)
  Water = 0.3  (반투명)
  Sand  = 1.0  (불투명)
  Dirt  = 1.0
  Grass = 1.0
  Stone = 1.0
```

컬럼 스캔 로직:

```
(x, y) 컬럼을 z=top부터 아래로 순회:
  accumulated = 0.0
  while accumulated < 1.0 && z >= 0:
    tile = getTile(x, y, z)
    if tile == Air: z -= 1; continue
    drawTile(ctx, sx, sy, tile, alpha = 1.0 - accumulated)
    accumulated += opacity(tile)
    z -= 1
```

효과:
- Water 3칸 + Stone → Water 3개 렌더링(0.9) + Stone(+1.0 → 완료)
- 깊은 물일수록 바닥이 어두워지는 효과
- `drawTile`에서 level 파라미터 제거, alpha 파라미터 추가
- Water 렌더링은 별도 레이어가 아닌 일반 타일과 동일

---

## Section 5: WASM/TS Interface

### Pack 포맷 (u8)

```
Bit:  7      6         5-3          2-0
    [unused][is_source][direction]  [tile_type]

tile_type (3 bits): Air=0, Grass=1, Dirt=2, Stone=3, Sand=4, Water=5
direction (3 bits): None=0, Down=1, North=2, South=3, East=4, West=5
is_source (1 bit):  Water 전용, 나머지 타일은 0
```

- 기존: tiles `Vec<u16>` + water `Vec<u8>` (2개 버퍼)
- 신규: tiles `Vec<u8>` 하나로 통합

### Rust WASM 변경 (`core/src/lib.rs`)

```rust
// 제거
water_levels_ptr()
water_levels_len()

// 변경
tiles_cache_ptr() → *const u8  (기존 *const u16)
tiles_cache_len() → usize
```

### TS 변경

**`src/entities/tile/model/world-data.ts`:**
- `Uint16Array` → `Uint8Array`
- `getTile(x, y, z)`: tile_type (bits 0-2)
- `getFlowDir(x, y, z)`: direction (bits 3-5)
- `getOpacity(tileType)`: 가시성 점수 반환
- `getTileLevel()` 제거

**`src/entities/water/` — 디렉토리 삭제**

**`src/pages/game/ui/GamePage.tsx`:**
- WaterData 관련 코드 전부 제거
- tick interval: `tick_water()` → tiles만 갱신

**`src/features/terrain-renderer/ui/IsometricCanvas.tsx`:**
- `waterData` prop 제거
- `world.getOpacity(type)` 사용하여 가시성 스캔
- `drawTile`에서 level 파라미터 제거, alpha 파라미터 추가

### 메모리

```
기존: 64×64×32 × (2 + 1) = 393,216 bytes
신규: 64×64×128 × 1       = 524,288 bytes
```

4x 높이 증가에도 총 메모리 ~33% 증가로 제한.
