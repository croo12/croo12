# Mass-Based Water System Design

## Background

기존 이산 블록 모델(Discrete Block Model)의 한계:
- 소스가 인접 4칸을 채우면 더 이상 생산 불가
- 물 블록이 자리를 비워야 다음 생산 → 지속적 유량 표현 불가
- 결과: 소스 주변 ~30셀 웅덩이, 강 형성 불가

## Architecture: SoA (Structure of Arrays) + 별도 레이어

지형과 물을 완전히 분리. 같은 셀에 지형(Grass) + 물(mass=50) 공존 가능.

### Data Structure

```rust
enum Tile { Air, Grass, Dirt, Stone, Sand }  // Water variant 제거

struct World {
    width: usize, depth: usize, height: usize,

    // 지형 레이어
    tiles: Vec<Tile>,
    tiles_cache: Vec<u8>,         // WASM export (pack)

    // 유체 레이어 (SoA)
    water_mass: Vec<u8>,          // 0~255, zero-copy WASM export
    water_sediment: Vec<u8>,      // 침식/퇴적용 (프론트 불필요)

    // 물리 버퍼
    mass_delta: Vec<i16>,         // 틱 중 변화량 기록
    sediment_delta: Vec<i16>,     // 퇴적물 변화량
    water_outflow: Vec<u16>,      // 셀에서 나간 물의 총량 (유속 측정용)

    // 메타
    sources: Vec<(usize, usize, usize)>,
}
```

### World API

```rust
fn water_mass(&self, x, y, z) -> u8
fn set_water_mass(&mut self, x, y, z, mass: u8)

// delta 기반 이동
fn flow_water(&mut self, from_idx: usize, to_idx: usize, amount: i16)
fn apply_water_deltas(&mut self)  // mass += delta, clamp(0,255), reset
```

### WASM Export

```rust
fn world_water_ptr() -> *const u8   // water_mass.as_ptr() (zero-copy)
fn world_water_len() -> usize
```

`tiles_cache` pack: Water variant 없으므로 3비트(0~4)만 사용.

## Flow Algorithm

매 틱 4 Phase 실행.

### Phase 1: 중력 (위→아래)

```
for each cell (x, y, z) top-down:
    mass = water_mass[i]
    if mass == 0: continue
    if tiles[below].is_solid(): skip to Phase 2

    transfer = min(mass, 255 - water_mass[below])
    mass_delta[i] -= transfer
    mass_delta[below] += transfer
    water_outflow[i] += transfer

    // sediment 비례 이동 (확률적 반올림)
    sed_exact = sediment[i] * transfer / mass  // u32로 캐스팅
    sed_transfer = floor(sed_exact) + (random < frac(sed_exact) ? 1 : 0)
    sediment_delta[i] -= sed_transfer
    sediment_delta[below] += sed_transfer

    remaining = mass - transfer
    // remaining > 0이면 Phase 2 진행
```

### Phase 2: 수평 확산

```
    if remaining == 0: continue

    valid = [n for n in NESW if passable and water_mass[n] < remaining]
    if valid.empty(): continue

    // 자신 포함 n+1 등분 (진자 현상 방지)
    for n in valid:
        actual_transfer = (remaining - water_mass[n]) / (valid.len() + 1)
        mass_delta[i] -= actual_transfer
        mass_delta[n] += actual_transfer
        water_outflow[i] += actual_transfer

        // sediment 비례 이동 (동일 방식)
```

### Phase 3: 수압 (아래→위)

```
for each cell (x, y, z) bottom-up:
    expected = water_mass[i] + mass_delta[i]
    if expected > 255:
        excess = expected - 255
        mass_delta[i] -= excess
        mass_delta[above] += excess
```

### Phase 4: 적용 + 소스

```
apply_water_deltas()
apply_sediment_deltas()
reset outflow

for (x,y,z) in sources:
    water_mass[i] = min(water_mass[i] + SOURCE_RATE, 255)
```

## Erosion / Deposition / Evaporation

### Erosion

유속 = `water_outflow[i]` (net delta가 아닌 실제 유출량).

```
flow = water_outflow[i]
pressure = count_water_above(x, y, z)
chance = (pressure * 5 + flow / 10).min(80)
if random < chance:
    tiles[below] = Air
    water_sediment[i] += 1
```

### Deposition

유속이 낮고 바닥이 solid일 때, 현재 위치를 Sand로 치환.
남은 물은 다음 틱에 수압으로 자동 배출.

```
flow = water_outflow[i]
if flow > 20: continue
if !tiles[below].is_solid(): continue

tiles[i] = Sand
water_sediment[i] -= 1
```

### Evaporation

비소스, 위에 물 없는 셀에서 5% 확률로 소량 증발.

```
if is_source: continue
if water_mass[above] > 0: continue

if random < 5%:
    evap = min(mass, 10)
    water_mass[i] -= evap
    if water_mass[i] == 0 and sediment > 0:
        tiles[i] = Sand
        sediment = 0
```

## Rendering

### Frontend Data

```typescript
class WorldData {
    tiles: Uint8Array;   // 지형 (기존)
    water: Uint8Array;   // 수위 0~255 (신규)

    getWaterMass(x, y, z): number
}
```

### Drawing

지형 블록 위에 mass 비례 높이의 반투명 물 블록 오버레이.

```
if waterMass > 0:
    waterHeight = waterMass / 255 * TILE_DEPTH
    drawWaterTile(sx, sy, waterHeight, alpha=0.3)
```

## Implementation Notes

- sediment 비례 이동 시 정수 절사 방지: u32 캐스팅 + 확률적 반올림
- Tile enum에서 Water 제거 → 기존 `is_water()`, `falls()` 등 모두 제거/수정
- 기존 water/ 모듈 (spread, gravity, erosion, evaporation, source) 전면 교체
- CLI renderer (render/ascii.rs)도 mass 기반으로 갱신
