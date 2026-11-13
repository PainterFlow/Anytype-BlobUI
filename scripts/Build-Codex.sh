#!/usr/bin/env bash
set -Eeuo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
workspaceRoot=$(cd -- "$script_dir/.." && pwd -P)
source "$script_dir/Workspace-Build-Paths.sh"
skip_mcp_build=0
while (($#)); do
    case "$1" in
        -SkipMcpBuild|--skip-mcp-build) skip_mcp_build=1 ;;
        -h|--help) printf 'Usage: %s [--skip-mcp-build]\n' "$0"; exit 0 ;;
        *) printf 'Unknown Build-Codex argument: %s\n' "$1" >&2; exit 2 ;;
    esac
    shift
done

dist="$workspaceRoot/codex-plugin/dist"
package="$dist/anyblob"
zip_path="$dist/anyblob.zip"
payload_path="$dist/marketplace-payload.json"
assert_workspace_build_path "$package" >/dev/null
assert_workspace_build_path "$zip_path" >/dev/null
assert_workspace_build_path "$payload_path" >/dev/null
if ((!skip_mcp_build)); then "$script_dir/Build-MCP.sh"; fi
[[ -f $workspaceRoot/mcp-server/dist/blobui-mcp.exe ]] || { printf '%s\n' 'Build the MCP server before packaging the Codex plugin.' >&2; exit 1; }
command -v node >/dev/null 2>&1 || { printf '%s\n' 'Node.js is required to validate the Codex plugin.' >&2; exit 1; }
command -v python3 >/dev/null 2>&1 || { printf '%s\n' 'Python 3 is required to package the Codex plugin.' >&2; exit 1; }
command -v zip >/dev/null 2>&1 || { printf '%s\n' 'The zip command is required to package the Codex plugin.' >&2; exit 1; }

if [[ -e $package || -L $package ]]; then
    [[ ! -L $package ]] || { printf 'Refusing to replace a symbolic link: %s\n' "$package" >&2; exit 1; }
    rm -rf -- "$package"
fi
mkdir -p "$package/bin" "$package/assets"
cp -a "$workspaceRoot/codex-plugin/src/." "$package/"
install -m 755 "$workspaceRoot/mcp-server/dist/blobui-mcp.exe" "$package/bin/blobui-mcp.exe"
python3 - "$workspaceRoot/settings-app/icons/icon.ico" "$package/assets/icon.png" <<'PY'
import pathlib
import struct
import sys

source, destination = map(pathlib.Path, sys.argv[1:])
data = source.read_bytes()
if len(data) < 6 or data[:4] != b"\0\0\1\0":
    raise SystemExit("Invalid app icon file.")
count = struct.unpack_from("<H", data, 4)[0]
frames = []
for index in range(count):
    entry = 6 + 16 * index
    if entry + 16 > len(data):
        raise SystemExit("Invalid app icon directory.")
    width, height = data[entry], data[entry + 1]
    size, offset = struct.unpack_from("<II", data, entry + 8)
    if offset + size > len(data):
        raise SystemExit("Invalid app icon frame.")
    frame = data[offset:offset + size]
    if frame.startswith(b"\x89PNG\r\n\x1a\n"):
        frames.append((max(width or 256, 1) * max(height or 256, 1), frame))
if not frames:
    raise SystemExit("The existing app icon has no PNG frame.")
destination.write_bytes(max(frames, key=lambda item: item[0])[1])
PY

node "$workspaceRoot/codex-plugin/tests/skills.cjs" "$package"
node "$workspaceRoot/codex-plugin/tests/validate.cjs" "$package"
expected=(
    plugin.json mcp.json .codex-plugin/plugin.json .mcp.json README.md
    assets/icon.png bin/blobui-mcp.exe references/workflow-rules.md
    skills/search-and-answer/SKILL.md skills/search-and-answer/agents/openai.yaml
    skills/capture-and-organize/SKILL.md skills/capture-and-organize/agents/openai.yaml
    skills/maintain-pages/SKILL.md skills/maintain-pages/agents/openai.yaml
    skills/plan-projects/SKILL.md skills/plan-projects/agents/openai.yaml
    skills/review-project-progress/SKILL.md skills/review-project-progress/agents/openai.yaml
    skills/process-meeting-notes/SKILL.md skills/process-meeting-notes/agents/openai.yaml
    skills/manage-types-and-templates/SKILL.md skills/manage-types-and-templates/agents/openai.yaml
    skills/build-queries-and-views/SKILL.md skills/build-queries-and-views/agents/openai.yaml
    skills/configure-kanban-calculations/SKILL.md skills/configure-kanban-calculations/agents/openai.yaml
    skills/configure-calendar-colors/SKILL.md skills/configure-calendar-colors/agents/openai.yaml
    skills/audit-organization/SKILL.md skills/audit-organization/agents/openai.yaml
)
tmp_zip="$dist/.anyblob-$$.zip"
trap 'rm -f -- "$tmp_zip"' EXIT
(cd "$package" && zip -q -r -X "$tmp_zip" .)
python3 - "$tmp_zip" "${expected[@]}" <<'PY'
import sys
import zipfile

archive, *expected = sys.argv[1:]
with zipfile.ZipFile(archive) as package:
    actual = sorted(name for name in package.namelist() if not name.endswith("/"))
if actual != sorted(expected):
    raise SystemExit("The ZIP does not contain the exact validated plugin files.")
PY
mv -f -- "$tmp_zip" "$zip_path"
python3 - "$workspaceRoot/codex-plugin/marketplace/marketplace.json" "$package" "$payload_path" "${expected[@]}" <<'PY'
import base64
import json
import pathlib
import sys

manifest_path, package_root, output_path, *files = sys.argv[1:]
root = pathlib.Path(package_root)
payload = {
    "marketplace": json.loads(pathlib.Path(manifest_path).read_text(encoding="utf-8")),
    "files": {name: base64.b64encode((root / name).read_bytes()).decode("ascii") for name in files},
}
pathlib.Path(output_path).write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
PY
printf 'Packaged Anyblob at %s and %s\n' "$package" "$zip_path"
