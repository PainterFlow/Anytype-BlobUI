param(
    [Parameter(Mandatory = $true)][string]$ApiKey,
    [Parameter(Mandatory = $true)][string]$OutputPath,
    [string]$SpaceId,
    [string[]]$ObjectIds,
    [string]$ExistingDataPath,
    [string]$InstallDirectory,
    [string]$ApiBaseUri = 'http://127.0.0.1:31009',
    [scriptblock]$SelectionProvider,
    [scriptblock]$ApiTransport
)

$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'BlobUI.Core.psm1') -Force

$arguments = @{
    ApiKey = $ApiKey
    OutputPath = $OutputPath
    SpaceId = $SpaceId
    ObjectIds = $ObjectIds
    ExistingDataPath = $ExistingDataPath
    InstallDirectory = $InstallDirectory
    ApiBaseUri = $ApiBaseUri
    SelectionProvider = $SelectionProvider
    ApiTransport = $ApiTransport
}
Get-BlobUiSettingsData @arguments | Out-Null

Write-Host 'Normalized settings were saved.'
