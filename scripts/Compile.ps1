param(
    [Parameter(Mandatory = $true)][string]$DataPath,
    [Parameter(Mandatory = $true)][string]$OutputPath,
    [string]$DevDirectory = (Join-Path (Split-Path -Parent $PSScriptRoot) 'Dev'),
    [string]$AssetSourceDirectory = (Join-Path (Split-Path -Parent $PSScriptRoot) 'assets')
)

$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'BlobUI.Core.psm1') -Force

$data = Read-BlobUiDataFile -Path $DataPath
$arguments = @{
    DevDirectory = $DevDirectory
    AssetSourceDirectory = $AssetSourceDirectory
    OutputPath = $OutputPath
    Data = $data
}
$result = Invoke-BlobUiCssCompile @arguments
Write-Host ('Compiled {0} CSS source files to {1}' -f $result.sourceFiles.Count, $result.path)
