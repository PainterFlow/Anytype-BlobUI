#!/usr/bin/env bash
set -Eeuo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
workspaceRoot=$(cd -- "$script_dir/../.." && pwd -P)
skip_build=0
while (($#)); do
    case "$1" in
        -SkipBuild|--skip-build) skip_build=1 ;;
        -h|--help) printf 'Usage: %s [--skip-build]\n' "$0"; exit 0 ;;
        *) printf 'Unknown Test-Anyblob argument: %s\n' "$1" >&2; exit 2 ;;
    esac
    shift
done

fixture="$workspaceRoot/.test-run/anyblob-validation"
navigationFixture="$workspaceRoot/.test-run/navigation"
mkdir -p "$fixture" "$navigationFixture"
original_home=${HOME:-}
if [[ -n $original_home ]]; then
    export CARGO_HOME=${CARGO_HOME:-"$original_home/.cargo"}
    export RUSTUP_HOME=${RUSTUP_HOME:-"$original_home/.rustup"}
fi
export HOME="$fixture/home"
export APPDATA="$fixture/roaming"
export LOCALAPPDATA="$fixture/local"
export XDG_CONFIG_HOME="$fixture/config"
export XDG_DATA_HOME="$fixture/data"
export TMPDIR="$fixture/temp"
export TMP="$TMPDIR"
export TEMP="$TMPDIR"
export ANYBLOB_BRIDGE_INSTANCE="fixture-$$-$RANDOM"
export ANYBLOB_CREDENTIAL_FILE="$fixture/bridge.credential"
export CARGO_NET_OFFLINE=true
mkdir -p "$HOME" "$APPDATA" "$LOCALAPPDATA" "$XDG_CONFIG_HOME" "$XDG_DATA_HOME" "$TMPDIR"

"$script_dir/../Build-MCP.sh" --test
(
    cd "$workspaceRoot"
    CARGO_TARGET_DIR="$workspaceRoot/mcp-server/target" cargo run --locked --release --quiet --manifest-path "$workspaceRoot/mcp-server/Cargo.toml" --example source-fixtures > "$navigationFixture/source-fixtures.json"
)
if ((!skip_build)); then "$script_dir/../Build-Codex.sh" --skip-mcp-build; fi
(
    cd "$workspaceRoot"
    CARGO_TARGET_DIR="$workspaceRoot/settings-app/target" cargo test --locked --release --manifest-path "$workspaceRoot/settings-app/Cargo.toml" -- --test-threads=1 anyblob:: anyblob_content:: anyblob_creation:: anyblob_scope:: marketplace:: codex:: codex_identity:: codex_projects::
)
