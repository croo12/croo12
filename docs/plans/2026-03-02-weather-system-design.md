# Weather System (Rain & Water Cycle) Design

## Goal

증발된 물이 대기 수분으로 축적되고, 임계점에서 이동하는 비구름이 생성되어 비를 뿌리는 완전한 물 순환 시스템을 구현한다.

## Architecture

글로벌 `atmospheric_moisture` 값 + 추상 `Cloud` 객체 방식. 증발 시스템이 대기 수분을 채우고, 임계점 도달 시 구름 객체를 생성. 구름은 매 tick 이동하며 반경 내 랜덤 셀에 빗방울을 투하. 기존 물 흐름 시스템이 나머지를 처리.

## Data Model

### World에 추가되는 필드

```rust
atmospheric_moisture: u32,     // 글로벌 대기 수분
clouds: Vec<Cloud>,            // 활성 구름 목록
cloud_buffer: Vec<f32>,        // WASM 렌더링용 flat 버퍼
```

### Cloud 구조체

```rust
pub struct Cloud {
    pub x: f32,           // 현재 위치 (서브픽셀 이동)
    pub y: f32,
    pub dx: f32,          // 이동 방향/속도
    pub dy: f32,
    pub water: u32,       // 남은 수분량
    pub radius: f32,      // 비 영향 반경
}
```

### 상수

| 상수 | 값 | 설명 |
|------|------|------|
| `CLOUD_THRESHOLD` | 10000 | 구름 생성 대기 수분 임계점 |
| `CLOUD_WATER` | 8000 | 구름 초기 수분량 |
| `DROPS_PER_TICK` | 5 | 틱당 반경 내 랜덤 빗방울 수 |
| `RAIN_MASS_PER_DROP` | 5 | 빗방울 1개당 water_mass |
| `CLOUD_SPEED` | 0.3 | 틱당 이동 거리 |
| `MAX_CLOUDS` | 3 | 동시 구름 최대 수 |

### 밸런스 검증

- 틱당 소모: 5 drops * 5 mass = 25
- 수명: 8000 / 25 = 320틱
- 이동 거리: 320 * 0.3 = 96칸
- 16x16 맵 기준 충분한 횡단 거리

## Water Cycle Flow

```
증발 (mass_evaporation.rs)
  └→ atmospheric_moisture += evap_amount

대기 수분 축적
  └→ atmospheric_moisture >= CLOUD_THRESHOLD?

구름 생성 (weather.rs)
  └→ Cloud 객체 생성
  └→ atmospheric_moisture -= CLOUD_WATER
  └→ 맵 가장자리에서 진입

구름 이동 + 강수 (weather.rs, 매 tick)
  └→ 위치 이동 (x += dx, y += dy)
  └→ 반경 내 랜덤 셀 선택
  └→ 해당 (x,y)의 최상위 노출 z에 water_mass += RAIN_MASS_PER_DROP
  └→ cloud.water -= DROPS_PER_TICK * RAIN_MASS_PER_DROP
  └→ water == 0 || 맵 밖 → 소멸

물 흐름 (기존 flow.rs)
  └→ 비로 추가된 물이 기존 시스템으로 흘러감

다시 증발 → 반복
```

## Tick Order

```rust
pub fn tick(world: &mut World) {
    gravity::pass_gravity(world);
    flow::pass_flow(world);
    source_replenishment(world);
    mass_erosion::pass_erosion(world);
    mass_evaporation::pass_evaporation(world);  // 증발 → atmospheric_moisture
    weather::pass_cloud_update(world);           // 구름 이동 + 비
    weather::pass_cloud_spawn(world);            // 구름 생성 체크
    world.sync_tiles_cache();
}
```

## Cloud Spawn Rules

- **생성 위치:** 맵 가장자리 4변 중 하나에서 진입
- **바람 방향:** 진입 변의 반대 방향 + 시드 기반 랜덤 각도 편차
- **동시 제한:** MAX_CLOUDS (3)개 초과 시 생성하지 않음
- **소멸:** water == 0 또는 맵 밖 이탈

## Code Changes

### 새 파일
- `core/src/water/weather.rs` — Cloud 구조체, pass_cloud_update, pass_cloud_spawn

### 수정 파일
- `core/src/world/mod.rs` — atmospheric_moisture, clouds, cloud_buffer 필드 추가, WASM export
- `core/src/water/mod.rs` — tick()에 weather 패스 추가
- `core/src/water/mass_evaporation.rs` — 증발된 양을 atmospheric_moisture에 전달
- `core/src/lib.rs` — world_clouds_ptr/len/count WASM 함수
- `src/entities/tile/model/world-data.ts` — 구름 데이터 로드
- `src/features/terrain-renderer/ui/IsometricCanvas.tsx` — 구름 오버레이 렌더링

## Frontend Rendering

### WASM 데이터 전달

```rust
// cloud_buffer: [x, y, radius, water, x, y, radius, water, ...]
pub fn world_clouds_ptr() -> *const f32
pub fn world_clouds_len() -> usize
pub fn world_clouds_count() -> usize
```

### 캔버스 렌더링 (1차)

- 타일 렌더링 완료 후 구름 오버레이
- 아이소메트릭 좌표 변환 → 반투명 타원으로 그림자 표현
- 대기 수분 게이지 (atmospheric_moisture / CLOUD_THRESHOLD) HUD에 표시

### 향후 개선

- 타일 렌더링 루프 내에서 tint로 그림자 정확도 향상 (Z-인덱스 이슈 해결)
- `lineTo` 기반 비 파티클 효과
