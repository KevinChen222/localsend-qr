#!/usr/bin/env bash
set -euo pipefail
root=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
if (( $# == 0 )); then printf '%s\n' 'Usage: ./start-share.sh FILE_OR_FOLDER [MORE_ITEMS...]' >&2; exit 2; fi
exec "$root/localsend-qr" share -- "$@"
