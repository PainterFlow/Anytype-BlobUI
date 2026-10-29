"""Verify the Rust runtime owns save/API work without PowerShell launch wrappers."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
rust = ROOT / "settings-app/src"
main = (rust / "main.rs").read_text(encoding="utf-8")
runtime_modules = "\n".join(
    (rust / name).read_text(encoding="utf-8")
    for name in ("api.rs", "css.rs", "storage.rs", "connection.rs")
)
build = (ROOT / "scripts/Build-App.ps1").read_text(encoding="utf-8")

assert "mod api;" in main and "mod css;" in main and "mod storage;" in main
assert "api::verify_connection" in main and "calendar::snapshot" in main
assert "css::compile" in main and "storage::commit_settings" in main
assert 'Command::new("powershell.exe")' not in main and 'Command::new("cmd.exe")' not in main
assert "PowerShell" not in main
assert "Rebuild.ps1" not in main and "Get-Data.ps1" not in main
assert "CryptProtectData" in runtime_modules and "CryptUnprotectData" in runtime_modules
assert "ReplaceFileW" in runtime_modules and "MoveFileExW" in runtime_modules
assert "rollback" in runtime_modules.lower()

assert {p.name for p in (ROOT / "scripts").glob("*.ps1")} == {
    "Build-App.ps1", "Build-Installer.ps1"
}
assert not list((ROOT / "scripts").glob("*.psm1"))
assert "cargo build --locked --release" in build

print("Native runtime checks passed. Rust owns API, CSS, and transactional saves; only developer build scripts remain.")
