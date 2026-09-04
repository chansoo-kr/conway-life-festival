#!/bin/bash
# Conway Life Festival — 자유 모드 (Batocera 포트 런처)
# 이 파일과 conway-life-festival/ 폴더를 /userdata/roms/ports/ 에 함께 둡니다.
set -u

APP="$(cd "$(dirname "$0")" && pwd)/conway-life-festival"
LOG=/userdata/system/logs/conway-free-mode.log

# 창 모드: borderless(테두리 없는 전체화면) 권장. windowed / fullscreen / 1600x900 도 가능.
export CONWAY_WINDOW="${CONWAY_WINDOW:-borderless}"
# Vulkan 이 없는 기기에서는 아래 줄의 주석을 풀어 OpenGL 로 폴백하세요.
#export WGPU_BACKEND=gl
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/var/run}"
export RUST_LOG="${RUST_LOG:-warn}"

mkdir -p "$(dirname "$LOG")"
exec >"$LOG" 2>&1          # 이 아래의 모든 출력(실패 원인 포함)은 $LOG 로 갑니다
cd "$APP" || { echo "no app dir: $APP"; exit 1; }
[[ -x ./free-mode ]] || { echo "not executable: $APP/free-mode (chmod +x 필요)"; exit 1; }
exec ./free-mode --idle-reset-sec 180  "$@"
