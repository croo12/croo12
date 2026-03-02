# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

WebAssembly 웹 게임 프로젝트. React + Vite 프론트엔드에서 Canvas 기반 게임을 렌더링하며, `core/` 디렉토리의 네이티브 코드(C++/Rust/Go)를 `.wasm`으로 빌드하여 브라우저에서 실행한다. GitHub Pages로 정적 배포를 목표로 한다.

## Principles

- fix bug even if it already existed. 

## Commands

```bash
yarn dev          # 개발 서버 (localhost:3000, HMR)
yarn build        # tsc 타입체크 + vite 프로덕션 빌드 → dist/
yarn preview      # 빌드 결과물 로컬 프리뷰
yarn lint         # biome lint
yarn format       # biome format (auto-fix)
```

타입체크만 단독 실행: `npx tsc -b`

## Architecture

Feature-Sliced Design (FSD) 구조의 React SPA.

```
core/           # Wasm 소스 (C++/Rust/Go) — 프론트엔드 독립, 빌드 결과물은 core/build/
src/
  app/          # 진입점 (main.tsx), 전역 스타일, 라우터
  pages/        # 화면 단위 (메인, 인게임, 엔딩 등)
  widgets/      # 복합 UI 블록 (HUD, 리더보드 등)
  features/     # 사용자 행동 단위 (게임 시작, 점수 저장 등)
  entities/     # 도메인 모델 (Player, Enemy, GameState 등)
  shared/       # 공통 레이어
    ui/         # 범용 UI 컴포넌트
    wasm/       # .wasm 로더 wrapper
    assets/     # 이미지, 사운드, 3D 모델
    lib/        # 순수 유틸 (Math, Logging 등)
```

FSD 의존성 방향: `app → pages → widgets → features → entities → shared` (상위→하위만 허용).

## Simulation Scale

| 항목 | 값 | 비고 |
|------|-----|------|
| 1 블록 | 10m × 10m × 10m | 지형 시뮬레이션용 스케일 |
| 물리 시간 (1 tick) | ≈ 1.5초 | 자유낙하 10m ≈ 1.43s 기준 |
| 지질 시간 (1 tick) | ≈ 수개월 | 침식/퇴적/지하수 |
| 질량 1 단위 | ≈ 4,000L (4m³) | 블록 부피 1000m³ 기준 |

**듀얼 타임스케일 모델**: 유체 역학(중력 낙하, 수평 유동)은 물리 시간으로, 침식·퇴적·지하수 흡수는 지질 시간으로 동작한다. 두 시간 스케일이 동일 tick에서 실행되지만 서로 다른 현실 시간을 추상화한다(Game Abstraction).

## Code Style

- **Formatter/Linter:** Biome (tab indent, double quotes)
- **Path alias:** `@/*` → `src/*`
- **TypeScript:** strict mode, ESNext target, `react-jsx`
- **Module:** ESM (`"type": "module"`)
