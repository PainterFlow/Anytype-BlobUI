param(
    [string]$InstallDirectory,
    [switch]$Background
)

$ErrorActionPreference = 'Stop'
$runtimeDirectory = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$candidates = @(
    (Join-Path $runtimeDirectory 'settings-app.exe'),
    (Join-Path $runtimeDirectory 'settings-app\dist\blobui-settings.exe'),
    (Join-Path $runtimeDirectory 'settings-app\target\release\blobui-settings.exe')
)
$settingsExecutable = $null
foreach ($candidate in $candidates) {
    if (Test-Path -LiteralPath $candidate -PathType Leaf) {
        $item = Get-Item -LiteralPath $candidate -Force -ErrorAction Stop
        if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "BlobUI Settings cannot be launched from a reparse point: $candidate"
        }
        $settingsExecutable = [System.IO.Path]::GetFullPath($candidate)
        break
    }
}

if (-not $settingsExecutable) {
    throw "The BlobUI Settings executable was not found. Run scripts/Build-Settings.ps1 from the source folder, or install a package that includes runtime/settings-app.exe."
}

$oldInstallDirectory = [Environment]::GetEnvironmentVariable('BLOBUI_INSTALL_DIRECTORY', 'Process')
try {
    if (-not [string]::IsNullOrWhiteSpace($InstallDirectory)) {
        [Environment]::SetEnvironmentVariable('BLOBUI_INSTALL_DIRECTORY', [System.IO.Path]::GetFullPath($InstallDirectory), 'Process')
    }
    if ($Background) {
        Start-Process -FilePath $settingsExecutable -ArgumentList '--background' -WindowStyle Hidden | Out-Null
    }
    else {
        Start-Process -FilePath $settingsExecutable -WindowStyle Normal | Out-Null
    }
}
finally {
    if ($null -eq $oldInstallDirectory) {
        # PowerShell converts $null to an empty string for this overload. On
        # .NET 9+ that creates an empty variable instead of removing it.
        [Environment]::SetEnvironmentVariable('BLOBUI_INSTALL_DIRECTORY', [NullString]::Value, 'Process')
    }
    else {
        [Environment]::SetEnvironmentVariable('BLOBUI_INSTALL_DIRECTORY', $oldInstallDirectory, 'Process')
    }
}
