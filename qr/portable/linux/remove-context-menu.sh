#!/usr/bin/env bash
set -euo pipefail
scripts="${XDG_DATA_HOME:-$HOME/.local/share}/nautilus/scripts"
for label in '通过二维码传送' '由二维码发送' '由二维码接收'; do
  target="$scripts/$label"
  if [[ -f "$target" ]] && grep -Fq 'NAUTILUS_SCRIPT_SELECTED_FILE_PATHS' "$target" && { grep -Fq 'LocalSend QR portable context menu' "$target" || grep -Fq 'share --pause-on-error' "$target"; }; then
    rm -- "$target"
    printf 'Removed: %s\n' "$label"
  fi
done
