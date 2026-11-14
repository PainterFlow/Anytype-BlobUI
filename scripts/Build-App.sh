#!/usr/bin/env bash
set -Eeuo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
workspaceRoot=$(cd -- "$script_dir/.." && pwd -P)
source "$script_dir/Restore-ProcessEnvironment.sh"
source "$script_dir/Workspace-Build-Paths.sh"

start_app=0
close_app=0
onboarding=0
background=0
while (($#)); do
    case "$1" in
        -s|--start) start_app=1 ;;
        -c|--close) close_app=1 ;;
        -n|--onboarding) onboarding=1 ;;
        -b|--background|--silent) background=1 ;;
        -h|--help) printf 'Usage: %s [-s] [-c] [-n|--onboarding] [-b|--silent]\n' "$0"; exit 0 ;;
        *) printf 'Unknown Build-App argument: %s\n' "$1" >&2; exit 2 ;;
    esac
    shift
done

projectDirectory="$workspaceRoot/settings-app"
manifestPath="$projectDirectory/Cargo.toml"
[[ -f $manifestPath ]] || { printf 'The BlobUI Settings Rust project was not found: %s\n' "$manifestPath" >&2; exit 1; }
command -v node >/dev/null 2>&1 && command -v npm >/dev/null 2>&1 || { printf '%s\n' 'Node.js and npm are required to build the Settings frontend.' >&2; exit 1; }
command -v cargo >/dev/null 2>&1 || { printf '%s\n' 'Cargo is required to build BlobUI Settings.' >&2; exit 1; }
command -v rustc >/dev/null 2>&1 || { printf '%s\n' 'rustc is required to identify the Settings build target.' >&2; exit 1; }
target=${BLOBUI_BUILD_TARGET:-$(rustc -vV | sed -n 's/^host: //p')}
[[ -n $target ]] || { printf '%s\n' 'Could not identify the Rust build target.' >&2; exit 1; }
buildCommit=$(git -C "$workspaceRoot" rev-parse --short HEAD)
printf 'Building blobui-app Dev+%s for %s\n' "$buildCommit" "$target"

if ((close_app)) && [[ ${BLOBUI_BUILD_ONLY:-0} != 1 ]]; then
    closed=0
    while read -r pid; do
        [[ -n $pid ]] || continue
        instance_path=$(readlink -f "/proc/$pid/exe" 2>/dev/null || true)
        if [[ -z $instance_path ]]; then
            printf 'Skipped Settings process %s because its executable path could not be verified.\n' "$pid"
        elif [[ $instance_path == "$workspaceRoot"/* ]]; then
            kill -TERM "$pid"
            closed=$((closed + 1))
        else
            printf 'Skipped Settings process %s outside this workspace.\n' "$pid"
        fi
    done < <(pgrep -x blobui-settings 2>/dev/null || true)
    if ((closed > 0)); then printf 'Closed %s existing workspace BlobUI Settings instance(s).\n' "$closed"; fi
fi

"$script_dir/Build-Codex.sh"
npm_cache="$workspaceRoot/.test-run/npm-cache"
mkdir -p "$npm_cache"
(
    cd "$projectDirectory"
    npm ci --prefer-offline --no-audit --no-fund --cache "$npm_cache"
    npm run build
)

blobui_save_environment BUILD_APP_ENV PATH CARGO_HOME RUSTUP_HOME TMPDIR CARGO_TARGET_DIR RUSTFLAGS CARGO_ENCODED_RUSTFLAGS
trap 'blobui_restore_environment BUILD_APP_ENV PATH CARGO_HOME RUSTUP_HOME TMPDIR CARGO_TARGET_DIR RUSTFLAGS CARGO_ENCODED_RUSTFLAGS' EXIT
source "$script_dir/Rust-Toolchain.sh"
export CARGO_TARGET_DIR="$projectDirectory/target"
cargo build --locked --release --quiet --target "$target" --manifest-path "$manifestPath"

case "$target" in
    *windows*) executable_name=blobui-settings.exe ;;
    *) executable_name=blobui-settings ;;
esac
releaseExecutable="$CARGO_TARGET_DIR/$target/release/$executable_name"
[[ -f $releaseExecutable ]] || { printf 'Cargo did not create the Settings executable: %s\n' "$releaseExecutable" >&2; exit 1; }
frontendOutputDirectory="$projectDirectory/frontend-dist"
distributionDirectory="$projectDirectory/dist"
distributionExecutable="$distributionDirectory/$executable_name"
assert_workspace_build_path "$distributionDirectory" >/dev/null
assert_workspace_build_path "$distributionExecutable" >/dev/null
mkdir -p "$distributionDirectory"
[[ -f $workspaceRoot/dev-guide.md && -f $frontendOutputDirectory/dev-guide.md ]] || { printf '%s\n' 'The Settings frontend did not bundle dev-guide.md.' >&2; exit 1; }

docsPath="$distributionDirectory/docs"
if [[ -e $docsPath || -L $docsPath ]]; then
    assert_workspace_build_path "$docsPath" >/dev/null
    [[ ! -L $docsPath ]] || { printf 'The obsolete Settings documentation folder is a symbolic link: %s\n' "$docsPath" >&2; exit 1; }
    rm -rf -- "$docsPath"
fi
cp -f -- "$frontendOutputDirectory/dev-guide.md" "$distributionDirectory/dev-guide.md"
for releaseNotesFile in Changelogs.md md-images.yaml; do
    source_file="$workspaceRoot/$releaseNotesFile"
    [[ -f $source_file ]] || { printf 'The Settings app build is missing its release notes file: %s\n' "$source_file" >&2; exit 1; }
    assert_workspace_build_path "$frontendOutputDirectory/$releaseNotesFile" >/dev/null
    assert_workspace_build_path "$distributionDirectory/$releaseNotesFile" >/dev/null
    cp -f -- "$source_file" "$frontendOutputDirectory/$releaseNotesFile"
    cp -f -- "$source_file" "$distributionDirectory/$releaseNotesFile"
done
for directory in "$frontendOutputDirectory" "$distributionDirectory"; do
    legacy="$directory/Changelogs.yaml"
    if [[ -e $legacy || -L $legacy ]]; then
        assert_workspace_build_path "$legacy" >/dev/null
        [[ ! -L $legacy && ! -d $legacy ]] || { printf 'Refusing unsafe legacy image manifest: %s\n' "$legacy" >&2; exit 1; }
        rm -f -- "$legacy"
    fi
done
cp -f -- "$releaseExecutable" "$distributionExecutable"
chmod +x "$distributionExecutable"
printf 'BlobUI Settings was built at %s\n' "$distributionExecutable"

write_request_marker() {
    local path=$1
    assert_workspace_build_path "$path" >/dev/null
    [[ ! -L $path && ! -d $path ]] || { printf 'Refusing unsafe Settings request marker: %s\n' "$path" >&2; return 1; }
    [[ -e $path ]] || : > "$path"
}
if ((background && !start_app)); then
    write_request_marker "$distributionDirectory/.blobui-background-next"
    printf '%s\n' 'The next BlobUI Settings launch will start in the background.'
fi
if ((onboarding && !start_app)); then
    write_request_marker "$distributionDirectory/.blobui-onboarding-next"
    printf '%s\n' 'The next BlobUI Settings launch will open onboarding.'
fi

if ((start_app)) && [[ ${BLOBUI_BUILD_ONLY:-0} != 1 ]]; then
    app_args=()
    if ((onboarding)); then app_args+=(--onboarding); fi
    if ((background)); then app_args+=(--background); fi
    host_target=$(rustc -vV | sed -n 's/^host: //p')
    if [[ $target == "$host_target" ]]; then
        if [[ $executable_name == *.exe ]]; then
            command -v wine >/dev/null 2>&1 || { printf '%s\n' 'Wine is required to launch a Windows build from Linux.' >&2; exit 1; }
            launch=(wine "$distributionExecutable")
        else
            launch=("$distributionExecutable")
        fi
    elif [[ $target == *windows* ]]; then
        command -v wine >/dev/null 2>&1 || { printf '%s\n' 'Wine is required to launch a Windows build from Linux.' >&2; exit 1; }
        launch=(wine "$distributionExecutable")
    else
        printf 'Cannot launch a cross-compiled target from this host: %s\n' "$target" >&2
        exit 1
    fi
    nohup "${launch[@]}" "${app_args[@]}" >/dev/null 2>&1 </dev/null &
fi
