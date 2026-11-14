#!/usr/bin/env bash
set -Eeuo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
workspace_root=$(cd -- "$script_dir/../.." && pwd -P)
installer="$workspace_root/scripts/Install-Remote-Skills.sh"
test_root=$(mktemp -d "${TMPDIR:-/tmp}/anyblob-remote-skills.XXXXXX")
trap 'rm -rf -- "$test_root"' EXIT

export ANYBLOB_SKILLS_HOME="$test_root/agents"
bash "$installer"

count=0
for source in "$workspace_root/codex-plugin/src/skills"/*; do
    name=${source##*/}
    installed="$ANYBLOB_SKILLS_HOME/skills/$name"
    [[ -f $installed/SKILL.md && -f $installed/.anyblob-managed-skill ]]
    cmp -s -- "$source/SKILL.md" "$installed/SKILL.md"
    count=$((count + 1))
done
[[ $count -gt 0 ]]
cmp -s -- "$workspace_root/codex-plugin/src/references/workflow-rules.md" \
    "$ANYBLOB_SKILLS_HOME/references/workflow-rules.md"

# Re-running the installer updates only directories that it created.
bash "$installer"

conflict_home="$test_root/conflict"
mkdir -p -- "$conflict_home/skills/search-and-answer"
printf 'keep this user skill\n' > "$conflict_home/skills/search-and-answer/SKILL.md"
if ANYBLOB_SKILLS_HOME="$conflict_home" bash "$installer"; then
    printf 'The installer replaced a user skill with the same name.\n' >&2
    exit 1
fi
[[ $(cat "$conflict_home/skills/search-and-answer/SKILL.md") == 'keep this user skill' ]]
[[ ! -e $conflict_home/skills/capture-and-organize ]]

printf 'Remote Anyblob skill installation checks passed.\n'
