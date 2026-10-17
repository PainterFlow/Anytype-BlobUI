param(
    [string]$ApiBaseUri = 'http://127.0.0.1:31009',
    [scriptblock]$ApiTransport,
    [string]$InstallDirectory
)

$ErrorActionPreference = 'Stop'
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
