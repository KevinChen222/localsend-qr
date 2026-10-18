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
target="$scripts/通过二维码传送"
# printf %q quotes paths as Bash literals, including spaces, quotes and newlines.
{
  printf '%s\n' '#!/usr/bin/env bash' 'set -euo pipefail'
  printf 'core=%q\n' "$core"
  cat <<'SCRIPT'
items=()
while IFS= read -r item; do [[ -z "$item" ]] || items+=("$item"); done <<< "${NAUTILUS_SCRIPT_SELECTED_FILE_PATHS:-}"
(( ${#items[@]} > 0 )) || exit 0
exec gnome-terminal -- "$core" share --pause-on-error -- "${items[@]}"
SCRIPT
} > "$target"
chmod 700 -- "$target"
printf 'Registered: %s\nRight-click → Scripts → 通过二维码传送\nRun again after moving the portable folder.\n' "$core"
