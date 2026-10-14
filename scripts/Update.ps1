param(
    [string]$ApiBaseUri = 'http://127.0.0.1:31009',
    [scriptblock]$ApiTransport,
    [string]$InstallDirectory
)

$ErrorActionPreference = 'Stop'
$rebuildArguments = @{
    ApiBaseUri = $ApiBaseUri
    ApiTransport = $ApiTransport
    ForUpdate = $true
}
if (-not [string]::IsNullOrWhiteSpace($InstallDirectory)) {
    $rebuildArguments.InstallDirectory = $InstallDirectory
}

& (Join-Path $PSScriptRoot 'Rebuild.ps1') @rebuildArguments
Write-Host 'BlobUI update completed. Your saved API key and settings object selections were reused.'
