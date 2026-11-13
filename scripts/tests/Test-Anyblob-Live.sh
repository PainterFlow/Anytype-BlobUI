#!/usr/bin/env bash
set -Eeuo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
workspaceRoot=$(cd -- "$script_dir/../.." && pwd -P)
if (($# != 2)); then printf 'Usage: %s TEST-EXECUTABLE ANYTYPE-DIRECTORY\n' "$0" >&2; exit 2; fi
testPath=$(realpath -- "$1")
anytypeDirectory=$(realpath -- "$2")
case "$testPath" in
    "$workspaceRoot"/*) ;;
    *) printf '%s\n' 'Use a test executable built inside this workspace.' >&2; exit 1 ;;
esac
[[ -f $testPath ]] || { printf '%s\n' 'Build the Settings native tests before invoking live validation.' >&2; exit 1; }
[[ -f $anytypeDirectory/blob/data.json && -f $anytypeDirectory/blob/credentials/anytype-api-key.dpapi ]] || {
    printf '%s\n' 'The Anytype directory must contain blob/data.json and the Windows DPAPI credential file.' >&2
    exit 1
}
command -v wine >/dev/null 2>&1 && command -v winepath >/dev/null 2>&1 || { printf '%s\n' 'Wine and winepath are required to run Windows live validation from Linux.' >&2; exit 1; }

fixture="$workspaceRoot/.test-run/anyblob-live-content"
profile="$fixture/profile"
blob="$profile/anytype/blob"
credential="$blob/credentials/anytype-api-key.dpapi"
sourceCredential="$anytypeDirectory/blob/credentials/anytype-api-key.dpapi"
[[ $(realpath -m -- "$sourceCredential") != $(realpath -m -- "$credential") ]] || { printf '%s\n' 'The test credential cannot also be the source credential.' >&2; exit 1; }
umask 077
mkdir -p "$blob/credentials" "$profile/local" "$profile/temp"
cleanup() { rm -f -- "$credential"; }
trap cleanup EXIT
python3 - "$anytypeDirectory/blob/data.json" "$blob/data.json" <<'PY'
import json
import pathlib
import sys

source, destination = map(pathlib.Path, sys.argv[1:])
data = json.loads(source.read_text(encoding="utf-8"))
if "apiConnection" not in data:
    raise SystemExit("Anytype blob data does not contain apiConnection.")
pathlib.Path(destination).write_text(json.dumps({"apiConnection": data["apiConnection"]}, indent=2) + "\n", encoding="utf-8")
PY
cp -- "$sourceCredential" "$credential"

export APPDATA=$(winepath -w "$profile")
export LOCALAPPDATA=$(winepath -w "$profile/local")
export TEMP=$(winepath -w "$profile/temp")
export TMP=$TEMP
export BLOBUI_INSTALL_DIRECTORY=$(winepath -w "$profile/anytype")
export BLOBUI_ANYBLOB_LIVE_TEST=1
export WINEDEBUG=${WINEDEBUG:--all}
stdout="$fixture/stdout.txt"
stderr="$fixture/stderr.txt"
windows_test=$(winepath -w "$testPath")
wine "$windows_test" --ignored --exact anyblob::live_tests::codex_test_content_live --nocapture >"$stdout" 2>"$stderr" || {
    cat "$stdout" "$stderr"
    printf '%s\n' 'Codex live validation failed. Check the test ledger before retrying.' >&2
    exit 1
}
cat "$stdout" "$stderr"
