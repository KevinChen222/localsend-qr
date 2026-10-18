#!/usr/bin/env bash
set -euo pipefail
target="${XDG_DATA_HOME:-$HOME/.local/share}/nautilus/scripts/通过二维码传送"
if [[ -f "$target" ]] && grep -Fq 'NAUTILUS_SCRIPT_SELECTED_FILE_PATHS' "$target" && grep -Fq 'share --pause-on-error' "$target"; then
  rm -- "$target"
  printf '%s\n' 'LocalSend QR Nautilus script removed.'
else
  printf '%s\n' 'No LocalSend QR script found; no files changed.'
fi
