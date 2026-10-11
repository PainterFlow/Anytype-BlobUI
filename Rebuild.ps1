param(
    [string]$ApiBaseUri = 'http://127.0.0.1:31009',
    [scriptblock]$ApiTransport
)

$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'BlobUI.Core.psm1') -Force

$blobDirectory = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$dataPath = Join-Path $blobDirectory 'data.json'
$credentialPath = Join-Path $blobDirectory 'credentials\anytype-api-key.dpapi'
$data = Read-BlobUiDataFile -Path $dataPath
$installDirectory = [System.IO.Path]::GetFullPath([string]$data.installDirectory)
if ([string]::IsNullOrWhiteSpace([string]$data.selectedSpace.id)) {
    throw 'The saved settings space is missing. Run Install.ps1 to choose it again.'
}
$storedObjectIds = @($data.settingsObjectIds)
if ($storedObjectIds.Count -eq 0) {
    throw 'The saved settings object IDs are missing. Run Install.ps1 to choose them again.'
}
if (-not (Test-Path -LiteralPath $installDirectory -PathType Container)) {
    throw "The saved Anytype directory does not exist: $installDirectory"
}

$runDirectory = Join-Path (Join-Path $blobDirectory 'staging') ([System.Guid]::NewGuid().ToString('N'))
[System.IO.Directory]::CreateDirectory($runDirectory) | Out-Null
$apiKey = $null
try {
    $apiKey = Read-BlobUiProtectedCredential -Path $credentialPath
    $candidateDataPath = Join-Path $runDirectory 'data.json'
    $compiledCssPath = Join-Path $runDirectory 'custom.css'

    $getDataArguments = @{
        ApiKey = $apiKey
        OutputPath = $candidateDataPath
        SpaceId = [string]$data.selectedSpace.id
        ObjectIds = $storedObjectIds
        ExistingDataPath = $dataPath
        InstallDirectory = $installDirectory
        ApiBaseUri = $ApiBaseUri
        ApiTransport = $ApiTransport
    }
    & (Join-Path $PSScriptRoot 'Get-Data.ps1') @getDataArguments

    $compileArguments = @{
        DataPath = $candidateDataPath
        OutputPath = $compiledCssPath
        DevDirectory = Join-Path $PSScriptRoot 'Dev'
        AssetSourceDirectory = Join-Path $PSScriptRoot 'assets'
    }
    & (Join-Path $PSScriptRoot 'Compile.ps1') @compileArguments

    $installArguments = @{
        InstallDirectory = $installDirectory
        RuntimeSourceDirectory = $PSScriptRoot
        AssetsSourceDirectory = Join-Path $PSScriptRoot 'assets'
        CompiledCssPath = $compiledCssPath
        DataCandidatePath = $candidateDataPath
    }
    $result = Invoke-BlobUiInstall @installArguments

    Write-Host "BlobUI was rebuilt at $($result.cssPath)"
    Write-Host 'Switch to Anytype and press Ctrl+R to reload the stylesheet. If custom CSS is not active, select Menu > File > Apply Custom CSS first.'
}
catch {
    Write-Host "BlobUI rebuild failed: $($_.Exception.Message)" -ForegroundColor Red
    throw
}
finally {
    $apiKey = $null
    if (Test-Path -LiteralPath $runDirectory -PathType Container) {
        Remove-Item -LiteralPath $runDirectory -Recurse -Force -ErrorAction SilentlyContinue
    }
    $stagingDirectory = Split-Path -Parent $runDirectory
    if ((Test-Path -LiteralPath $stagingDirectory -PathType Container) -and
        @(Get-ChildItem -LiteralPath $stagingDirectory -Force).Count -eq 0) {
        Remove-Item -LiteralPath $stagingDirectory -Force -ErrorAction SilentlyContinue
    }
}
