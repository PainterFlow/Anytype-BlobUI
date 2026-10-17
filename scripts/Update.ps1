# Compatibility launcher. Update checks and installation belong to the Rust app.
param(
    [string]$ApiBaseUri = 'http://127.0.0.1:31009',
    [scriptblock]$ApiTransport,
    [string]$InstallDirectory
)

$ErrorActionPreference = 'Stop'
Write-Host 'Open Advanced Settings to check for and install BlobUI updates.'
$launchArguments = @{}
if (-not [string]::IsNullOrWhiteSpace($InstallDirectory)) {
    $launchArguments.InstallDirectory = $InstallDirectory
}

& (Join-Path $PSScriptRoot 'Settings.ps1') @launchArguments
