# web — 부스 웹 (GitHub Pages)

Trunk + wasm 로 빌드하는 정적 페이지입니다. `main` 에 푸시하면
[.github/workflows/deploy.yml](../.github/workflows/deploy.yml) 이 `web/dist` 를 GitHub Pages 로 올립니다.

```
trunk serve            # 로컬 확인 (http://127.0.0.1:8080)
trunk build --release  # dist/ 생성 — CI 가 하는 것과 같음
```

## 화면

| 주소 | 내용 |
|---|---|
| `#/` | 부스 안내, 모드 소개, 규칙 |
| `#/pay` | 구매 · 계좌 이체 안내 (은행/예금주/계좌번호, 키캡·체험·USB 가격표) |
| `#/board` | 스피드런 리더보드 — 이번 라운드 목표 모양, 남은 시간, 순위, QR 스캔 |
| `#/battle` | 1 대 1 전투 리더보드 — 경기 순위, QR 스캔 |
| `#/r?...` | 스피드런 결과 QR이 가리키는 주소 — 이름을 적고 등록 |
| `#/b?...` | 전투 결과 QR이 가리키는 주소 — 두 사람 이름을 적고 등록 |
| `#/dev` | 진행 요원 확인용 — 앱 없이 등록 흐름을 확인할 샘플 주소 |

## 고칠 곳은 한 파일

가격, 계좌번호, 예금주, 안내 문구, 라운드 주기는 전부
[src/config.rs](src/config.rs) 에 있습니다. 바뀌면 그 파일만 고치고 푸시하면 됩니다.
가격이 `None` 이면 화면에 '가격 미정' 으로 나옵니다.

## 동작 흐름

1. 챌린지/전투가 끝나면 앱이 결과 값을 주소에 담아 **QR 로 띄웁니다**
   (`conway-core/src/qr.rs`, 챌린지 25초 · 전투 30초 동안 표시).
2. 그 QR 을 찍으면 이 웹의 `#/r` 또는 `#/b` 가 열립니다.
3. 값이 서명과 맞으면 결과를 보여 주고, **이름을 적어 리더보드에 등록**합니다.

## 라운드(목표 모양)

`round = unix초 / ROUND_INTERVAL_SECS`, 모양은 `POOL[splitmix64(round) % POOL.len()]`.
[challenge](../challenge) 의 `LocalSource` 와 같은 계산이고 기본 주기도 양쪽 다 60분이라 그대로 맞습니다.
(`--interval-min` 으로 챌린지 주기를 바꾸면 `ROUND_INTERVAL_SECS` 도 같이 바꿔야 QR 안의 라운드 번호가 맞습니다.)
`POOL` 의 순서는 `challenge/src/main.rs` 의 `TARGET_POOL` 순서와 같아야 합니다.

스피드런 리더보드는 저장할 때 라운드 번호를 같이 적어 두고, 읽을 때 지금 라운드와 다르면 통째로 버립니다.
**모양이 바뀌는 순간 순위표가 비워집니다.** 화면은 1초마다 남은 시간을 갱신하고, 넘어가면 스스로 새로 그립니다.
지난 라운드의 QR 을 찍으면 결과는 보여 주되 등록은 막습니다.

전투 리더보드는 라운드가 없어서 `[기록 지우기]` 를 누를 때까지 쌓입니다.

## 기록은 어디에 쌓이나

GitHub Pages 는 서버가 없는 정적 호스팅이라, 기록은 **리더보드를 띄운 그 기기의** `localStorage` 에 쌓입니다.
그래서 부스 운영은 이렇게 하는 걸 전제로 합니다.

1. 부스 화면(태블릿/노트북) 한 대에 `#/board` 또는 `#/battle` 을 띄워 둔다.
2. 참가자가 끝내면 게임 화면에 결과 QR 이 뜬다.
3. 부스 화면에서 `[QR 스캔]` 을 눌러 그 QR 을 찍는다 → 이름을 적고 등록 → 순위표에 올라간다.

참가자가 자기 휴대폰으로 찍어도 결과 화면은 정상적으로 뜨지만, 그 기록은 그 휴대폰에만 남습니다.
여러 기기가 순위를 같이 보게 하려면 서버가 필요합니다. 그때는 [src/store.rs](src/store.rs) 의
`load_*` / `add_*` / `clear_*` 함수만 원격 호출로 바꾸면 나머지는 그대로 씁니다.

## 결과 QR 주소 형식

주소는 [conway-core/src/qr.rs](../conway-core/src/qr.rs) 가 만듭니다.
기본 주소는 `WEB_BASE_URL` 이고, `--web-url <주소>` 인자나 `CONWAY_WEB_URL` 환경 변수로 바꿀 수 있습니다.

```
<웹 주소>/#/r?r=<라운드>&c=<0|1>&a=<정확도×10000>&t=<밀리초>&k=<서명>
<웹 주소>/#/b?w=<0 무승부|1 P1|2 P2>&x=<P1 셀>&y=<P2 셀>&g=<세대>&k=<서명>
```

`k` 는 그 앞까지의 질의문자열에 대한 서명입니다(암호가 아니라 오타·장난 방지).
`SUBMIT_SECRET` 이 양쪽에서 같아야 하고, 계산은 `conway-core` 의 `sign_query` 와
[src/store.rs](src/store.rs) 의 `signature` 가 같은 함수입니다 — FNV-1a 64 해시의 아래 30비트를 36진수 6자리로.

## 구조

```
index.html      Trunk 진입점 (헤더/내비게이션 껍데기)
style.css       터미널풍 단색 스타일
src/main.rs     라우팅 + 화면 + DOM
src/config.rs   가격·계좌·주기 (여기만 고치면 됨)
src/round.rs    라운드 계산과 목표 모양 목록
src/store.rs    리더보드 저장 (localStorage) + 결과 서명
src/scan.rs     카메라로 QR 읽기 (rqrr)
```
