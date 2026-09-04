# Batocera 커스텀 게임(포트)으로 넣기

`tutorial` · `free-mode` · `challenge` · `battle` 네 바이너리를 Batocera 의 **Ports** 시스템에
커스텀 게임으로 등록하기 위한 준비물 모음입니다.

```
batocera/
  build.sh        릴리스 빌드 + 배포 번들(dist/) 생성
  ports/*.sh      EmulationStation 이 실행할 런처 스크립트 4개
  gamelist.xml    Ports 목록에 보일 이름·설명·이미지 (영문)
  gamelist.ko.xml 같은 내용의 한글판
  keys/*.keys     게임패드 → 마우스/키보드 매핑(padtokey) 예시
```

## 1) 번들 만들기 (PC에서)

```bash
./batocera/build.sh --docker          # 권장
./batocera/build.sh                   # 이 PC 툴체인 그대로
./batocera/build.sh --docker --screenshots   # gamelist 이미지까지
```

`--docker` 를 권장하는 이유: 이 PC 의 glibc(2.44)로 링크한 바이너리는 그보다 오래된 glibc 를 쓰는
Batocera 에서 `GLIBC_2.4x not found` 로 실행되지 않습니다. Debian bookworm(glibc 2.36) 컨테이너에서
빌드하면 대부분의 Batocera 버전에서 그대로 돕니다. 빌드 끝에 각 바이너리가 요구하는 최소 glibc 를
출력하니, 기기에서 `ldd --version` 결과와 비교하세요.

결과물 `batocera/dist/`:

```
dist/
  conway-life-festival/   tutorial free-mode challenge battle + assets/(셰이더·폰트·패턴)
  conway-*.sh             런처 4개
  conway-*.sh.keys        패드 매핑 4개
  gamelist.xml
  images/                 --screenshots 로 만든 경우
```

바이너리는 **자기 옆의 `assets/`** 를 읽으므로 `conway-life-festival/` 폴더 구조를 그대로 유지해야 합니다.

## 2) 기기에 복사

```bash
scp -r batocera/dist/* root@<batocera-ip>:/userdata/roms/ports/
ssh root@<batocera-ip> 'chmod +x /userdata/roms/ports/conway-*.sh'
```

(SMB 공유 `\\BATOCERA\ports` 에 복사해도 됩니다. 단 이때는 실행 권한이 빠질 수 있으니 `chmod +x` 를 꼭 실행하세요.)

## 3) 목록 갱신

기기에서:

```bash
batocera-es-swissknife --restart
```

또는 ES 메뉴 → `게임 설정` → `게임 목록 업데이트`. Ports 시스템에 네 항목이 뜹니다.
`gamelist.xml` 이 이미 있던 기기라면 새로 덮어쓰지 말고 `<game>` 블록만 합쳐 넣으세요.

**이름은 영문입니다.** EmulationStation 테마 폰트에 한글 글리프가 없으면 제목이 네모(두부)로 깨지기 때문입니다.
기기에서 한글이 잘 나온다면(ES 언어를 한국어로 두고 CJK 폰트가 있는 테마) `gamelist.ko.xml` 을
`gamelist.xml` 로 덮어쓰면 됩니다:

```bash
cp /userdata/roms/ports/gamelist.ko.xml /userdata/roms/ports/gamelist.xml
```

## 4) 설정 저장

```bash
batocera-save-overlay      # 필요 시 (userdata 는 원래 영구 저장이라 보통 불필요)
```

## 런처에서 조정할 수 있는 것

각 `conway-*.sh` 상단 환경 변수:

| 변수 | 기본값 | 설명 |
|---|---|---|
| `CONWAY_WINDOW` | `borderless` | `borderless`(테두리 없는 전체화면) · `fullscreen` · `windowed` · `1600x900` |
| `WGPU_BACKEND` | (주석 처리) | Vulkan 이 없는 기기면 `gl` 로 풀어 주세요 |
| `RUST_LOG` | `warn` | 문제 추적 시 `info` |

앱 인자도 그 줄에서 바꿉니다 — `--idle-reset-sec`(무입력 자동 초기화), challenge 의
`--interval-min` · `--rate` · `--limit-sec`, free-mode 의 `--preset` · `--zoom` 등.

## 문제가 생기면

로그는 `/userdata/system/logs/conway-<모드>.log` 에 남습니다.

- **목록에 안 보임** → `chmod +x`, 그리고 게임 목록 업데이트
- **실행하자마자 종료되고 로그 파일도 안 생김** → 런처 스크립트가 아예 실행되지 못한 것입니다.
  Windows 에서 복사했다면 줄바꿈이 CRLF 로 바뀌어 `#!/bin/bash` 줄이 깨진 경우가 대부분입니다
  (`head -c 12 /userdata/roms/ports/conway-tutorial.sh | od -c` 에 `\r` 이 보이면 확정).
  기기에서 `sed -i 's/\r$//' /userdata/roms/ports/conway-*.sh` 로 고치거나 `build.sh` 로 번들을
  다시 만드세요(`build.sh` 가 CR 을 걷어냅니다). `chmod +x` 누락도 같은 증상입니다
- **잠깐 검은 화면 후 ES 로 복귀** → 로그 확인. `GLIBC_ ... not found` 면 `--docker` 로 다시 빌드,
  `Failed to create graphics device` / `no suitable adapter` 면 `WGPU_BACKEND=gl`
- **창이 안 뜨거나 입력이 안 먹음** → Wayland/X11 문제. 바이너리는 둘 다 지원하도록 빌드되어 있고,
  런처가 `XDG_RUNTIME_DIR` 을 채워 줍니다. 그래도 안 되면 로그의 winit 줄을 보세요
- **한글이 깨짐** → 번들된 `assets/fonts/` 가 함께 복사됐는지 확인

## 입력 장치

네 앱 모두 **마우스로 그리는** 게임이라 부스에는 USB 마우스(또는 트랙볼)를 붙이는 게 가장 확실합니다.
패드만 있는 경우를 위해 `keys/*.keys`(Batocera padtokey) 매핑을 함께 넣어 뒀습니다 — 왼쪽 스틱=커서,
A=그리기, B=지우기, Start=재생/정지 등. padtokey 스키마는 Batocera 버전에 따라 조금씩 달라서,
기기에서 한 번 눌러 보고 안 맞으면 `/usr/share/evmapy/` 의 기본 파일을 참고해 손보세요.
`battle` 은 2인용이라 패드로 쓰려면 `actions_player2` 블록을 추가해야 합니다.

## 부스 운영 팁

- ES 메뉴 → `시스템 설정` → 부팅 시 특정 게임 바로 실행(kiosk)으로 튜토리얼을 자동 시작할 수 있습니다.
- 무입력 자동 초기화(`--idle-reset-sec`)가 켜져 있어 방문자가 떠나도 다음 사람이 처음 화면부터 시작합니다.
- ES 로 빠져나오는 건 패드 `HOTKEY + START`(또는 키보드 `F4`)입니다. 방문자가 못 나가게 하려면
  ES 의 핫키 비활성 옵션을 쓰세요.
