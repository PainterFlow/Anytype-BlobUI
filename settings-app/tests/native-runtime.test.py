"""Verify the Rust runtime owns save/API work and legacy launch wrappers remain."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
rust = ROOT / "settings-app/src"
main = (rust / "main.rs").read_text(encoding="utf-8")
runtime_modules = "\n".join(
    (rust / name).read_text(encoding="utf-8")
    for name in ("api.rs", "css.rs", "storage.rs", "connection.rs")
)
app_wrapper = (ROOT / "scripts/App.ps1").read_text(encoding="utf-8")
settings_wrapper = (ROOT / "scripts/Settings.ps1").read_text(encoding="utf-8")
setup = (ROOT / "scripts/Setup.ps1").read_text(encoding="utf-8")
build = (ROOT / "scripts/Build-Settings.ps1").read_text(encoding="utf-8")

assert "mod api;" in main and "mod css;" in main and "mod storage;" in main
assert "api::verify_connection" in main and "api::calendar_colors" in main
assert "css::compile" in main and "storage::commit_settings" in main
assert "Command::new" not in main and "PowerShell" not in main
assert "Rebuild.ps1" not in main and "Get-Data.ps1" not in main
assert "CryptProtectData" in runtime_modules and "CryptUnprotectData" in runtime_modules
assert "ReplaceFileW" in runtime_modules and "MoveFileExW" in runtime_modules
assert "rollback" in runtime_modules.lower()

# The compatibility wrapper forwards supported arguments to the required
# Settings.ps1 launcher. The launcher starts the Rust executable only.
assert "Settings.ps1" in app_wrapper
assert "Windows.Forms" not in app_wrapper and "Rebuild.ps1" not in app_wrapper
for option in ("InstallDirectory", "Background", "Onboarding"):
    assert option in app_wrapper and option in settings_wrapper
assert "Start-Process" in settings_wrapper
assert "settings-app\\dist\\blobui-settings.exe" in settings_wrapper
assert "settings-app/dist/blobui-settings.exe" in setup
assert "scripts/Build-Settings.ps1 before installing" in setup
assert "-InstallDirectory \"' + $AnytypeDirectory" in setup
assert "cargo build --locked --release" in build

print("Native runtime checks passed: Rust owns API, CSS, and transactional saves; PowerShell build and Settings launch entry points remain.")
