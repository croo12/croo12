# Rendering Interface 분리 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rust core에 `WorldRenderer` trait과 `AsciiRenderer`를 추가하여 디버깅용 텍스트 렌더링을 지원한다.

**Architecture:** `WorldRenderer` trait으로 렌더링을 추상화하고, `AsciiRenderer`가 이를 구현한다. SliceAxis enum으로 top-down(z고정)/side view(y고정)를 선택하며, 셀은 3문자 고정폭으로 표시한다.

**Tech Stack:** Rust (core/ 크레이트), cargo test

---

### Task 1: WorldRenderer trait 생성

**Files:**
- Create: `core/src/render/mod.rs`
- Modify: `core/src/lib.rs:1` — `mod render;` 추가

**Step 1: Create render module with trait**

```rust
// core/src/render/mod.rs
pub mod ascii;

use crate::water::WaterSimulator;
use crate::world::World;

pub trait WorldRenderer {
	fn render(&self, world: &World<impl WaterSimulator>) -> String;
}
```

**Step 2: Add mod render to lib.rs**

`core/src/lib.rs` 최상단 mod 선언부에 추가:
```rust
mod render;
```

**Step 3: Create empty ascii module**

```rust
// core/src/render/ascii.rs
```

빈 파일로 생성 (컴파일 확인용).

**Step 4: Verify compilation**

Run: `cd core && cargo check`
Expected: 컴파일 성공 (warning 허용)

**Step 5: Commit**

```bash
git add core/src/render/ core/src/lib.rs
git commit -m "feat(render): add WorldRenderer trait and render module"
```

---

### Task 2: AsciiRenderer 구조체 및 셀 포맷 함수

**Files:**
- Modify: `core/src/render/ascii.rs`

**Step 1: Write failing test — cell formatting**

`core/src/render/ascii.rs` 하단에 테스트 모듈 추가:

```rust
#[cfg(test)]
mod tests {
	use super::*;
	use crate::tile::TileType;
	use crate::water::WaterCell;

	#[test]
	fn format_cell_air() {
		assert_eq!(format_cell(TileType::Air as u8, WaterCell::EMPTY), " . ");
	}

	#[test]
	fn format_cell_stone() {
		assert_eq!(format_cell(TileType::Stone as u8, WaterCell::EMPTY), " # ");
	}

	#[test]
	fn format_cell_grass() {
		assert_eq!(format_cell(TileType::Grass as u8, WaterCell::EMPTY), " G ");
	}

	#[test]
	fn format_cell_water_level() {
		let cell = WaterCell { level: 4, is_source: false };
		assert_eq!(format_cell(TileType::Air as u8, cell), "~4 ");
	}

	#[test]
	fn format_cell_water_source() {
		let cell = WaterCell { level: 8, is_source: true };
		assert_eq!(format_cell(TileType::Air as u8, cell), "*8 ");
	}

	#[test]
	fn format_cell_water_on_solid_shows_water() {
		let cell = WaterCell { level: 3, is_source: false };
		assert_eq!(format_cell(TileType::Stone as u8, cell), "~3 ");
	}
}
```

**Step 2: Run tests to verify they fail**

Run: `cd core && cargo test --lib render::ascii::tests`
Expected: FAIL — `format_cell` not found

**Step 3: Implement format_cell and structs**

`core/src/render/ascii.rs` 상단에 구현:

```rust
use crate::tile::TileType;
use crate::water::WaterCell;

pub enum SliceAxis {
	TopDown(usize),
	Side(usize),
}

pub struct AsciiRenderer {
	slice: SliceAxis,
}

impl AsciiRenderer {
	pub fn top_down(z: usize) -> Self {
		Self { slice: SliceAxis::TopDown(z) }
	}

	pub fn side_view(y: usize) -> Self {
		Self { slice: SliceAxis::Side(y) }
	}
}

fn format_cell(tile: u8, water: WaterCell) -> String {
	if water.level > 0 {
		let prefix = if water.is_source { '*' } else { '~' };
		return format!("{}{} ", prefix, water.level);
	}
	match tile {
		t if t == TileType::Grass as u8 => " G ".to_string(),
		t if t == TileType::Dirt as u8 => " D ".to_string(),
		t if t == TileType::Stone as u8 => " # ".to_string(),
		t if t == TileType::Sand as u8 => " S ".to_string(),
		_ => " . ".to_string(),
	}
}
```

**Step 4: Run tests to verify they pass**

Run: `cd core && cargo test --lib render::ascii::tests`
Expected: 6 tests PASS

**Step 5: Commit**

```bash
git add core/src/render/ascii.rs
git commit -m "feat(render): add AsciiRenderer struct and cell formatting"
```

---

### Task 3: WorldRenderer trait 구현 — top-down 렌더링

**Files:**
- Modify: `core/src/render/ascii.rs`

**Step 1: Write failing test — top-down render**

테스트 모듈에 추가:

```rust
use crate::water::cellular::CellularWaterSimulator;
use crate::world::World;
use crate::render::WorldRenderer;

#[test]
fn render_top_down_empty_world() {
	let world = World::new(3, 3, 2, CellularWaterSimulator::new());
	let renderer = AsciiRenderer::top_down(0);
	let output = renderer.render(&world);
	let expected = "z=0 (3x3):\n .  .  . \n .  .  . \n .  .  . \n";
	assert_eq!(output, expected);
}

#[test]
fn render_top_down_with_tiles_and_water() {
	let mut world = World::new(3, 3, 2, CellularWaterSimulator::new());
	world.set_tile(0, 0, 0, TileType::Stone);
	world.set_tile(1, 0, 0, TileType::Grass);
	world.place_water(2, 0, 0, 5);
	let renderer = AsciiRenderer::top_down(0);
	let output = renderer.render(&world);
	let expected = "z=0 (3x3):\n #  G ~5 \n .  .  . \n .  .  . \n";
	assert_eq!(output, expected);
}
```

**Step 2: Run tests to verify they fail**

Run: `cd core && cargo test --lib render::ascii::tests::render_top_down`
Expected: FAIL — `WorldRenderer` not implemented

**Step 3: Implement top-down rendering**

`ascii.rs`에 import 추가 및 trait 구현:

```rust
use crate::render::WorldRenderer;
use crate::water::WaterSimulator;
use crate::world::World;

impl AsciiRenderer {
	fn render_top_down(&self, world: &World<impl WaterSimulator>, z: usize) -> String {
		let w = world.width();
		let d = world.depth();
		let mut out = format!("z={} ({}x{}):\n", z, w, d);
		for y in 0..d {
			for x in 0..w {
				out.push_str(&format_cell(world.get_tile(x, y, z), world.water().get(x, y, z)));
			}
			out.push('\n');
		}
		out
	}
}

impl WorldRenderer for AsciiRenderer {
	fn render(&self, world: &World<impl WaterSimulator>) -> String {
		match self.slice {
			SliceAxis::TopDown(z) => self.render_top_down(world, z),
			SliceAxis::Side(_y) => String::new(), // Task 4에서 구현
		}
	}
}
```

**Step 4: Run tests to verify they pass**

Run: `cd core && cargo test --lib render::ascii::tests`
Expected: 모든 테스트 PASS

**Step 5: Commit**

```bash
git add core/src/render/ascii.rs
git commit -m "feat(render): implement top-down ASCII rendering"
```

---

### Task 4: WorldRenderer trait 구현 — side view 렌더링

**Files:**
- Modify: `core/src/render/ascii.rs`

**Step 1: Write failing test — side view render**

테스트 모듈에 추가:

```rust
#[test]
fn render_side_view_empty_world() {
	let world = World::new(3, 3, 2, CellularWaterSimulator::new());
	let renderer = AsciiRenderer::side_view(0);
	let output = renderer.render(&world);
	// z는 위에서 아래로 (z=1 먼저, z=0 나중)
	let expected = "y=0 (x->, z^, 3x2):\n .  .  . \n .  .  . \n";
	assert_eq!(output, expected);
}

#[test]
fn render_side_view_with_tiles_and_water() {
	let mut world = World::new(3, 3, 2, CellularWaterSimulator::new());
	world.set_tile(0, 0, 0, TileType::Stone);
	world.set_tile(1, 0, 0, TileType::Stone);
	world.place_water(2, 0, 1, 6);
	let renderer = AsciiRenderer::side_view(0);
	let output = renderer.render(&world);
	// z=1 행 먼저 (상단), z=0 행 나중 (하단)
	let expected = "y=0 (x->, z^, 3x2):\n .  . ~6 \n #  #  . \n";
	assert_eq!(output, expected);
}
```

**Step 2: Run tests to verify they fail**

Run: `cd core && cargo test --lib render::ascii::tests::render_side_view`
Expected: FAIL — Side branch returns empty string

**Step 3: Implement side view rendering**

`AsciiRenderer` impl 블록에 추가:

```rust
fn render_side(&self, world: &World<impl WaterSimulator>, y: usize) -> String {
	let w = world.width();
	let h = world.height();
	let mut out = format!("y={} (x->, z^, {}x{}):\n", y, w, h);
	for z in (0..h).rev() {
		for x in 0..w {
			out.push_str(&format_cell(world.get_tile(x, y, z), world.water().get(x, y, z)));
		}
		out.push('\n');
	}
	out
}
```

`WorldRenderer::render`의 `Side` 브랜치 업데이트:

```rust
SliceAxis::Side(y) => self.render_side(world, y),
```

**Step 4: Run all tests**

Run: `cd core && cargo test`
Expected: 모든 테스트 PASS

**Step 5: Commit**

```bash
git add core/src/render/ascii.rs
git commit -m "feat(render): implement side view ASCII rendering"
```
