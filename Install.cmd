@echo off
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\Setup.ps1"
if errorlevel 1 (
  echo BlobUI installation failed.
  pause
  exit /b 1
)
pause
