#!/usr/bin/env bash
# Ubuntu 24 / GNOME Files (Nautilus). No sudo needed.
set -euo pipefail
root=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
core=${1:-"$root/localsend-qr"}
core=$(realpath -- "$core")
[[ -f "$core" && -x "$core" ]] || { printf '%s\n' 'Core executable not found or not executable' >&2; exit 1; }
command -v gnome-terminal >/dev/null || { printf '%s\n' 'gnome-terminal is required for the Nautilus integration' >&2; exit 1; }
scripts="${XDG_DATA_HOME:-$HOME/.local/share}/nautilus/scripts"
mkdir -p -- "$scripts"
# Remove the old entry only if it belongs to this tool.
legacy="$scripts/通过二维码传送"
if [[ -f "$legacy" ]] && grep -Fq 'share --pause-on-error' "$legacy"; then rm -- "$legacy"; fi
for mode in share receive; do
  if [[ "$mode" == share ]]; then label='由二维码发送'; else label='由二维码接收'; fi
  target="$scripts/$label"
  {
    printf '%s\n' '#!/usr/bin/env bash' '# LocalSend QR portable context menu' 'set -euo pipefail'
    printf 'core=%q\nmode=%q\n' "$core" "$mode"
    cat <<'SCRIPT'
items=()
while IFS= read -r item; do [[ -z "$item" ]] || items+=("$item"); done <<< "${NAUTILUS_SCRIPT_SELECTED_FILE_PATHS:-}"
# Nautilus starts scripts in the current folder for local file:// locations.
# Never silently receive in another directory when browsing a remote location.
if (( ${#items[@]} == 0 )); then
  case "${NAUTILUS_SCRIPT_CURRENT_URI:-file://}" in
    file:///*|file://localhost/*|file://) items+=("$PWD") ;;
    *) exec gnome-terminal -- bash -c 'echo "Select a local folder first."; read -r -p "Press Enter to close..."' ;;
  esac
fi
exec gnome-terminal -- "$core" "$mode" --pause-on-error -- "${items[@]}"
SCRIPT
  } > "$target"
  chmod 700 -- "$target"
done
printf 'Registered: %s\nRight-click → Scripts → 由二维码发送 / 由二维码接收\nRun again after moving the portable folder.\n' "$core"
