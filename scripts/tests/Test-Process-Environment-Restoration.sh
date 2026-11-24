#!/usr/bin/env bash
set -Eeuo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
source "$script_dir/../Restore-ProcessEnvironment.sh"
suffix="${BASHPID}_${RANDOM}"
unset_name="BLOBUI_RESTORE_UNSET_$suffix"
empty_name="BLOBUI_RESTORE_EMPTY_$suffix"
value_name="BLOBUI_RESTORE_VALUE_$suffix"
names=("$unset_name" "$empty_name" "$value_name")
blobui_save_environment original_environment "${names[@]}"
trap 'blobui_restore_environment original_environment "${names[@]}"' EXIT

unset "$unset_name"
printf -v "$empty_name" '%s' ''
export "$empty_name"
printf -v "$value_name" '%s' before-build
export "$value_name"
blobui_save_environment saved_environment "${names[@]}"
for name in "${names[@]}"; do printf -v "$name" '%s' during-build; export "$name"; done
blobui_restore_environment saved_environment "${names[@]}"

[[ ! ${!unset_name+x} ]] || { printf '%s\n' 'An absent environment variable was restored as present.' >&2; exit 1; }
[[ ${!empty_name+x} && -z ${!empty_name} ]] || { printf '%s\n' 'An empty environment variable was not restored as empty.' >&2; exit 1; }
[[ ${!value_name} == before-build ]] || { printf '%s\n' 'A non-empty environment variable was not restored.' >&2; exit 1; }
printf '%s\n' 'Process environment restoration checks passed.'
