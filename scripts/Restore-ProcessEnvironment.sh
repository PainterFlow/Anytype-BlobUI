#!/usr/bin/env bash

blobui_save_environment() {
    local snapshot_name=$1 name
    shift
    declare -gA "$snapshot_name"
    local -n snapshot=$snapshot_name
    for name in "$@"; do
        if [[ ${!name+x} ]]; then
            snapshot["set:$name"]=1
            snapshot["value:$name"]=${!name}
        else
            snapshot["set:$name"]=0
            snapshot["value:$name"]=
        fi
    done
}

blobui_restore_environment() {
    local snapshot_name=$1 name
    shift
    local -n snapshot=$snapshot_name
    for name in "$@"; do
        if [[ ${snapshot[set:$name]:-0} == 1 ]]; then
            printf -v "$name" '%s' "${snapshot[value:$name]}"
            export "$name"
        else
            unset "$name"
        fi
    done
}
