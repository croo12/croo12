# 지하수 시스템 (Groundwater System) 설계 문서

## 세계관 스케일 (프로젝트 공통)

| 항목 | 값 | 비고 |
|------|---|------|
| 1 블록 | 10m × 10m × 10m (1,000m³) | 복셀 시뮬레이션 표준 |
| mass 1단위 | ~4,000L (4톤) | 255 / 1,000m³ |
| 물리적 시간 | 1 tick ≈ 1.5초 | 자유낙하 10m ≈ 1.43초 |
| 지질학적 시간 | 1 tick ≈ 수 개월 | 침식/퇴적/지하수 가속 |
| 이중 시간대 모델 | 물리 엔진 + 지질 엔진 병행 | 게임적 허용(Game Abstraction) |

> **유체 역학**(물 흐름, 낙하)은 물리적 시간 기준으로 사실적이며,
> **지질학적 현상**(침식, 퇴적, 지하수 이동)은 수억 배 가속된 타임랩스로 동작한다.

---

## 목표

흙이 물을 흡수하고, 지하수가 중력과 압력으로 이동하며, 절벽·계곡에서 샘물이 솟아나는 완전한 수문학적 순환을 구현한다.

## 아키텍처

기존 mass-based cellular automata 패턴을 확장하여 `soil_moisture: Vec<u8>` 레이어를 추가한다. 지표수 `flow.rs`와 동일한 delta-based 접근법으로 안정적인 지하수 흐름을 구현한다.

---

## 1. 새로운 데이터

### World 구조체 추가 필드

```rust
pub(crate) soil_moisture: Vec<u8>,       // 고체 타일 수분량 (0~255)
pub(crate) moisture_delta: Vec<i16>,      // 틱 내 수분 변화 누적
```

### WASM Export 추가

```rust
#[wasm_bindgen]
pub fn world_moisture_ptr() -> *const u8;
#[wasm_bindgen]
pub fn world_moisture_len() -> usize;
```

---

## 2. 타일별 토양 특성

| 타일 | 용량 (capacity) | 흡수 속도 (absorb_rate) | 투수율 (permeability) | 비고 |
|------|----------------|----------------------|---------------------|------|
| Sand | 48 | 8/tick | 높음 (6) | 초고속 배수 |
| Grass | 160 | 5/tick | 중간 (3) | 뿌리 스펀지 |
| Dirt | 128 | 2/tick | 낮음 (1) | 표면 경화, 유출 |
| Stone | 0 | 0 | 0 | 완전 불투수 |

### Tile 메서드 추가

```rust
impl Tile {
    pub fn moisture_capacity(&self) -> u8 { ... }
    pub fn absorb_rate(&self) -> u8 { ... }
    pub fn permeability(&self) -> u8 { ... }
}
```

---

## 3. 흡수 메커니즘 (Absorption)

**조건**: 지표수(Air 셀의 water_mass > 0)가 고체 타일 위에 있을 때

```
transfer = min(absorb_rate, remaining_capacity, surface_water_mass)
surface_water_mass -= transfer
soil_moisture += transfer
```

- Sand는 8/tick으로 빠르게 빨아들여 지표수 유출 없음
- Dirt는 2/tick으로 느려서 지표수가 넘쳐흘러 침식 유발
- Grass는 5/tick으로 중간, 홍수 방지 효과

**처리 위치**: `pass_flow` 이후, `pass_erosion` 이전 (새 패스 `pass_groundwater`)

---

## 4. 지하수 흐름 (Underground Flow)

`flow.rs`의 Phase 1(중력) + Phase 2(수평 균등화)와 동일한 패턴.

### Phase A: 중력 (위→아래)

- 수분이 아래 고체 타일로 이동
- `transfer = min(permeability, moisture, below_remaining_capacity)`
- Stone 아래는 차단

### Phase B: 수평 압력 균등화

- 인접 고체 셀과 수분 차이에 비례하여 이동
- `budget = moisture / 8` (지표수의 1/4 속도)
- Stone은 차단벽

### Phase C: Delta 적용

```rust
soil_moisture[i] = (soil_moisture[i] as i16 + moisture_delta[i]).clamp(0, capacity)
```

---

## 5. 배출 (Seepage / 샘물)

**조건**: `moisture > capacity * 50%` AND `Air 인접`

```
threshold = capacity / 2
seep_amount = min(2, moisture - threshold)
soil_moisture -= seep_amount
adjacent_air_water_mass += seep_amount
```

- 절벽 측면, 계곡 바닥에서 자연스러운 샘물 형성
- 낮은 지대로 지하수가 모여 배출 → 오아시스/샘물
- 배출량은 소량(2/tick)으로 졸졸 흐르는 느낌

---

## 6. 침식 상호작용 (3단계 모델)

`pass_erosion`에서 수분 상태에 따른 침식 배율 적용:

| 수분 상태 | 범위 | 배율 | 현상 |
|----------|------|------|------|
| 건조 (Dry) | moisture = 0 | 1.0x | 기본 침식 |
| 젖음 (Damp) | 1% ~ 80% | 0.4x | 표면장력이 흙을 묶음 (모래성 원리) |
| 포화 (Saturated) | 80% ~ 100% | 1.8x | 액상화, 산사태/붕괴 |

```rust
fn erosion_multiplier(moisture: u8, capacity: u8) -> f32 {
    if capacity == 0 || moisture == 0 { return 1.0; }
    let ratio = moisture as f32 / capacity as f32;
    if ratio < 0.8 { 0.4 } else { 1.8 }
}
```

---

## 7. 렌더링 (Frontend)

### WASM → Frontend 데이터 흐름

1. `world_moisture_ptr()` / `world_moisture_len()`으로 soil_moisture 배열 export
2. `WorldData`에 `moisture: Uint8Array` 추가
3. `getSoilMoisture(x, y, z)` 접근자 추가

### 색상 변환

수분 비율에 따라 타일 색상을 어둡게:

```typescript
function applyMoisture(baseColor: string, moisture: number, capacity: number): string {
    if (capacity === 0) return baseColor;
    const ratio = moisture / capacity;
    const darkening = ratio * 0.35; // 최대 35% 어두워짐
    // RGB 각 채널에 (1 - darkening) 곱하기
}
```

- 건조한 Dirt: `#b87840` (밝은 갈색)
- 젖은 Dirt: `#785030` (어두운 갈색)
- 포화된 Dirt: `#604028` (진흙색)

---

## 8. Tick 순서 (수정됨)

```
1. pass_gravity         (고체 타일 중력)
2. pass_flow            (지표수 흐름)
3. source replenishment (수원 보충)
4. pass_groundwater     (흡수 + 지하수 흐름 + 배출)  ← NEW
5. pass_erosion         (침식/퇴적, 수분 배율 적용)   ← MODIFIED
6. pass_evaporation     (증발)
7. weather              (구름/비)
8. sync buffers         (WASM export)
```

---

## 9. CLI 디버그 출력 추가

`cli_simulation_debug` 테스트에 지하수 관련 통계 추가:
- 총 soil_moisture 합계
- 샘물 활성 셀 개수
- 포화된 셀 개수
