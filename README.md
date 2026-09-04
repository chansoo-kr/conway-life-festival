# Conway Life Festival

축제 부스용 콘웨이의 생명 게임 앱 모음 (Rust · Bevy 0.19 · GPU 컴퓨트).
워크스페이스 안에 **공용 라이브러리 1개 + 목적별 패키지 4개**가 있습니다.
튜토리얼은 별도 앱(`tutorial`)이고, 세 모드 앱은 시작 팝업 → 플레이 → 처음으로 흐름만 갖습니다.

| 패키지 | 용도 | 실행 |
|---|---|---|
| `conway-core` | 공용 라이브러리 (GPU 시뮬레이션, 페인팅, 카메라, UI·시작 팝업·세션 초기화, 프리셋, 에셋) | — |
| `tutorial` | 생명 게임 규칙과 조작을 직접 해 보며 배우는 인터랙티브 튜토리얼 + 세 모드 소개 | `cargo run --release -p tutorial` |
| `free-mode` | 자유 모드 + 프리셋 체험 (16384×16384 격자 · 시계, 8/16비트 컴퓨터, 튜링 머신, 소수 계산기 등 초대형 프리셋 포함) | `cargo run --release -p free-mode` |
| `challenge` | **실시간으로 진화하는 격자** 위에서 한 시간마다 제시되는 모양 빠르게 만들기 | `cargo run --release -p challenge` |
| `battle` | 1 vs 1 대전 (Immigration 규칙, 색 다수결) | `cargo run --release -p battle` |
| `web` | 부스 웹(GitHub Pages): 구매·계좌 이체 안내 + 챌린지 리더보드 | `cd web && trunk serve` |

## 조작

### 공통
- **시작 팝업**: 실행하면 현재 앱의 설명이 뜹니다. '시작'(Enter 또는 Space)으로 닫습니다.
- 셀 그리기: **왼쪽 클릭/드래그**, 지우기: **오른쪽 클릭/드래그**
- **처음으로(세션 초기화)**: 각 흐름이 끝나면(튜토리얼 완료, 챌린지 결과 표시 후, 대전 결과 표시 후) 자동으로 시작 팝업과 초기 격자로 돌아가 다음 방문자를 맞습니다.
  우상단 '처음으로' 버튼으로 언제든 초기화할 수 있고, 입력이 없으면 자동 초기화됩니다 (기본 180초, `--idle-reset-sec S`, 0이면 끔).

### tutorial
- 시작 팝업을 닫으면 바로 튜토리얼이 시작됩니다. 화면 아래 안내 카드가 뜨고, 직접 클릭·조작해 각 단계의 목표를 달성하면 자동으로 다음 단계로 넘어갑니다.
- `1. 생명 게임 규칙`(생명 하나 → 사라짐 / 셋 나란히 → 깜빡이 / 2×2 네모 → 정물 / 낙서 → 붐비면 사라짐, 마지막에 규칙 정리) → `2. 조작 익히기`(지우기, 글라이더 스탬프, 재생, 속도, 줌) → `3. 축제의 세 가지 모드` 소개 순서, 약 3~4분.
  규칙을 먼저 외우게 하지 않고 "놓고 → 한 세대 흘리고 → 무슨 일이 생겼는지 보기"로 겪게 합니다. 모양 목표 단계는 카드 안에 목표 모양 미리보기가 뜨고, 판정은 GPU readback(실제 격자)으로 합니다.
- '건너뛰기'로 단계를 넘기고 '종료'로 끝낼 수 있습니다. 마지막 단계의 '마치기'나 '종료'를 누르면 시작 팝업으로 돌아갑니다.
- `Space` 재생/정지 · `N` 한 세대 · `[` `]` 속도 · `C` 지우기 · `F` 전체 보기 · `P` 펜 · `G` 글라이더 스탬프 · 휠 줌, `WASD`/가운데 버튼 드래그 이동
- 단계 정의는 [tutorial/src/main.rs](tutorial/src/main.rs)의 `steps()`, 엔진(목표 판정·카드 UI)은 [tutorial/src/engine.rs](tutorial/src/engine.rs)

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
- 판정은 GPU readback(실제 화면 상태)으로 매 프레임. 돌리거나 뒤집은 모양도 인정. 성공하면 그 순간의 격자를 고정하고 기록을 남깁니다. 라운드별 최고 기록 5개 표시
- 목표는 실시간 진화 중에도 만들 수 있는 작은 패턴(블록·벌집·빵·보트·튜브·깜빡이·두꺼비·비컨·글라이더·이터)에서 라운드마다 선택
- 도전마다 제한 시간(기본 60초)이 있습니다. 완성하면 기록을 보여 준 뒤, 시간이 다 되면 **최고 정확도(%)** 를 보여 준 뒤 시작 팝업으로 돌아갑니다. 진행 중에는 현재·최고 정확도가 표시됩니다
- 결과가 나오면 목표 미리보기 자리에 **결과 QR** 이 뜹니다(25초). 찍으면 웹에서 이름을 적고 리더보드에 올라갑니다
- `--interval-min N` 라운드 간격(분, 기본 60 — 웹 리더보드와 같아야 함) · `--rate R` 시뮬레이션 속도(세대/초, 기본 1) · `--limit-sec S` 제한 시간(초, 기본 60)
  예: `cargo run -p challenge -- --interval-min 1 --rate 2 --limit-sec 90`

### battle
- 배치 단계: 플레이어 1(왼쪽, 초록) → 플레이어 2(오른쪽, 주황), 각 60초 / 60셀 예산
- 무기고 숫자 키 `1~8`, `H`/`V` 좌우·상하 반전, `Enter` 준비 완료
- 전투: 600세대, `F` 빨리 감기. 결과 후 `R` 다시 시작
- 결과 카드에 **결과 QR** 이 같이 뜹니다(30초). 찍으면 웹에서 두 사람 이름을 적고 전투 리더보드에 올라갑니다

## 부스 웹 (web)

`main` 에 푸시하면 [.github/workflows/deploy.yml](.github/workflows/deploy.yml) 이 `web/dist` 를 GitHub Pages 로 올립니다.
루트 워크스페이스와 분리된 wasm 전용 크레이트라 `cargo build` 에는 딸려 오지 않습니다.

- `#/pay` 구매·계좌 이체 안내 — 가격과 계좌번호는 [web/src/config.rs](web/src/config.rs) 한 파일에 모여 있습니다.
- `#/board` 스피드런 리더보드 — 목표 모양이 한 시간마다 바뀌고, **바뀌는 순간 순위표가 비워집니다.**
- `#/battle` 1 대 1 전투 리더보드 — 이긴 쪽이 남긴 셀이 많은 경기가 위로.
- 챌린지·전투가 끝나면 화면에 **결과 QR** 이 뜹니다. 리더보드 화면의 `[QR 스캔]` 으로 찍으면
  이름을 적고 순위에 올라갑니다. 주소를 만드는 쪽은 [conway-core/src/qr.rs](conway-core/src/qr.rs),
  형식·서명과 운영 방식은 [web/README.md](web/README.md) 참고.

## 외부 웹 서비스 연동 (challenge)

[challenge/src/main.rs](challenge/src/main.rs)의 `ChallengeSource` 트레이트가 "지금 라운드의 목표"를 결정합니다.
현재는 시각 기반 결정적 선택(`LocalSource`)이며, 서버에서 라운드를 받아오는 구현체를 만들어
`main()`의 `Source(Box::new(...))` 한 줄만 바꾸면 됩니다. 기록 업로드는 `check_match`의 `Phase::Cleared` 전환 지점에 넣으면 됩니다.

## 구조

```
Cargo.toml            워크스페이스 (members: conway-core, free-mode, challenge, battle, tutorial)
conway-core/
  src/lib.rs          공용 코어 크레이트 (conway_core) + 에셋 경로/DefaultPlugins 헬퍼
  src/sim.rs          GPU 컴퓨트 플러그인: ping-pong 버퍼, 편집 적용 pass, readback, shader def로 그리드 크기 주입
  src/grid.rs         비트 패킹 그리드, CPU 미러, 편집 큐
  src/paint.rs        마우스 그리기/스탬프 → PaintRequest
  src/camera.rs       줌/팬, 커서 → 셀 변환
  src/ui.rs           한글 폰트 선택, 버튼/패널/카드, 시작 팝업, 세션 초기화(처음으로·무입력)
  src/presets.rs      내장 프리셋 + assets/patterns/*.rle
  src/qr.rs           결과 QR (주소 생성·서명·텍스처 굽기)
  src/rle.rs          RLE 파서/변환 (+ CPU 규칙 단위 테스트)
  src/debug.rs        스크린샷/셀프 테스트 (환경 변수로만 활성)
  assets/shaders/     conway_compute.wgsl (1인용) · conway_battle_compute.wgsl (2인용) · conway_edit.wgsl · conway_grid.wgsl
  assets/patterns/    clock_pm.rle 등
tutorial/src/main.rs  튜토리얼 앱 (격자·상단 바·단계 목록)
tutorial/src/engine.rs 튜토리얼 엔진 (StepGoal 판정, 단계 카드 오버레이)
free-mode/src/main.rs
challenge/src/main.rs
battle/src/main.rs
web/                  부스 웹 (Trunk + wasm, 루트 워크스페이스와 분리)
  index.html          Trunk 진입점
  style.css           터미널풍 스타일
  src/main.rs         라우팅·화면·DOM
  src/config.rs       가격·계좌·라운드 주기
  src/round.rs        라운드와 목표 모양 (challenge 의 LocalSource 와 동일 규칙)
  src/store.rs        리더보드 저장(localStorage, 스피드런·전투) + 결과 서명
  src/scan.rs         카메라 QR 인식 (rqrr)
```

## 폰트

한글 UI는 번들된 IBM Plex Sans KR(`conway-core/assets/fonts/`, OFL 라이선스)을 씁니다.
본문은 Regular, 제목·섹션 라벨은 SemiBold 입니다.
다른 폰트를 강제하려면 같은 폴더에 `ui.ttf`(+ 선택으로 `ui-bold.ttf`)를 넣으면 번들 폰트보다 우선합니다.
번들 폰트 파일이 없으면 시스템 폰트 패밀리(Pretendard, Noto Sans KR, 맑은 고딕 등)를 자동으로 찾습니다.

## 빌드 메모

- 배포 시 실행 파일 옆에 `assets/` 폴더(`conway-core/assets` 복사)를 함께 두면 됩니다. 개발 중에는 `conway-core/assets` 를 자동 사용합니다.
- 패키지 3개를 동시에 링크할 때 드물게 `can't find crate` / `invalid metadata` 오류가 나면
  `cargo build -j 1` 또는 `cargo build -p <name>` 으로 하나씩 빌드하세요.

## Batocera 부스 설치 (커스텀 게임)

네 앱을 Batocera 의 **Ports** 에 커스텀 게임으로 올리는 준비물은 [batocera/](batocera/) 에 있습니다 —
빌드/번들 스크립트([batocera/build.sh](batocera/build.sh)), 런처 스크립트, `gamelist.xml`, 패드→마우스 매핑.
설치 절차는 [batocera/README.md](batocera/README.md) 참고.

창 모드는 `--window <spec>` 또는 `CONWAY_WINDOW=<spec>` 으로 정합니다
(`borderless` · `fullscreen` · `windowed` · `1600x900`). 키오스크에서는 `borderless` 를 권장합니다.

## 검증 (디버그 환경 변수)

- `CONWAY_SELFTEST=1 cargo run -p free-mode` / `-p battle` : 패턴을 찍고 40세대 진행 후 GPU readback을 기대값과 비트 단위로 비교, 종료 코드 0/1
- `CONWAY_SCREENSHOT_DIR=<dir>` : 시작 팝업과 본 화면 PNG 저장 후 종료
- `cargo test -p conway-core` : RLE 파서 및 글라이더/LWSS 이동 방향 단위 테스트
