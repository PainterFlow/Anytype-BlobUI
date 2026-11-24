#!/usr/bin/env bash
set -Eeuo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
workspaceRoot=$(cd -- "$script_dir/../.." && pwd -P)
skip_build=0
while (($#)); do
    case "$1" in
        -SkipBuild|--skip-build) skip_build=1 ;;
        -h|--help) printf 'Usage: %s [--skip-build]\n' "$0"; exit 0 ;;
        *) printf 'Unknown Test-Installer argument: %s\n' "$1" >&2; exit 2 ;;
    esac
    shift
done
if ((!skip_build)); then
    "$script_dir/../Build-Installer.sh" --development
    "$script_dir/../Build-Installer.sh"
fi
fixture="$workspaceRoot/.test-run/installer-validation"
mkdir -p "$fixture/home" "$fixture/config" "$fixture/data" "$fixture/temp"
original_home=${HOME:-}
if [[ -n $original_home ]]; then
    export CARGO_HOME=${CARGO_HOME:-"$original_home/.cargo"}
    export RUSTUP_HOME=${RUSTUP_HOME:-"$original_home/.rustup"}
fi
export HOME="$fixture/home"
export XDG_CONFIG_HOME="$fixture/config"
export XDG_DATA_HOME="$fixture/data"
export TMPDIR="$fixture/temp"
export CARGO_TARGET_DIR="$workspaceRoot/installer/target"
cargo test --locked --release --manifest-path "$workspaceRoot/installer/Cargo.toml"
