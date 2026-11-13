#!/usr/bin/env bash
set -Eeuo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
workspaceRoot=$(cd -- "$script_dir/../.." && pwd -P)
mkdir -p "$workspaceRoot/.test-run"
fixture=$(mktemp -d "$workspaceRoot/.test-run/build-app-arguments.XXXXXX")
trap 'rm -rf -- "$fixture"' EXIT
mkdir -p "$fixture/scripts" "$fixture/settings-app" "$fixture/bin" "$fixture/.test-run"
cp "$workspaceRoot/scripts/Build-App.sh" "$workspaceRoot/scripts/Restore-ProcessEnvironment.sh" "$workspaceRoot/scripts/Workspace-Build-Paths.sh" "$workspaceRoot/scripts/Rust-Toolchain.sh" "$fixture/scripts/"
cat > "$fixture/scripts/Build-Codex.sh" <<'SH'
#!/usr/bin/env bash
exit 0
SH
chmod +x "$fixture/scripts/Build-Codex.sh"
echo fixture > "$fixture/settings-app/Cargo.toml"
echo guide > "$fixture/dev-guide.md"
echo notes > "$fixture/Changelogs.md"
echo 'images: []' > "$fixture/md-images.yaml"

if command -v rustc >/dev/null 2>&1; then
    host_target=$(rustc -vV | sed -n 's/^host: //p')
else
    host_target="$(uname -m)-unknown-linux-gnu"
fi
cat > "$fixture/bin/git" <<'SH'
#!/usr/bin/env bash
echo testcommit
SH
cat > "$fixture/bin/node" <<'SH'
#!/usr/bin/env bash
exit 0
SH
cat > "$fixture/bin/rustc" <<'SH'
#!/usr/bin/env bash
echo "host: $BLOBUI_BUILD_TARGET"
SH
cat > "$fixture/bin/npm" <<'SH'
#!/usr/bin/env bash
if [[ $1 == run && $2 == build ]]; then
    mkdir -p frontend-dist
    cp "$BLOBUI_TEST_ROOT/dev-guide.md" frontend-dist/dev-guide.md
fi
SH
cat > "$fixture/bin/cargo" <<'SH'
#!/usr/bin/env bash
release="$CARGO_TARGET_DIR/$BLOBUI_BUILD_TARGET/release"
mkdir -p "$release"
cat > "$release/blobui-settings" <<'APP'
#!/usr/bin/env bash
echo "$*" > "$BLOBUI_LAUNCH_LOG"
APP
chmod +x "$release/blobui-settings"
SH
chmod +x "$fixture/bin/"*

export PATH="$fixture/bin:$PATH"
export BLOBUI_BUILD_TARGET="$host_target"
export BLOBUI_BUILD_ONLY=0
export BLOBUI_TEST_ROOT="$fixture"
export BLOBUI_LAUNCH_LOG="$fixture/launch.log"
run_build() { "$fixture/scripts/Build-App.sh" "$@" >/dev/null; }

run_build --silent
[[ -f $fixture/settings-app/dist/.blobui-background-next && ! -e $BLOBUI_LAUNCH_LOG ]] || { echo '--silent did not queue background mode without launching.' >&2; exit 1; }
run_build -n
[[ -f $fixture/settings-app/dist/.blobui-onboarding-next ]] || { echo '-n did not queue onboarding.' >&2; exit 1; }
run_build -b -n
[[ -f $fixture/settings-app/dist/.blobui-background-next && -f $fixture/settings-app/dist/.blobui-onboarding-next ]] || { echo 'Combined deferred settings were not queued.' >&2; exit 1; }
rm -f "$BLOBUI_LAUNCH_LOG"
run_build -s -n
for _ in {1..50}; do [[ -f $BLOBUI_LAUNCH_LOG ]] && break; sleep 0.02; done
[[ $(cat "$BLOBUI_LAUNCH_LOG") == --onboarding ]] || { echo '-s -n did not launch with onboarding.' >&2; exit 1; }
rm -f "$BLOBUI_LAUNCH_LOG"
run_build -s -b -n
for _ in {1..50}; do [[ -f $BLOBUI_LAUNCH_LOG ]] && break; sleep 0.02; done
[[ $(cat "$BLOBUI_LAUNCH_LOG") == '--onboarding --background' ]] || { echo 'Silent launch arguments differ.' >&2; exit 1; }
if "$fixture/scripts/Build-App.sh" --invalid >/dev/null 2>&1; then
    echo 'Unknown Build-App options were accepted.' >&2
    exit 1
fi
echo 'Build-App silent and onboarding argument checks passed.'
