#!/usr/bin/env bash
set -Eeuo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
workspaceRoot=$(cd -- "$script_dir/.." && pwd -P)
source "$script_dir/Workspace-Build-Paths.sh"

if (($#)); then
    printf 'Unknown Build-Codex argument: %s\n' "$1" >&2
    exit 2
fi

plugin="$workspaceRoot/codex-plugin/src"
archive="$workspaceRoot/codex-plugin/dist/anyblob.zip"
assert_workspace_build_path "$archive" >/dev/null
node "$workspaceRoot/codex-plugin/tests/skills.cjs" "$plugin"
node "$workspaceRoot/codex-plugin/tests/validate.cjs" "$plugin"
mkdir -p -- "$(dirname -- "$archive")"

temporary="${archive%.zip}-$$.zip"
trap 'rm -f -- "$temporary"' EXIT
(cd "$plugin" && zip -q -r -X "$temporary" .)
mv -f -- "$temporary" "$archive"
printf 'Packaged Anyblob at %s\n' "$archive"
