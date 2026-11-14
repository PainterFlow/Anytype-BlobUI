#!/usr/bin/env bash
set -Eeuo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
workspace_root=$(cd -- "$script_dir/.." && pwd -P)
source_skills="$workspace_root/codex-plugin/src/skills"
source_reference="$workspace_root/codex-plugin/src/references/workflow-rules.md"
agents_home=${ANYBLOB_SKILLS_HOME:-"${HOME:?HOME is not set}/.agents"}
skills_home="$agents_home/skills"
references_home="$agents_home/references"
reference_target="$references_home/workflow-rules.md"

[[ -d $source_skills && -f $source_reference ]] || {
    printf 'Anyblob skill sources are missing from %s.\n' "$workspace_root" >&2
    exit 1
}

for path in "$agents_home" "$skills_home" "$references_home" "$reference_target"; do
    [[ ! -L $path ]] || {
        printf 'Refusing to write through a symbolic link: %s\n' "$path" >&2
        exit 1
    }
done

skill_count=0
for source in "$source_skills"/*; do
    [[ -d $source && ! -L $source && -f $source/SKILL.md ]] || {
        printf 'Invalid Anyblob skill folder: %s\n' "$source" >&2
        exit 1
    }
    name=${source##*/}
    target="$skills_home/$name"
    [[ ! -L $target ]] || {
        printf 'Refusing to replace a symbolic link: %s\n' "$target" >&2
        exit 1
    }
    if [[ -e $target && ( ! -d $target || ! -f $target/.anyblob-managed-skill ) ]]; then
        printf 'A skill already uses this name. Move it before installing Anyblob: %s\n' "$target" >&2
        exit 1
    fi
    skill_count=$((skill_count + 1))
done

if [[ -e $reference_target ]] && ! cmp -s -- "$source_reference" "$reference_target"; then
    printf 'Refusing to replace a different reference file: %s\n' "$reference_target" >&2
    exit 1
fi
[[ $skill_count -gt 0 ]] || {
    printf 'No Anyblob skills were found.\n' >&2
    exit 1
}

mkdir -p -- "$skills_home" "$references_home"
for source in "$source_skills"/*; do
    name=${source##*/}
    target="$skills_home/$name"
    mkdir -p -- "$target"
    cp -R -- "$source/." "$target/"
    : > "$target/.anyblob-managed-skill"
done
cp -- "$source_reference" "$reference_target"

printf 'Installed %s Anyblob skills in %s. Start a new Codex session to load them.\n' \
    "$skill_count" "$skills_home"
