# Rendering Interface 분리 설계

## 목적

Rust core에서 디버깅/개발용 ASCII 텍스트 렌더링을 지원하기 위해 렌더링 인터페이스를 trait으로 분리한다.

## 요구사항

- Rust core에서만 변경 (TS 변경 없음)
- `WorldRenderer` trait으로 렌더링 추상화
- `AsciiRenderer` 구현: top-down(z고정) + side view(y고정)
- 각 셀에 타일 종류 + water level + source 여부 표시
- 용도: cargo test, CLI 바이너리에서 시뮬레이션 상태 확인

## 설계

### Trait

```rust
// core/src/render/mod.rs
pub trait WorldRenderer {
    fn render(&self, world: &World<impl WaterSimulator>) -> String;
}
```

- World를 읽기 전용으로 받아서 String 반환
- 뷰 설정은 각 구현체의 내부 상태

### AsciiRenderer

```rust
// core/src/render/ascii.rs
pub enum SliceAxis {
    TopDown(usize),  // z 고정, x-y 평면
    Side(usize),     // y 고정, x-z 단면
}

pub struct AsciiRenderer {
    slice: SliceAxis,
}

impl AsciiRenderer {
    pub fn top_down(z: usize) -> Self;
    pub fn side_view(y: usize) -> Self;
}

impl WorldRenderer for AsciiRenderer { ... }
```

### 셀 포맷 (3문자 고정폭)

| 상태 | 표시 | 설명 |
|------|------|------|
| Air (물 없음) | ` . ` | 빈 공간 |
| Grass | ` G ` | 타일 첫 글자 대문자 |
| Dirt | ` D ` | |
| Stone | ` # ` | |
| Sand | ` S ` | |
| 물 level N | `~N ` | ~N 형태 |
| 물 source level N | `*N ` | *N (source 표시) |

### 출력 예시

top-down (z=1):
```
z=1 (4x4):
 .  .  #  #
 .  ~4 *8  #
 .  ~2 ~3  .
 .  .  ~1  .
```

side view (y=2, z축 역순):
```
y=2 (x->, z^, 4x4):
 .  .  .  .
 .  .  ~3  .
 #  #  *8  #
 #  #  #  #
```

### 모듈 구조

```
core/src/
  render/
    mod.rs      # WorldRenderer trait
    ascii.rs    # SliceAxis, AsciiRenderer
  lib.rs        # mod render; 추가
```

### 데이터 접근

World의 기존 public API만 사용:
- `world.get_tile(x, y, z)` — 타일 종류
- `world.water().get(x, y, z)` — WaterCell (level, is_source)
- `world.width()`, `world.depth()`, `world.height()` — 크기
