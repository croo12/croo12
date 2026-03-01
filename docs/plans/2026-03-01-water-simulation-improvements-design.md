# 물 시뮬레이션 개선 설계

## 현재 상태

CA(Cellular Automata) 기반 물 시뮬레이션이 동작 중이나, 실제 강과 다른 결과를 보임.

### 현재 동작 방식
1. **중력**: 아래 셀이 비어있으면 한 칸씩 낙하
2. **수평 분배**: 4방향 이웃과 물 레벨 차이를 균등 분배
3. **수원 보충**: is_source 셀은 매 틱 level=8 유지

### 문제점
- 물이 모든 방향으로 균등하게 퍼져서 얇은 웅덩이가 됨
- 지형 경사를 무시하여 강처럼 한 방향으로 흐르지 않음
- 지형이 고정되어 자연적인 강 채널이 형성되지 않음
- 타일이 type만 가지고 있어 부분 침식 불가

## 구현 범위

이번 구현: **타일 구조 확장 + 경사 기반 분배 + 수력침식**
향후: 모멘텀 (선택적)

## 설계

### 1. Tile 구조 확장

현재 타일은 `u8` (TileType만)인데, struct로 확장한다.

```rust
pub struct Tile {
    pub tile_type: TileType,
    pub level: u8,      // 0-8, 타일 충전량 (침식 시 점진적 감소)
    pub moisture: u8,   // 0-7, 습도 (침식 속도 영향)
    pub variant: u8,    // 0-3, 시각적 변형
}
```

**JS 공유**: Rust 내부는 Tile struct, JS에는 렌더링용 packed cache 제공.
WaterState의 levels_cache와 동일한 패턴.

```
tiles_cache: Vec<u16>
팩킹: type(4bit) | level(4bit) | variant(2bit) | reserved(6bit)
```

**영향 범위**:
- `tile.rs`: Tile struct + pack/unpack
- `world.rs`: `Vec<u8>` → `Vec<Tile>`, tiles_cache 추가, sync 메서드
- `terrain.rs`: 타일 생성 시 level=8로 배치
- TS `WorldData`: u16 배열 읽기, getTile/getTileLevel 마스킹
- TS `IsometricCanvas`: drawTile에 이미 level 파라미터 있으므로 최소 변경

### 2. 경사 기반 분배 (Gradient-Based Spread)

수평 분배(Pass 2)에서 이웃의 **가용 깊이(available depth)**를 가중치로 사용.

```rust
fn available_depth(terrain: &[Tile], water: &[WaterCell],
                   w: usize, d: usize, nx: usize, ny: usize, z: usize) -> usize {
    let mut capacity = 0;
    for dz in 0..=z {
        let check_z = z - dz;
        let idx = nx + ny * w + check_z * w * d;
        if terrain[idx].is_solid() { break; }
        capacity += 8 - water[idx].level;
    }
    capacity
}
```

- 아래로 스캔하여 비어있는 공간(고체를 만나면 중단)의 총 남은 용량 계산
- 가용 깊이가 큰 이웃에게 더 많이 분배
- 이미 물이 차 있는 방향은 가중치 낮아짐
- 성능: 매 셀마다 아래 스캔 O(h). 추후 최적화 가능.

### 3. 수력침식 (Hydraulic Erosion)

tick에 새로운 패스 추가 (Pass 3).

**WaterCell 변경**:
```rust
pub struct WaterCell {
    pub level: u8,
    pub is_source: bool,
    pub sediment: u8,    // 운반 중인 퇴적물량 (0-8)
}
```

**침식 규칙**:
- 물 level > 0이고 아래 타일이 침식 가능(Dirt, Sand, Grass)이면 침식
- 침식량: 물 level × 경사(available_depth) 비례, 타일 level에서 차감
- 타일 level이 0이 되면 Air로 전환
- Stone은 침식 불가

**퇴적 규칙**:
- 경사가 완만하거나(available_depth 낮음) 물이 정체되면 퇴적
- sediment에서 차감, 해당 위치 타일 level 증가 또는 Sand 타일 생성

**지형 생성 적용**:
- `generate_rivers`에서 수원 배치 후 CA 안정화 시 침식 함께 동작
- 자연스러운 강 채널 형성

### 4. 최종 tick 순서

```
Pass 1: Gravity (기존)
Pass 2: Horizontal spread + 가용깊이 가중치 (수정)
Pass 3: Erosion & Deposition (신규)
Pass 4: Source replenishment (기존)
```

### 5. 변경 파일 요약

| 파일 | 변경 |
|------|------|
| `tile.rs` | TileType → Tile struct, pack/unpack |
| `water/mod.rs` | WaterCell에 sediment 추가 |
| `water/cellular.rs` | Pass 2 가용깊이 가중치 + Pass 3 침식/퇴적 |
| `world.rs` | `Vec<u8>` → `Vec<Tile>`, tiles_cache, sync |
| `terrain.rs` | Tile struct 사용, level=8 배치, 안정화 조정 |
| `render/ascii.rs` | Tile struct 대응, level 표시 |
| TS `world-data.ts` | u16 packed cache 읽기, getTileLevel 추가 |
| TS `IsometricCanvas.tsx` | 타일 level 반영 (최소 변경) |
| TS `GamePage.tsx` | tiles 배열 Uint16Array로 변경 |

## 모멘텀 (향후)

경사 분배 + 침식으로 충분히 자연스러우면 생략 가능.
필요 시 WaterCell에 vx/vy 속도 벡터 추가.
