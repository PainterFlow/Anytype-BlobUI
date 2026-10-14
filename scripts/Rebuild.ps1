param(
    [string]$ApiBaseUri = 'http://127.0.0.1:31009',
    [scriptblock]$ApiTransport,
    [string]$InstallDirectory,
    [switch]$ForUpdate
)

$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'BlobUI.Core.psm1') -Force
$sourceDirectory = Split-Path -Parent $PSScriptRoot

function Resolve-BlobUiRebuildState {
    param(
        [string]$RequestedInstallDirectory,
        [string]$SourceDirectory
    )

    $runtimeBlobDirectory = [System.IO.Path]::GetFullPath((Join-Path $SourceDirectory '..'))
    $runtimeDataPath = Join-Path $runtimeBlobDirectory 'data.json'
    if ([string]::IsNullOrWhiteSpace($RequestedInstallDirectory) -and
        (Test-Path -LiteralPath $runtimeDataPath -PathType Leaf)) {
        return [pscustomobject]@{
            BlobDirectory = $runtimeBlobDirectory
            InstallDirectory = $null
        }
    }

    if (-not [string]::IsNullOrWhiteSpace($RequestedInstallDirectory)) {
        $installDirectory = [System.IO.Path]::GetFullPath($RequestedInstallDirectory)
    }
    else {
        $pointerPath = Get-BlobUiInstallPointerPath -SourceDirectory $SourceDirectory
        if (Test-Path -LiteralPath $pointerPath -PathType Leaf) {
            try {
                $pointer = ConvertFrom-Json -InputObject ([System.IO.File]::ReadAllText($pointerPath)) -ErrorAction Stop
                $installDirectory = [System.IO.Path]::GetFullPath([string]$pointer.installDirectory)
            }
            catch {
                throw "The saved BlobUI install location is invalid: $pointerPath. Run Install.ps1 again or pass -InstallDirectory."
            }
            if (-not (Test-Path -LiteralPath $installDirectory -PathType Container)) {
                throw "The saved BlobUI install directory no longer exists: $installDirectory. Run Install.ps1 again or pass -InstallDirectory."
            }
        }
        elseif (-not [string]::IsNullOrWhiteSpace($env:APPDATA)) {
            $defaultInstallDirectory = [System.IO.Path]::GetFullPath((Join-Path $env:APPDATA 'anytype'))
            $defaultDataPath = Join-Path (Join-Path $defaultInstallDirectory 'blob') 'data.json'
            if (Test-Path -LiteralPath $defaultDataPath -PathType Leaf) {
                $installDirectory = $defaultInstallDirectory
            }
        }
    }

    if ([string]::IsNullOrWhiteSpace($installDirectory)) {
        throw 'No BlobUI installation was found. Run Install.ps1 from this source folder, or pass -InstallDirectory with the Anytype data directory.'
    }

    $installDataPath = Join-Path (Join-Path $installDirectory 'blob') 'data.json'
    if (-not (Test-Path -LiteralPath $installDataPath -PathType Leaf)) {
        throw "BlobUI settings were not found at the selected installation: $installDataPath. Run Install.ps1 again or pass -InstallDirectory."
    }
    $savedData = Read-BlobUiDataFile -Path $installDataPath
    $savedInstallDirectory = [System.IO.Path]::GetFullPath([string]$savedData.installDirectory)
    if (-not $savedInstallDirectory.Equals($installDirectory, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "The saved BlobUI settings point to '$savedInstallDirectory', but the selected location is '$installDirectory'. Run Install.ps1 again or pass -InstallDirectory."
    }
    return [pscustomobject]@{
        BlobDirectory = Join-Path $installDirectory 'blob'
        InstallDirectory = $installDirectory
    }
}

$state = Resolve-BlobUiRebuildState -RequestedInstallDirectory $InstallDirectory -SourceDirectory $sourceDirectory
$blobDirectory = $state.BlobDirectory
$dataPath = Join-Path $blobDirectory 'data.json'
$credentialPath = Join-Path $blobDirectory 'credentials\anytype-api-key.dpapi'
$data = Read-BlobUiDataFile -Path $dataPath
$installDirectory = [System.IO.Path]::GetFullPath([string]$data.installDirectory)
if ($null -eq $state.InstallDirectory) {
    $runtimeExpectedInstallDirectory = [System.IO.Path]::GetFullPath((Join-Path $blobDirectory '..'))
    if (-not $installDirectory.Equals($runtimeExpectedInstallDirectory, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "The runtime copy's settings point to '$installDirectory', but its blob folder belongs to '$runtimeExpectedInstallDirectory'. Run the source Rebuild.ps1 with -InstallDirectory to use another installation."
    }
}
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

    if ($ForUpdate) {
        $previousReleaseVersion = ''
        $manifestPath = Join-Path $blobDirectory '.blobui-managed-files.json'
        if (Test-Path -LiteralPath $manifestPath) {
            $manifestItem = Get-Item -LiteralPath $manifestPath -Force -ErrorAction Stop
            if ($manifestItem.PSIsContainer -or
                (($manifestItem.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0)) {
                throw "The saved BlobUI install manifest cannot be read safely: $manifestPath"
            }
            try {
                $previousManifest = ConvertFrom-Json -InputObject ([System.IO.File]::ReadAllText($manifestPath)) -ErrorAction Stop
                $previousReleaseVersion = [string]$previousManifest.releaseVersion
            }
            catch {
                throw "The saved BlobUI install manifest is invalid: $manifestPath"
            }
        }
        $candidateData = Read-BlobUiDataFile -Path $candidateDataPath
        $candidateData = Invoke-BlobUiLocalDataMigrations -Data $candidateData -FromReleaseVersion $previousReleaseVersion -ToReleaseVersion (Get-BlobUiReleaseVersion)
        Write-BlobUiAtomicJson -Path $candidateDataPath -InputObject $candidateData
    }

    $compileArguments = @{
        DataPath = $candidateDataPath
        OutputPath = $compiledCssPath
        DevDirectory = Join-Path $sourceDirectory 'Dev'
        AssetSourceDirectory = Join-Path $sourceDirectory 'assets'
    }
    & (Join-Path $PSScriptRoot 'Compile.ps1') @compileArguments

    $installArguments = @{
        InstallDirectory = $installDirectory
        RuntimeSourceDirectory = $sourceDirectory
        AssetsSourceDirectory = Join-Path $sourceDirectory 'assets'
        CompiledCssPath = $compiledCssPath
        DataCandidatePath = $candidateDataPath
    }
    $result = Invoke-BlobUiInstall @installArguments

    if ($ForUpdate) {
        Write-Host "BlobUI was updated to release $(Get-BlobUiReleaseVersion) at $($result.cssPath)"
    }
    else {
        Write-Host "BlobUI was rebuilt at $($result.cssPath)"
    }
    Write-Host 'Switch to Anytype and press Ctrl+R to reload the stylesheet. If custom CSS is not active, select Menu > File > Apply Custom CSS first.'
}
catch {
    $operationName = if ($ForUpdate) { 'update' } else { 'rebuild' }
    Write-Host "BlobUI $operationName failed: $($_.Exception.Message)" -ForegroundColor Red
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
