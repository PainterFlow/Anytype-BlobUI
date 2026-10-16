param(
    [string]$ApiBaseUri = 'http://127.0.0.1:31009',
    [scriptblock]$ApiTransport,
    [string]$InstallDirectory,
    [string]$SettingsOverridePath,
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

function New-BlobUiUpdateCompileDirectory {
    param(
        [Parameter(Mandatory = $true)][string]$PackageDevDirectory,
        [Parameter(Mandatory = $true)][string]$InstalledDevDirectory,
        [Parameter(Mandatory = $true)][string]$OutputDirectory,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$ManagedFiles
    )

    $packageRoot = [System.IO.Path]::GetFullPath($PackageDevDirectory).TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
    if (-not (Test-Path -LiteralPath $PackageDevDirectory -PathType Container)) {
        throw "Development CSS directory was not found: $PackageDevDirectory"
    }
    [System.IO.Directory]::CreateDirectory($OutputDirectory) | Out-Null

    foreach ($file in (Get-ChildItem -LiteralPath $PackageDevDirectory -Recurse -File -Filter '*.css')) {
        if (($file.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) { continue }
        $relativePath = $file.FullName.Substring($packageRoot.Length)
        $destinationPath = Join-Path $OutputDirectory $relativePath
        [System.IO.Directory]::CreateDirectory((Split-Path -Parent $destinationPath)) | Out-Null
        [System.IO.File]::WriteAllBytes($destinationPath, [System.IO.File]::ReadAllBytes($file.FullName))
    }

    if (-not (Test-Path -LiteralPath $InstalledDevDirectory -PathType Container)) { return }
    $installedRoot = [System.IO.Path]::GetFullPath($InstalledDevDirectory).TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
    $managedSet = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
    foreach ($managedFile in $ManagedFiles) {
        $key = [string]$managedFile
        if ($key.StartsWith('runtime/Dev/', [System.StringComparison]::OrdinalIgnoreCase)) {
            $null = $managedSet.Add($key)
        }
    }

    foreach ($file in (Get-ChildItem -LiteralPath $InstalledDevDirectory -Recurse -File -Filter '*.css')) {
        if (($file.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) { continue }
        $relativePath = $file.FullName.Substring($installedRoot.Length)
        $managedKey = 'runtime/Dev/' + $relativePath.Replace('\', '/')
        if ($managedSet.Contains($managedKey)) { continue }

        $destinationPath = Join-Path $OutputDirectory $relativePath
        # A release file with the same name takes precedence over a local file.
        if (Test-Path -LiteralPath $destinationPath -PathType Leaf) { continue }
        [System.IO.Directory]::CreateDirectory((Split-Path -Parent $destinationPath)) | Out-Null
        [System.IO.File]::WriteAllBytes($destinationPath, [System.IO.File]::ReadAllBytes($file.FullName))
    }
}

$state = Resolve-BlobUiRebuildState -RequestedInstallDirectory $InstallDirectory -SourceDirectory $sourceDirectory
$blobDirectory = $state.BlobDirectory
$dataPath = Join-Path $blobDirectory 'data.json'
$credentialPath = Join-Path $blobDirectory 'credentials\anytype-api-key.dpapi'
$data = Read-BlobUiDataFile -Path $dataPath
$installDirectory = [System.IO.Path]::GetFullPath([string]$data.installDirectory)
$releasePathParts = @($installDirectory -split '[\\/]+' | Where-Object {
    $_.Equals('Releases', [System.StringComparison]::OrdinalIgnoreCase)
})
if ($releasePathParts.Count -gt 0) {
    throw 'The selected directory is inside a Releases path. BlobUI will not overwrite custom.css in a Releases folder.'
}
if ($null -eq $state.InstallDirectory) {
    $runtimeExpectedInstallDirectory = [System.IO.Path]::GetFullPath((Join-Path $blobDirectory '..'))
    if (-not $installDirectory.Equals($runtimeExpectedInstallDirectory, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "The runtime copy's settings point to '$installDirectory', but its blob folder belongs to '$runtimeExpectedInstallDirectory'. Run the source Rebuild.ps1 with -InstallDirectory to use another installation."
    }
}
$settingsV2Property = $data.PSObject.Properties['settingsV2']
$settingsOverlay = if ($null -ne $settingsV2Property) { $settingsV2Property.Value } else { $null }
$useSettingsV2 = $null -ne $settingsOverlay
if (-not [string]::IsNullOrWhiteSpace($SettingsOverridePath)) {
    $overridePath = [System.IO.Path]::GetFullPath($SettingsOverridePath)
    $overrideRoot = [System.IO.Path]::GetFullPath((Join-Path $blobDirectory 'staging')).TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
    if (-not $overridePath.StartsWith($overrideRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw 'A local settings override must be staged inside the BlobUI blob\staging directory.'
    }
    $overrideItem = Get-Item -LiteralPath $overridePath -Force -ErrorAction SilentlyContinue
    if ($null -eq $overrideItem -or $overrideItem.PSIsContainer -or
        (($overrideItem.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0)) {
        throw 'The staged local settings override is not a safe regular file.'
    }
    try {
        $settingsOverlay = ConvertFrom-Json -InputObject ([System.IO.File]::ReadAllText($overridePath, [System.Text.Encoding]::UTF8)) -ErrorAction Stop
    }
    catch {
        throw 'The staged local settings override is not valid JSON.'
    }
    $useSettingsV2 = $true
}
if (-not $useSettingsV2) {
    if ([string]::IsNullOrWhiteSpace([string]$data.selectedSpace.id)) {
        throw 'The saved settings space is missing. Run Install.ps1 to choose it again.'
    }
    $storedObjectIds = @($data.settingsObjectIds)
    if ($storedObjectIds.Count -eq 0) {
        throw 'The saved settings object IDs are missing. Run Install.ps1 to choose them again.'
    }
}
if (-not (Test-Path -LiteralPath $installDirectory -PathType Container)) {
    throw "The saved Anytype directory does not exist: $installDirectory"
}

$runDirectory = Join-Path (Join-Path $blobDirectory 'staging') ([System.Guid]::NewGuid().ToString('N'))
[System.IO.Directory]::CreateDirectory($runDirectory) | Out-Null
$apiKey = $null
try {
    $candidateDataPath = Join-Path $runDirectory 'data.json'
    $compiledCssPath = Join-Path $runDirectory 'custom.css'

    if ($useSettingsV2) {
        $candidateData = ConvertFrom-Json -InputObject (ConvertTo-Json -InputObject $data -Depth 40) -ErrorAction Stop
        if ($null -eq $candidateData.PSObject.Properties['calendarObjectColors']) {
            $null = $candidateData | Add-Member -MemberType NoteProperty -Name 'calendarObjectColors' -Value ([ordered]@{})
        }
        $calendarCacheRefreshed = $false
        if (Test-Path -LiteralPath $credentialPath -PathType Leaf) {
            try {
                $apiKey = Read-BlobUiProtectedCredential -Path $credentialPath
                $candidateData = Update-BlobUiCalendarColorCache -ApiKey $apiKey -Data $candidateData -ApiBaseUri $ApiBaseUri -ApiTransport $ApiTransport
                $calendarCacheRefreshed = $true
            }
            catch {
                Write-Host "BlobUI_CALENDAR_CACHE=cached"
                Write-Host "Calendar color data stayed at its last saved values because the Anytype API refresh failed: $($_.Exception.Message)" -ForegroundColor Yellow
            }
        }
        else {
            Write-Host 'BlobUI_CALENDAR_CACHE=cached'
            Write-Host 'Calendar color data stayed at its last saved values because no Anytype API key is saved.' -ForegroundColor Yellow
        }
        if ($calendarCacheRefreshed) {
            Write-Host 'BlobUI_CALENDAR_CACHE=refreshed'
        }
        $candidateData = Set-BlobUiSettingsV2Overlay -Data $candidateData -Overlay $settingsOverlay
        Write-BlobUiAtomicJson -Path $candidateDataPath -InputObject $candidateData
    }
    else {
        $apiKey = Read-BlobUiProtectedCredential -Path $credentialPath
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
    }

    if ($ForUpdate) {
        $previousReleaseVersion = ''
        $previousManagedFiles = @()
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
                $previousManagedFiles = @($previousManifest.managedFiles)
            }
            catch {
                throw "The saved BlobUI install manifest is invalid: $manifestPath"
            }
        }
        $candidateData = Read-BlobUiDataFile -Path $candidateDataPath
        $candidateData = Invoke-BlobUiLocalDataMigrations -Data $candidateData -FromReleaseVersion $previousReleaseVersion -ToReleaseVersion (Get-BlobUiReleaseVersion)
        Write-BlobUiAtomicJson -Path $candidateDataPath -InputObject $candidateData
    }

    $compileDevDirectory = Join-Path $sourceDirectory 'Dev'
    if ($ForUpdate) {
        $compileDevDirectory = Join-Path $runDirectory 'compile-Dev'
        $installedDevDirectory = Join-Path (Join-Path $blobDirectory 'runtime') 'Dev'
        New-BlobUiUpdateCompileDirectory -PackageDevDirectory (Join-Path $sourceDirectory 'Dev') -InstalledDevDirectory $installedDevDirectory -OutputDirectory $compileDevDirectory -ManagedFiles $previousManagedFiles
    }

    $compileArguments = @{
        DataPath = $candidateDataPath
        OutputPath = $compiledCssPath
        DevDirectory = $compileDevDirectory
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
