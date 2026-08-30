# Conway Life Festival

축제 부스용 콘웨이의 생명 게임 앱 모음 (Rust · Bevy 0.19 · GPU 컴퓨트).
워크스페이스 안에 **공용 라이브러리 1개 + 목적별 패키지 3개**가 있으며, 모든 앱에 **튜토리얼 버튼(T)** 이 있습니다.

| 패키지 | 용도 | 실행 |
|---|---|---|
| `conway-core` | 공용 라이브러리 (GPU 시뮬레이션, 페인팅, 카메라, UI/튜토리얼, 프리셋, 에셋) | — |
| `free-mode` | 자유 모드 + 프리셋 체험 (16384×16384 격자 · 시계, 8/16비트 컴퓨터, 튜링 머신, 소수 계산기 등 초대형 프리셋 포함) | `cargo run --release -p free-mode` |
| `challenge` | **실시간으로 진화하는 격자** 위에서 30분마다 제시되는 모양 빠르게 만들기 | `cargo run --release -p challenge` |
| `battle` | 1 vs 1 대전 (Immigration 규칙, 색 다수결) | `cargo run --release -p battle` |

## 조작

### 공통
- **튜토리얼**: 우상단 버튼 또는 `T` (Esc 닫기, ←/→ 페이지). 시작 시 자동으로 열립니다.
- 셀 그리기: **왼쪽 클릭/드래그**, 지우기: **오른쪽 클릭/드래그**

### free-mode
- `Space` 재생/정지 · `N` 한 세대 · `[` `]` 속도 (1~2048 세대/초) · `R` 보이는 영역 랜덤 · `C` 지우기 · `F` 전체 보기 · `P` 펜
- 휠 줌(커서 기준), `WASD`/방향키 또는 가운데 버튼 드래그로 이동
- 왼쪽 패널 프리셋 클릭 → 중앙 로드, "스탬프" → 클릭한 자리에 찍기
- `.rle` 파일을 창에 드롭하면 로드. `conway-core/assets/patterns/*.rle`, `*.mc`(Golly 매크로셀)는 시작 시 프리셋으로 자동 등록 (`#N 이름` 주석이 표시 이름, 출처는 `SOURCES.md`)
- 실행 옵션: `--preset <이름 일부>` 시작 프리셋 지정 · `--paused` 정지 상태로 시작 · `--zoom <셀 수>` 시작 화면 가로 셀 수
  예: `cargo run --release -p free-mode -- --preset 8비트 --paused --zoom 400`

### challenge
- `Space` 시작/다시 도전 · `C` 지우기 · `[` `]` 시뮬레이션 속도(난이도, 진행 요원용)
- '시작'과 동시에 **시뮬레이션이 계속 돌아갑니다** (기본 1 세대/초). 그린 셀도 규칙대로 태어나고 죽으므로
  진화를 피하거나 이용해 격자 위 살아있는 셀 전체가 목표와 같아지는 순간을 만들어야 합니다.
- 판정은 GPU readback(실제 화면 상태)으로 매 프레임. 성공하면 그 순간의 격자를 고정하고 기록을 남깁니다. 라운드별 최고 기록 5개 표시
- `--interval-min N` 라운드 간격(분, 기본 30) · `--rate R` 시뮬레이션 속도(세대/초, 기본 1)
  예: `cargo run -p challenge -- --interval-min 1 --rate 2`

### battle
- 배치 단계: 플레이어 1(왼쪽, 초록) → 플레이어 2(오른쪽, 주황), 각 60초 / 60셀 예산
- 무기고 숫자 키 `1~8`, `H`/`V` 좌우·상하 반전, `Enter` 준비 완료
- 전투: 600세대, `F` 빨리 감기. 결과 후 `R` 다시 시작

## 외부 웹 서비스 연동 (challenge)

[challenge/src/main.rs](challenge/src/main.rs)의 `ChallengeSource` 트레이트가 "지금 라운드의 목표"를 결정합니다.
현재는 시각 기반 결정적 선택(`LocalSource`)이며, 서버에서 라운드를 받아오는 구현체를 만들어
`main()`의 `Source(Box::new(...))` 한 줄만 바꾸면 됩니다. 기록 업로드는 `check_match`의 `Phase::Cleared` 전환 지점에 넣으면 됩니다.

## 구조

```
Cargo.toml            워크스페이스 (members: conway-core, free-mode, challenge, battle)
conway-core/
  src/lib.rs          공용 코어 크레이트 (conway_core) + 에셋 경로/DefaultPlugins 헬퍼
  src/sim.rs          GPU 컴퓨트 플러그인: ping-pong 버퍼, 편집 적용 pass, readback, shader def로 그리드 크기 주입
  src/grid.rs         비트 패킹 그리드, CPU 미러, 편집 큐
  src/paint.rs        마우스 그리기/스탬프 → PaintRequest
  src/camera.rs       줌/팬, 커서 → 셀 변환
  src/ui.rs           한글 폰트 선택, 버튼/패널, 튜토리얼 오버레이
  src/presets.rs      내장 프리셋 + assets/patterns/*.rle
  src/rle.rs          RLE 파서/변환 (+ CPU 규칙 단위 테스트)
  src/debug.rs        스크린샷/셀프 테스트 (환경 변수로만 활성)
  assets/shaders/     conway_compute.wgsl (1인용) · conway_battle_compute.wgsl (2인용) · conway_edit.wgsl · conway_grid.wgsl
  assets/patterns/    clock_pm.rle 등
free-mode/src/main.rs
challenge/src/main.rs
battle/src/main.rs
```

## 폰트

한글 UI는 시스템 폰트 패밀리(Noto Sans KR, 맑은 고딕, Pretendard 등)를 자동으로 찾아 씁니다.
특정 폰트를 강제하려면 `conway-core/assets/fonts/ui.ttf` 를 넣으면 됩니다.

## 빌드 메모

- 배포 시 실행 파일 옆에 `assets/` 폴더(`conway-core/assets` 복사)를 함께 두면 됩니다. 개발 중에는 `conway-core/assets` 를 자동 사용합니다.
- 패키지 3개를 동시에 링크할 때 드물게 `can't find crate` / `invalid metadata` 오류가 나면
  `cargo build -j 1` 또는 `cargo build -p <name>` 으로 하나씩 빌드하세요.

## 검증 (디버그 환경 변수)

- `CONWAY_SELFTEST=1 cargo run -p free-mode` / `-p battle` : 패턴을 찍고 40세대 진행 후 GPU readback을 기대값과 비트 단위로 비교, 종료 코드 0/1
- `CONWAY_SCREENSHOT_DIR=<dir>` : 튜토리얼 화면과 본 화면 PNG 저장 후 종료
- `cargo test -p conway-core` : RLE 파서 및 글라이더/LWSS 이동 방향 단위 테스트
