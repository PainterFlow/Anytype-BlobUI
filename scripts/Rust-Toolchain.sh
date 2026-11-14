#!/usr/bin/env bash

if ! command -v cargo >/dev/null 2>&1; then
    toolchain_root="${workspaceRoot:?workspaceRoot must be set}/.test-run/toolchain"
    rust_root="$toolchain_root/rustup/toolchains/stable-x86_64-unknown-linux-gnu"
    rust_bin="$rust_root/bin"
    if [[ ! -x $rust_bin/cargo ]]; then
        printf '%s\n' 'Cargo was not found on PATH or in the workspace Linux toolchain. Install Rust or add its cargo bin folder to PATH.' >&2
        return 1 2>/dev/null || exit 1
    fi
    export CARGO_HOME="$toolchain_root/cargo"
    export RUSTUP_HOME="$toolchain_root/rustup"
    export TMPDIR="$toolchain_root/temp"
    mkdir -p "$TMPDIR"
    export PATH="$rust_bin:$PATH"
    printf '%s\n' 'Using the workspace Linux Rust toolchain.'
fi
