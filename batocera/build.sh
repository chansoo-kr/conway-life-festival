#!/usr/bin/env bash
# Batocera 배포 번들 생성기.
#
#   ./batocera/build.sh                 # 이 PC의 툴체인으로 빌드
#   ./batocera/build.sh --docker        # 오래된 glibc(Debian bookworm) 컨테이너로 빌드 (권장)
#   ./batocera/build.sh --screenshots   # gamelist 이미지용 스크린샷도 생성 (GPU/디스플레이 필요)
#
# 결과: batocera/dist/  →  통째로 Batocera 의 /userdata/roms/ports/ 에 복사.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST="$ROOT/batocera/dist"
APPDIR="$DIST/conway-life-festival"
BINS=(tutorial free-mode challenge battle)
USE_DOCKER=0
SHOTS=0
for a in "$@"; do
  case "$a" in
    --docker) USE_DOCKER=1 ;;
    --screenshots) SHOTS=1 ;;
    *) echo "unknown option: $a" >&2; exit 2 ;;
  esac
done

echo "==> release 빌드"
if [[ $USE_DOCKER == 1 ]]; then
  # Batocera 의 glibc 는 이 PC 보다 오래된 경우가 많습니다.
  # 낮은 glibc 위에서 빌드해야 대상 기기에서 GLIBC_x.yz not found 가 안 납니다.
  docker run --rm -t \
    -v "$ROOT":/src -w /src \
    -v "$ROOT/batocera/.docker-target":/src/target \
    -v "$ROOT/batocera/.docker-cargo":/usr/local/cargo/registry \
    rust:1-bookworm \
    bash -c 'apt-get update -qq && apt-get install -y -qq --no-install-recommends \
        pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev \
        libx11-dev libxcursor-dev libxrandr-dev libxi-dev >/dev/null && \
      cargo build --release -p tutorial -p free-mode -p challenge -p battle'
  TARGET_DIR="$ROOT/batocera/.docker-target/release"
else
  ( cd "$ROOT" && cargo build --release -p tutorial -p free-mode -p challenge -p battle )
  TARGET_DIR="$ROOT/target/release"
fi

echo "==> 번들 구성: $DIST"
rm -rf "$DIST"
mkdir -p "$APPDIR" "$DIST/images"
for b in "${BINS[@]}"; do
  install -m 755 "$TARGET_DIR/$b" "$APPDIR/$b"
  strip "$APPDIR/$b" 2>/dev/null || true
done
cp -r "$ROOT/conway-core/assets" "$APPDIR/assets"   # 실행 파일 옆의 assets/ 를 자동으로 씁니다
cp "$ROOT/batocera/ports/"*.sh "$DIST/"
cp "$ROOT/batocera/gamelist.xml" "$DIST/gamelist.xml"
cp "$ROOT/batocera/gamelist.ko.xml" "$DIST/gamelist.ko.xml"   # 한글 테마용 대체본
for k in "$ROOT/batocera/keys/"*.keys; do
  [[ -e "$k" ]] && cp "$k" "$DIST/$(basename "$k")"
done
chmod +x "$DIST"/*.sh

if [[ $SHOTS == 1 ]]; then
  echo "==> 스크린샷 생성"
  for b in "${BINS[@]}"; do
    mkdir -p "$DIST/images/$b"   # 앱은 폴더를 만들어 주지 않습니다
    ( cd "$APPDIR" && CONWAY_WINDOW=1600x900 CONWAY_SCREENSHOT_DIR="$DIST/images/$b" "./$b" ) \
      || echo "  ($b 스크린샷 실패, 건너뜀)"
    # gamelist.xml 이 가리키는 images/<모드>.png 로 대표 한 장을 올립니다
    shot="$(ls "$DIST/images/$b/"*.png 2>/dev/null | tail -1 || true)"
    [[ -n "$shot" ]] && cp "$shot" "$DIST/images/$b.png"
  done
fi

echo "==> 대상 기기 호환성 확인 (필요한 최소 glibc)"
for b in "${BINS[@]}"; do
  printf '  %-10s glibc %s\n' "$b" \
    "$(objdump -T "$APPDIR/$b" 2>/dev/null | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -1)"
done

cat <<MSG

완료. 이제 이 폴더를 Batocera 로 복사하세요:

  scp -r "$DIST"/* root@<batocera-ip>:/userdata/roms/ports/

그다음 기기에서:
  chmod +x /userdata/roms/ports/conway-*.sh
  batocera-es-swissknife --restart
MSG
