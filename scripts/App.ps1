param(
    [string]$InstallDirectory,
    [switch]$Background,
    [Alias('n')]
    [switch]$Onboarding
)

# Compatibility entry point for BlobUI.cmd and older shortcuts. Runtime work is
# performed by the Rust Settings app; Settings.ps1 only locates and launches it.
$ErrorActionPreference = 'Stop'
$settingsLauncher = Join-Path $PSScriptRoot 'Settings.ps1'
$arguments = @{}
if (-not [string]::IsNullOrWhiteSpace($InstallDirectory)) { $arguments.InstallDirectory = $InstallDirectory }
if ($Background) { $arguments.Background = $true }
if ($Onboarding) { $arguments.Onboarding = $true }
& $settingsLauncher @arguments
