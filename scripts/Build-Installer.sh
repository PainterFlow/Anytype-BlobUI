#!/usr/bin/env bash
set -Eeuo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
workspaceRoot=$(cd -- "$script_dir/.." && pwd -P)
source "$script_dir/Restore-ProcessEnvironment.sh"
source "$script_dir/Workspace-Build-Paths.sh"

development=0
package_release=0
while (($#)); do
    case "$1" in
        -d|--dev|--development) development=1 ;;
        -PackageRelease|--package-release) package_release=1 ;;
        -h|--help) printf 'Usage: %s [-d|--development] [--package-release]\n' "$0"; exit 0 ;;
        *) printf 'Unknown Build-Installer argument: %s\n' "$1" >&2; exit 2 ;;
    esac
    shift
done

manifestPath="$workspaceRoot/installer/Cargo.toml"
versionSource="$workspaceRoot/blobui-release.json"
changelogSource="$workspaceRoot/Changelogs.md"
imageManifest="$workspaceRoot/md-images.yaml"
[[ -f $manifestPath ]] || { printf 'Installer Cargo manifest was not found: %s\n' "$manifestPath" >&2; exit 1; }
[[ -f $versionSource ]] || { printf 'Release version source was not found: %s\n' "$versionSource" >&2; exit 1; }
if ((package_release)); then
    [[ -f $changelogSource && -f $imageManifest ]] || { printf '%s\n' 'Release notes must include Changelogs.md and md-images.yaml.' >&2; exit 1; }
    [[ -f $workspaceRoot/settings-app/dist/blobui-settings.exe ]] || { printf '%s\n' 'Build BlobUI Settings for Windows before packaging a Windows release.' >&2; exit 1; }
    command -v wine >/dev/null 2>&1 && command -v winepath >/dev/null 2>&1 || { printf '%s\n' 'Wine and winepath are required to validate the Windows release package on Linux.' >&2; exit 1; }
    command -v zip >/dev/null 2>&1 || { printf '%s\n' 'The zip command is required to create the Windows release package.' >&2; exit 1; }
fi
command -v cargo >/dev/null 2>&1 || { printf '%s\n' 'Cargo is required to build the installer.' >&2; exit 1; }
command -v rustc >/dev/null 2>&1 || { printf '%s\n' 'rustc is required to build the installer.' >&2; exit 1; }
command -v python3 >/dev/null 2>&1 || { printf '%s\n' 'Python 3 is required to validate release metadata.' >&2; exit 1; }

releaseVersion=$(python3 - "$versionSource" <<'PY'
import json
import re
import sys

data = json.load(open(sys.argv[1], encoding="utf-8"))
version = data.get("version", "")
if data.get("formatVersion") != 1 or not re.fullmatch(r"[0-9][0-9A-Za-z.+-]{0,63}", version):
    raise SystemExit("Invalid release metadata in blobui-release.json.")
print(version)
PY
)
buildCommit=$(git -C "$workspaceRoot" rev-parse --short HEAD)
target=${BLOBUI_WINDOWS_TARGET:-x86_64-pc-windows-gnu}
[[ $target == *windows-gnu ]] || { printf 'Linux builds must use a Windows GNU target: %s\n' "$target" >&2; exit 1; }
mingw_compiler="${target%%-pc-windows-gnu}-w64-mingw32-gcc"
command -v "$mingw_compiler" >/dev/null 2>&1 || command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1 || {
    printf '%s\n' 'A MinGW-w64 cross-compiler is required. Install the compiler for the selected Windows GNU target.' >&2
    exit 1
}
printf 'Building installer for blobui-app Dev+%s (%s)\n' "$buildCommit" "$target"

blobui_save_environment BUILD_INSTALLER_ENV PATH CARGO_HOME RUSTUP_HOME TMPDIR CARGO_TARGET_DIR RUSTFLAGS CARGO_ENCODED_RUSTFLAGS
trap 'blobui_restore_environment BUILD_INSTALLER_ENV PATH CARGO_HOME RUSTUP_HOME TMPDIR CARGO_TARGET_DIR RUSTFLAGS CARGO_ENCODED_RUSTFLAGS' EXIT
source "$script_dir/Rust-Toolchain.sh"
export CARGO_TARGET_DIR="$workspaceRoot/installer/target"
export RUSTFLAGS='-C target-feature=+crt-static'
unset CARGO_ENCODED_RUSTFLAGS
cargo_args=(build --locked --release --quiet --target "$target" --manifest-path "$manifestPath")
if ((development)); then cargo_args+=(--features development); fi
cargo "${cargo_args[@]}"

distributionDirectory="$workspaceRoot/dist"
targetName=blobui-installer.exe
if ((development)); then targetName=blobui-installer-dev.exe; fi
sourceExecutable="$CARGO_TARGET_DIR/$target/release/blobui-installer.exe"
destinationExecutable="$distributionDirectory/$targetName"
assert_workspace_build_path "$destinationExecutable" >/dev/null
[[ -f $sourceExecutable ]] || { printf 'Cargo did not create the installer executable: %s\n' "$sourceExecutable" >&2; exit 1; }
mkdir -p "$distributionDirectory"
cp -f -- "$sourceExecutable" "$destinationExecutable"
printf 'Native installer built: %s\n' "$destinationExecutable"

if ((package_release)); then
    packageRoot=$(mktemp -d "$distributionDirectory/.blobui-package.XXXXXX")
    tempArchive="$distributionDirectory/.blobui-windows-$$.zip"
    packagePath="$distributionDirectory/blobui-windows.zip"
    [[ ! -L $packagePath ]] || { printf 'Refusing to replace a symbolic link: %s\n' "$packagePath" >&2; exit 1; }
    cleanup_package() { rm -rf -- "$packageRoot"; rm -f -- "$tempArchive"; }
    trap 'cleanup_package; blobui_restore_environment BUILD_INSTALLER_ENV PATH CARGO_HOME RUSTUP_HOME TMPDIR CARGO_TARGET_DIR RUSTFLAGS CARGO_ENCODED_RUSTFLAGS' EXIT
    for required in "$workspaceRoot/Dev" "$workspaceRoot/assets"; do
        [[ -d $required ]] || { printf 'Release package source folder is missing: %s\n' "$required" >&2; exit 1; }
        if find "$required" -type l -print -quit | grep -q .; then
            printf 'Refusing to package symbolic links under %s\n' "$required" >&2
            exit 1
        fi
    done
    mkdir -p "$packageRoot/Dev" "$packageRoot/assets" "$packageRoot/settings-app/dist"
    copy_tree() {
        local src=$1 dst=$2 item name
        if [[ -d $src ]]; then
            mkdir -p "$dst"
            while IFS= read -r -d '' item; do
                name=${item##*/}
                [[ ${name,,} == .blobui* ]] && continue
                [[ ! -L $item ]] || { printf 'Refusing to package a symbolic link: %s\n' "$item" >&2; return 1; }
                copy_tree "$item" "$dst/$name"
            done < <(find "$src" -mindepth 1 -maxdepth 1 -print0)
        else
            cp -p -- "$src" "$dst"
        fi
    }
    copy_tree "$workspaceRoot/Dev" "$packageRoot/Dev"
    copy_tree "$workspaceRoot/assets" "$packageRoot/assets"
    cp -f -- "$changelogSource" "$packageRoot/Changelogs.md"
    cp -f -- "$imageManifest" "$packageRoot/md-images.yaml"
    cp -f -- "$workspaceRoot/settings-app/dist/blobui-settings.exe" "$packageRoot/settings-app/dist/blobui-settings.exe"
    python3 - "$releaseVersion" "$packageRoot/blobui-release.json" <<'PY'
import json
import pathlib
import sys
pathlib.Path(sys.argv[2]).write_text(json.dumps({"formatVersion": 1, "version": sys.argv[1]}, separators=(",", ":")) + "\n", encoding="utf-8")
PY
    windowsPackageRoot=$(winepath -w "$packageRoot")
    WINEDEBUG=-all wine "$destinationExecutable" --no-pause --validate-package "$windowsPackageRoot"
    (cd "$packageRoot" && zip -q -r -X "$tempArchive" .)
    mv -f -- "$tempArchive" "$packagePath"
    printf 'Native installer release package created: %s\n' "$packagePath"
fi
