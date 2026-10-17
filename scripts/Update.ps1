# TEMPORARILY OUTDATED: this maintenance path still depends on the legacy
# PowerShell runtime and is not supported for current Rust-runtime installs.
param(
    [string]$ApiBaseUri = 'http://127.0.0.1:31009',
    [scriptblock]$ApiTransport,
    [string]$InstallDirectory
)

$ErrorActionPreference = 'Stop'
Write-Warning 'TEMPORARILY OUTDATED: Update.ps1 still targets the legacy PowerShell runtime.'
$rebuildArguments = @{
    ApiTransport = $ApiTransport
    ForUpdate = $true
}
if ($PSBoundParameters.ContainsKey('ApiBaseUri')) { $rebuildArguments.ApiBaseUri = $ApiBaseUri }
if (-not [string]::IsNullOrWhiteSpace($InstallDirectory)) {
    $rebuildArguments.InstallDirectory = $InstallDirectory
}

& (Join-Path $PSScriptRoot 'Rebuild.ps1') @rebuildArguments
Write-Host 'BlobUI update completed. Your saved connection mode and settings were reused.'
