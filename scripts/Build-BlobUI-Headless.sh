#!/usr/bin/env bash
set -Eeuo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
workspaceRoot=$(cd -- "$script_dir/.." && pwd -P)
source "$script_dir/Workspace-Build-Paths.sh"

install_cli=0
close_cli=0
for arg in "$@"; do
    case "$arg" in
        -s|--start) install_cli=1 ;;
        -c|--close) close_cli=1 ;;
        -h|--help) printf 'Usage: %s [-c|--close] [-s|--start]\n-s installs blob-ui in ~/.local/bin. Codex starts the MCP process on demand.\n' "$0"; exit 0 ;;
        *) printf 'Unknown Build-BlobUI-Headless argument: %s\n' "$arg" >&2; exit 2 ;;
    esac
done

target_dir=$(assert_workspace_build_path "$workspaceRoot/settings-app/target")
binary=$(assert_workspace_build_path "$target_dir/release/blob-ui")
output=$(assert_workspace_build_path "$workspaceRoot/settings-app/dist/headless-cli/blob-ui")

if ((close_cli)); then
    while read -r pid; do
        [[ $pid =~ ^[0-9]+$ ]] || continue
        instance_path=$(readlink -e -- "/proc/$pid/exe" 2>/dev/null) || continue
        [[ ${instance_path##*/} == blob-ui ]] || continue
        assert_workspace_build_path "$instance_path" >/dev/null 2>&1 || continue
        kill -TERM "$pid"
        for ((i=0; i<50; i++)); do
            [[ $(readlink -e -- "/proc/$pid/exe" 2>/dev/null || true) == "$instance_path" ]] || break
            sleep 0.1
        done
        if ((i == 50)); then
            printf 'BlobUI Headless process %s did not exit.\n' "$pid" >&2
            exit 1
        fi
    done < <(pgrep -x blob-ui 2>/dev/null || true)
fi

source "$script_dir/Rust-Toolchain.sh"
export CARGO_TARGET_DIR="$target_dir"
cargo build --locked --release --no-default-features --features headless-cli --bin blob-ui --manifest-path "$workspaceRoot/settings-app/Cargo.toml"
install -D -m 755 -- "$binary" "$output"
printf 'Built BlobUI Headless at %s\n' "$output"

if ((install_cli)); then
    bin_directory="${HOME:?HOME is unavailable}/.local/bin"
    install -D -m 755 -- "$output" "$bin_directory/blob-ui"
    printf 'Installed blob-ui at %s\n' "$bin_directory/blob-ui"
    case ":${PATH:-}:" in
        *":$bin_directory:"*) printf '%s\n' 'Restart Codex to start its MCP process.' ;;
        *) printf '%s\n' 'Add export PATH="$HOME/.local/bin:$PATH" to your shell startup file, then restart Codex with the updated PATH.' ;;
    esac
fi
