#!/usr/bin/env bash
set -Eeuo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
workspaceRoot=$(cd -- "$script_dir/.." && pwd -P)
source "$script_dir/Restore-ProcessEnvironment.sh"
source "$script_dir/Workspace-Build-Paths.sh"

run_tests=0
while (($#)); do
    case "$1" in
        -t|-Test|--test) run_tests=1 ;;
        -h|--help) printf 'Usage: %s [-t|--test]\n' "$0"; exit 0 ;;
        *) printf 'Unknown Build-MCP argument: %s\n' "$1" >&2; exit 2 ;;
    esac
    shift
done

manifest="$workspaceRoot/mcp-server/Cargo.toml"
[[ -f $manifest ]] || { printf 'MCP manifest not found: %s\n' "$manifest" >&2; exit 1; }
command -v cargo >/dev/null 2>&1 || { printf '%s\n' 'Cargo is required to build the MCP server.' >&2; exit 1; }
command -v rustc >/dev/null 2>&1 || { printf '%s\n' 'rustc is required to identify the MCP build target.' >&2; exit 1; }
target=${BLOBUI_BUILD_TARGET:-$(rustc -vV | sed -n 's/^host: //p')}
[[ -n $target ]] || { printf '%s\n' 'Could not identify the Rust host target.' >&2; exit 1; }
assert_workspace_build_path "$workspaceRoot/mcp-server/target" >/dev/null
assert_workspace_build_path "$workspaceRoot/mcp-server/dist/blobui-mcp.exe" >/dev/null
blobui_save_environment BUILD_MCP_ENV PATH CARGO_HOME RUSTUP_HOME TMPDIR CARGO_TARGET_DIR RUSTFLAGS CARGO_ENCODED_RUSTFLAGS
trap 'blobui_restore_environment BUILD_MCP_ENV PATH CARGO_HOME RUSTUP_HOME TMPDIR CARGO_TARGET_DIR RUSTFLAGS CARGO_ENCODED_RUSTFLAGS' EXIT
source "$script_dir/Rust-Toolchain.sh"
export CARGO_TARGET_DIR="$workspaceRoot/mcp-server/target"
if ((run_tests)); then cargo test --locked --release --manifest-path "$manifest"; fi
cargo build --locked --release --target "$target" --manifest-path "$manifest"

source_binary="$CARGO_TARGET_DIR/$target/release/blobui-mcp"
[[ -f $source_binary ]] || { printf 'Cargo did not create the MCP executable: %s\n' "$source_binary" >&2; exit 1; }
dist="$workspaceRoot/mcp-server/dist"
mkdir -p "$dist"
install -m 755 "$source_binary" "$dist/blobui-mcp.exe"
printf 'Built %s\n' "$dist/blobui-mcp.exe"
