#!/usr/bin/env bash

assert_workspace_build_path() {
    local requested=$1 root=${workspaceRoot:?workspaceRoot must be set}
    local root_real requested_path full_path relative part cursor
    root_real=$(realpath -m -- "$root") || return
    if [[ $requested = /* ]]; then requested_path=$requested; else requested_path="$root/$requested"; fi
    full_path=$(realpath -m -- "$requested_path") || return
    if [[ $full_path != "$root_real"/* ]]; then
        printf 'Build output must stay inside the workspace: %s\n' "$full_path" >&2
        return 1
    fi
    if [[ -L $root ]]; then
        printf 'Build output cannot use a symbolic link workspace: %s\n' "$root" >&2
        return 1
    fi
    relative=${requested_path#"$root"/}
    cursor=$root
    IFS=/ read -r -a parts <<< "$relative"
    for part in "${parts[@]}"; do
        [[ -n $part && $part != . ]] || continue
        cursor="$cursor/$part"
        if [[ -L $cursor ]]; then
            printf 'Build output cannot use a symbolic link: %s\n' "$cursor" >&2
            return 1
        fi
    done
    printf '%s\n' "$full_path"
}
