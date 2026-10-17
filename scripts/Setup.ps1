param(
    [string]$AnytypeDirectory,
    [string]$AppDirectory,
    [switch]$NoShortcut
)

$ErrorActionPreference = 'Stop'
$sourceDirectory = Split-Path -Parent $PSScriptRoot
Import-Module (Join-Path $PSScriptRoot 'BlobUI.Core.psm1') -Force

# Prompt separately: Anytype's executable folder is not its data folder.
if ([string]::IsNullOrWhiteSpace($AnytypeDirectory)) {
    $defaultAnytype = if ($env:APPDATA) { Join-Path $env:APPDATA 'anytype' } else { '' }
    $AnytypeDirectory = (Read-Host "Anytype data folder (contains custom.css) [$defaultAnytype]").Trim().Trim('"')
    if (-not $AnytypeDirectory) { $AnytypeDirectory = $defaultAnytype }
}
if ([string]::IsNullOrWhiteSpace($AppDirectory)) {
    $defaultApp = if ($env:LOCALAPPDATA) { Join-Path $env:LOCALAPPDATA 'Programs\BlobUI' } else { '' }
    $AppDirectory = (Read-Host "Install BlobUI app in [$defaultApp]").Trim().Trim('"')
    if (-not $AppDirectory) { $AppDirectory = $defaultApp }
}
if (-not $AnytypeDirectory -or -not $AppDirectory) { throw 'Both installation folders are required.' }
$AnytypeDirectory = [IO.Path]::GetFullPath($AnytypeDirectory).TrimEnd([IO.Path]::DirectorySeparatorChar)
$AppDirectory = [IO.Path]::GetFullPath($AppDirectory).TrimEnd([IO.Path]::DirectorySeparatorChar)
if (-not (Test-Path -LiteralPath $AnytypeDirectory -PathType Container)) {
    throw "Anytype data folder does not exist: $AnytypeDirectory. Open Anytype once or select its custom data folder."
}
foreach ($path in @($AnytypeDirectory, $AppDirectory)) {
    if ($path -split '[\\/]' -icontains 'Releases') { throw 'Installation inside a Releases folder is not supported.' }
    if (-not [IO.Path]::GetFileName($path)) { throw 'Choose a folder rather than a drive root.' }
    $ancestor = $path
    while ($ancestor) {
        if ((Test-Path -LiteralPath $ancestor) -and ((Get-Item -LiteralPath $ancestor -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            throw "Installation paths must not contain links or junctions: $ancestor"
        }
        $ancestor = Split-Path -Parent $ancestor
    }
}
foreach ($other in @($AnytypeDirectory, $sourceDirectory)) {
    $other = [IO.Path]::GetFullPath($other).TrimEnd([IO.Path]::DirectorySeparatorChar)
    $separator = [IO.Path]::DirectorySeparatorChar
    if ($AppDirectory.Equals($other, [StringComparison]::OrdinalIgnoreCase) -or
        $AppDirectory.StartsWith($other + $separator, [StringComparison]::OrdinalIgnoreCase) -or
        $other.StartsWith($AppDirectory + $separator, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Choose a BlobUI app folder separate from Anytype and the extracted download.'
    }
}
# Never claim an arbitrary directory or overwrite an existing app install.
if (Test-Path -LiteralPath $AppDirectory) {
    if (-not (Test-Path -LiteralPath $AppDirectory -PathType Container) -or @(Get-ChildItem -LiteralPath $AppDirectory -Force).Count) {
        throw 'The BlobUI app folder must be empty. Choose a new folder, or use Advanced Settings in the existing app to update the theme.'
    }
}
if (-not $NoShortcut -and $env:OS -ne 'Windows_NT') { throw 'Shortcuts require Windows. Use -NoShortcut for command-line validation.' }
$settingsExecutable = Join-Path $sourceDirectory 'settings-app/dist/blobui-settings.exe'
if (-not (Test-Path -LiteralPath $settingsExecutable -PathType Leaf)) {
    throw 'The Rust Settings app executable is missing. Run scripts/Build-Settings.ps1 before installing BlobUI.'
}

$appCreated = $false
$installed = $false
$shortcutPath = $null
$shortcutCreated = $false
try {
    [IO.Directory]::CreateDirectory($AppDirectory) | Out-Null
    $appCreated = $true
    foreach ($name in @('scripts', 'Dev', 'assets', 'BlobUI Settings Space')) {
        Copy-Item -LiteralPath (Join-Path $sourceDirectory $name) -Destination $AppDirectory -Recurse
    }
    Copy-Item -LiteralPath (Join-Path $sourceDirectory 'BlobUI.cmd') -Destination $AppDirectory
    $appDistribution = Join-Path $AppDirectory 'settings-app/dist'
    [IO.Directory]::CreateDirectory($appDistribution) | Out-Null
    Copy-Item -LiteralPath $settingsExecutable -Destination $appDistribution
    Write-BlobUiInstallPointer -SourceDirectory $AppDirectory -InstallDirectory $AnytypeDirectory
    $dataPath = Join-Path $AnytypeDirectory 'blob\data.json'
    if (Test-Path -LiteralPath $dataPath -PathType Leaf) {
        # Reinstalling the app preserves existing settings and encrypted credentials.
        $data = Read-BlobUiDataFile -Path $dataPath
        if (-not ([IO.Path]::GetFullPath([string]$data.installDirectory)).Equals($AnytypeDirectory, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'Existing BlobUI settings belong to another Anytype folder.'
        }
    }
    else {
        $data = [ordered]@{
            schemaVersion = 1; apiVersion = 'v2'; installDirectory = $AnytypeDirectory
            setupMode = 'defaults'; selectedSpace = @{ id = ''; name = '' }; settingsObjectIds = @()
            settings = @{ fontFamily = 'Libron'; codeFontFamily = 'Consolas'; accentColor = '#DE50DE' }
            calendarObjectColors = @{}
        }
    }
    $candidate = Join-Path $AppDirectory 'setup-data.json'
    $css = Join-Path $AppDirectory 'setup-custom.css'
    Write-BlobUiAtomicJson -Path $candidate -InputObject $data
    & (Join-Path $PSScriptRoot 'Compile.ps1') -DataPath $candidate -OutputPath $css
    if (-not $NoShortcut) {
        $programs = [Environment]::GetFolderPath('Programs')
        $shortcutPath = Join-Path $programs 'BlobUI.lnk'
        if (Test-Path -LiteralPath $shortcutPath) { throw 'A BlobUI Start menu shortcut already exists. Remove it or use -NoShortcut.' }
        $shell = New-Object -ComObject WScript.Shell
        $shortcut = $shell.CreateShortcut($shortcutPath)
        $shortcut.TargetPath = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
        $shortcut.Arguments = '-NoProfile -STA -ExecutionPolicy Bypass -File "' + (Join-Path $AppDirectory 'scripts\App.ps1') + '" -InstallDirectory "' + $AnytypeDirectory + '"'
        $shortcut.WorkingDirectory = $AppDirectory
        $shortcut.Save()
        $shortcutCreated = $true
    }
    $existingCss = Join-Path $AnytypeDirectory 'custom.css'
    if (Test-Path -LiteralPath $existingCss -PathType Leaf) {
        $backupDirectory = Join-Path $AnytypeDirectory 'blob/backups'
        foreach ($target in @((Join-Path $AnytypeDirectory 'blob'), $backupDirectory, $existingCss)) {
            if ((Test-Path -LiteralPath $target) -and ((Get-Item -LiteralPath $target -Force).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
                throw "Cannot back up CSS through a link or junction: $target"
            }
        }
        [IO.Directory]::CreateDirectory($backupDirectory) | Out-Null
        Copy-Item -LiteralPath $existingCss -Destination (Join-Path $backupDirectory ('custom-' + [Guid]::NewGuid().ToString('N') + '.css.bak'))
    }
    $null = Invoke-BlobUiInstall -InstallDirectory $AnytypeDirectory -RuntimeSourceDirectory $AppDirectory -AssetsSourceDirectory (Join-Path $AppDirectory 'assets') -CompiledCssPath $css -DataCandidatePath $candidate
    $installed = $true
    Remove-Item -LiteralPath $candidate, $css -Force
    Write-Host "BlobUI installed in $AppDirectory"
    Write-Host "CSS installed in $AnytypeDirectory\custom.css"
    Write-Host 'Open BlobUI from the Start menu or BlobUI.cmd. In Anytype enable File > Apply Custom CSS and press Ctrl+R.'
}
finally {
    if (-not $installed -and $appCreated) {
        Remove-Item -LiteralPath $AppDirectory -Recurse -Force
        if ($shortcutCreated -and (Test-Path -LiteralPath $shortcutPath)) { Remove-Item -LiteralPath $shortcutPath -Force }
    }
}
