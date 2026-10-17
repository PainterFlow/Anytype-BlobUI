@echo off
start "BlobUI" powershell.exe -NoProfile -STA -ExecutionPolicy Bypass -File "%~dp0scripts\App.ps1"
