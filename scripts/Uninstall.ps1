param(
    [string]$InstallDirectory
)

$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'BlobUI.Core.psm1') -Force

function Normalize-BlobUiUninstallPath {
    param([Parameter(Mandatory = $true)][string]$Path)

    $fullPath = [System.IO.Path]::GetFullPath($Path)
    $pathRoot = [System.IO.Path]::GetPathRoot($fullPath)
    while ($fullPath.Length -gt $pathRoot.Length -and
        ($fullPath.EndsWith([System.IO.Path]::DirectorySeparatorChar) -or
         $fullPath.EndsWith([System.IO.Path]::AltDirectorySeparatorChar))) {
        $fullPath = $fullPath.Substring(0, $fullPath.Length - 1)
    }
    return $fullPath
}

function Test-BlobUiReparsePoint {
    param([Parameter(Mandatory = $true)]$Item)

    return (($Item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0)
}

function Assert-BlobUiUninstallRoot {
    param([Parameter(Mandatory = $true)][string]$Path)

    $root = Normalize-BlobUiUninstallPath -Path $Path
    if (-not (Test-Path -LiteralPath $root -PathType Container)) {
        throw "The selected Anytype data directory does not exist: $root"
    }
    $rootItem = Get-Item -LiteralPath $root -Force -ErrorAction Stop
    if (-not $rootItem.PSIsContainer) {
        throw "The selected Anytype data path is not a directory: $root"
    }
    if (Test-BlobUiReparsePoint -Item $rootItem) {
        throw "The selected Anytype data directory is a reparse point, so BlobUI cannot verify its child paths safely: $root"
    }
    if ([System.IO.Path]::GetPathRoot($root).Equals($root, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "The selected Anytype data directory cannot be a filesystem root: $root"
    }
    $releasePathParts = @($root -split '[\\/]+' | Where-Object {
        $_.Equals('Releases', [System.StringComparison]::OrdinalIgnoreCase)
    })
    if ($releasePathParts.Count -gt 0) {
        throw 'The selected directory is inside a Releases path. BlobUI will not remove a custom.css file from a Releases folder.'
    }
    return $root
}

function Get-BlobUiUninstallTarget {
    param(
        [string]$RequestedInstallDirectory,
        [Parameter(Mandatory = $true)][string]$ScriptDirectory,
        [Parameter(Mandatory = $true)][string]$ScriptRoot
    )

    if (-not [string]::IsNullOrWhiteSpace($RequestedInstallDirectory)) {
        return Assert-BlobUiUninstallRoot -Path $RequestedInstallDirectory
    }

    $scriptRootFullPath = Normalize-BlobUiUninstallPath -Path $ScriptRoot
    $runtimeDirectory = Normalize-BlobUiUninstallPath -Path (Join-Path $ScriptDirectory '..')
    $blobDirectory = Normalize-BlobUiUninstallPath -Path (Join-Path $runtimeDirectory '..')
    $runtimeExpectedScripts = Normalize-BlobUiUninstallPath -Path (Join-Path (Join-Path $blobDirectory 'runtime') 'scripts')
    $runtimeContext = [System.IO.Path]::GetFileName($runtimeDirectory).Equals('runtime', [System.StringComparison]::OrdinalIgnoreCase) -and
        [System.IO.Path]::GetFileName($blobDirectory).Equals('blob', [System.StringComparison]::OrdinalIgnoreCase) -and
        $scriptRootFullPath.Equals($runtimeExpectedScripts, [System.StringComparison]::OrdinalIgnoreCase)
    if ($runtimeContext) {
        return Assert-BlobUiUninstallRoot -Path (Join-Path $blobDirectory '..')
    }

    $sourceDirectory = Normalize-BlobUiUninstallPath -Path (Join-Path $ScriptDirectory '..')
    $pointerPath = Get-BlobUiInstallPointerPath -SourceDirectory $sourceDirectory
    if (Test-Path -LiteralPath $pointerPath) {
        $pointerItem = Get-Item -LiteralPath $pointerPath -Force -ErrorAction Stop
        if ($pointerItem.PSIsContainer) {
            throw "The saved BlobUI install pointer is a directory instead of a file: $pointerPath"
        }
        if (Test-BlobUiReparsePoint -Item $pointerItem) {
            throw "The saved BlobUI install pointer is a reparse point and cannot be read safely: $pointerPath"
        }
        try {
            $pointer = ConvertFrom-Json -InputObject ([System.IO.File]::ReadAllText($pointerPath)) -ErrorAction Stop
            $savedInstallDirectory = [string]$pointer.installDirectory
            if ([string]::IsNullOrWhiteSpace($savedInstallDirectory)) {
                throw 'The saved install directory is missing.'
            }
            $savedInstallDirectory = Normalize-BlobUiUninstallPath -Path $savedInstallDirectory
        }
        catch {
            throw "The saved BlobUI install location is invalid: $pointerPath. Pass -InstallDirectory with the Anytype data directory."
        }
        if (-not (Test-Path -LiteralPath $savedInstallDirectory -PathType Container)) {
            throw "The saved BlobUI install directory no longer exists: $savedInstallDirectory. Pass -InstallDirectory with the Anytype data directory."
        }
        return Assert-BlobUiUninstallRoot -Path $savedInstallDirectory
    }

    if (-not [string]::IsNullOrWhiteSpace($env:APPDATA)) {
        $defaultInstallDirectory = Normalize-BlobUiUninstallPath -Path (Join-Path $env:APPDATA 'anytype')
        $defaultDataPath = Join-Path (Join-Path $defaultInstallDirectory 'blob') 'data.json'
        if ((Test-Path -LiteralPath $defaultInstallDirectory -PathType Container) -and
            (Test-Path -LiteralPath $defaultDataPath -PathType Leaf)) {
            $defaultBlobItem = Get-Item -LiteralPath (Join-Path $defaultInstallDirectory 'blob') -Force -ErrorAction Stop
            $defaultDataItem = Get-Item -LiteralPath $defaultDataPath -Force -ErrorAction Stop
            if (-not $defaultBlobItem.PSIsContainer -or (Test-BlobUiReparsePoint -Item $defaultBlobItem) -or
                (Test-BlobUiReparsePoint -Item $defaultDataItem)) {
                throw "The default BlobUI settings path contains a reparse point and cannot confirm the installation safely: $defaultDataPath"
            }
            try {
                $defaultData = ConvertFrom-Json -InputObject ([System.IO.File]::ReadAllText($defaultDataPath)) -ErrorAction Stop
                $recordedInstallDirectory = [string]$defaultData.installDirectory
                if ([string]::IsNullOrWhiteSpace($recordedInstallDirectory)) {
                    throw 'The saved install directory is missing.'
                }
                $recordedInstallDirectory = Normalize-BlobUiUninstallPath -Path $recordedInstallDirectory
            }
            catch {
                throw "The default BlobUI settings file is invalid: $defaultDataPath. Pass -InstallDirectory to select a different installation."
            }
            if (-not $recordedInstallDirectory.Equals($defaultInstallDirectory, [System.StringComparison]::OrdinalIgnoreCase)) {
                throw "The default BlobUI settings point to '$recordedInstallDirectory', not '$defaultInstallDirectory'. Pass -InstallDirectory with the saved Anytype data directory."
            }
            return Assert-BlobUiUninstallRoot -Path $defaultInstallDirectory
        }
    }

    throw 'No BlobUI installation was found. Pass -InstallDirectory with the Anytype data directory to uninstall it.'
}

function Get-BlobUiUninstallChild {
    param(
        [Parameter(Mandatory = $true)][string]$InstallRoot,
        [Parameter(Mandatory = $true)][string]$Name
    )

    $expectedPath = Normalize-BlobUiUninstallPath -Path (Join-Path $InstallRoot $Name)
    $expectedParent = Normalize-BlobUiUninstallPath -Path ([System.IO.Path]::GetDirectoryName($expectedPath))
    if (-not $expectedParent.Equals($InstallRoot, [System.StringComparison]::OrdinalIgnoreCase) -or
        -not [System.IO.Path]::GetFileName($expectedPath).Equals($Name, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "BlobUI refused an uninstall target outside the selected Anytype directory: $expectedPath"
    }

    $matches = @(Get-ChildItem -LiteralPath $InstallRoot -Force -ErrorAction Stop | Where-Object {
        [string]::Equals($_.Name, $Name, [System.StringComparison]::OrdinalIgnoreCase)
    })
    if ($matches.Count -gt 1) {
        throw "BlobUI found more than one case-insensitive '$Name' entry in the selected Anytype directory."
    }
    if ($matches.Count -eq 0) {
        return $null
    }

    $actualPath = Normalize-BlobUiUninstallPath -Path $matches[0].FullName
    $actualParent = Normalize-BlobUiUninstallPath -Path ([System.IO.Path]::GetDirectoryName($actualPath))
    if (-not $actualParent.Equals($InstallRoot, [System.StringComparison]::OrdinalIgnoreCase) -or
        -not $actualPath.Equals($expectedPath, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "BlobUI refused an uninstall target outside the selected Anytype directory: $actualPath"
    }
    return $matches[0]
}

function Assert-BlobUiUninstallTargets {
    param($CssItem, $BlobItem)

    if ($null -ne $CssItem -and $CssItem.PSIsContainer -and -not (Test-BlobUiReparsePoint -Item $CssItem)) {
        throw "The custom.css uninstall target is a directory, so BlobUI left it untouched: $($CssItem.FullName)"
    }
    if ($null -ne $BlobItem -and -not $BlobItem.PSIsContainer -and -not (Test-BlobUiReparsePoint -Item $BlobItem)) {
        throw "The blob uninstall target is a file, so BlobUI left it untouched: $($BlobItem.FullName)"
    }
}

function Remove-BlobUiUninstallEntry {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$TreeRoot
    )

    $normalizedTreeRoot = Normalize-BlobUiUninstallPath -Path $TreeRoot
    $normalizedPath = Normalize-BlobUiUninstallPath -Path $Path
    $treePrefix = $normalizedTreeRoot.TrimEnd([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
    if (-not $normalizedPath.Equals($normalizedTreeRoot, [System.StringComparison]::OrdinalIgnoreCase) -and
        -not $normalizedPath.StartsWith($treePrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "BlobUI refused to remove a path outside the validated uninstall target: $normalizedPath"
    }

    $item = Get-Item -LiteralPath $normalizedPath -Force -ErrorAction Stop
    if (Test-BlobUiReparsePoint -Item $item) {
        Remove-Item -LiteralPath $item.FullName -Force -ErrorAction Stop
        return
    }
    if ($item.PSIsContainer) {
        $children = @(Get-ChildItem -LiteralPath $item.FullName -Force -ErrorAction Stop)
        foreach ($child in $children) {
            $childPath = Normalize-BlobUiUninstallPath -Path $child.FullName
            $childParent = Normalize-BlobUiUninstallPath -Path ([System.IO.Path]::GetDirectoryName($childPath))
            if (-not $childParent.Equals($normalizedPath, [System.StringComparison]::OrdinalIgnoreCase)) {
                throw "BlobUI refused a child path outside the directory being removed: $childPath"
            }
            Remove-BlobUiUninstallEntry -Path $childPath -TreeRoot $normalizedTreeRoot
        }
        Remove-Item -LiteralPath $item.FullName -Force -ErrorAction Stop
        return
    }
    Remove-Item -LiteralPath $item.FullName -Force -ErrorAction Stop
}

function Remove-BlobUiSourceInstallPointer {
    param(
        [Parameter(Mandatory = $true)][string]$SourceDirectory,
        [Parameter(Mandatory = $true)][string]$UninstalledDirectory
    )

    $pointerPath = Get-BlobUiInstallPointerPath -SourceDirectory $SourceDirectory
    if (-not (Test-Path -LiteralPath $pointerPath -PathType Leaf)) {
        return
    }
    $pointerItem = Get-Item -LiteralPath $pointerPath -Force -ErrorAction Stop
    if (Test-BlobUiReparsePoint -Item $pointerItem) {
        Write-Warning "BlobUI left the source install pointer because it is a reparse point: $pointerPath"
        return
    }

    try {
        $pointer = ConvertFrom-Json -InputObject ([System.IO.File]::ReadAllText($pointerPath)) -ErrorAction Stop
        $savedInstallDirectory = [string]$pointer.installDirectory
        if ([string]::IsNullOrWhiteSpace($savedInstallDirectory)) { return }
        $savedInstallDirectory = Normalize-BlobUiUninstallPath -Path $savedInstallDirectory
    }
    catch {
        return
    }
    if ($savedInstallDirectory.Equals($UninstalledDirectory, [System.StringComparison]::OrdinalIgnoreCase)) {
        Remove-Item -LiteralPath $pointerPath -Force -ErrorAction Stop
    }
}

$sourceScriptsDirectory = Normalize-BlobUiUninstallPath -Path $PSScriptRoot
$sourceDirectory = Normalize-BlobUiUninstallPath -Path (Join-Path $PSScriptRoot '..')
$isInstalledRuntime = [System.IO.Path]::GetFileName($sourceDirectory).Equals('runtime', [System.StringComparison]::OrdinalIgnoreCase) -and
    [System.IO.Path]::GetFileName((Split-Path -Parent $sourceDirectory)).Equals('blob', [System.StringComparison]::OrdinalIgnoreCase) -and
    $sourceScriptsDirectory.Equals(
        (Normalize-BlobUiUninstallPath -Path (Join-Path (Join-Path (Split-Path -Parent $sourceDirectory) 'runtime') 'scripts')),
        [System.StringComparison]::OrdinalIgnoreCase
    )

$installRoot = Get-BlobUiUninstallTarget -RequestedInstallDirectory $InstallDirectory -ScriptDirectory $PSScriptRoot -ScriptRoot $PSScriptRoot
$cssItem = Get-BlobUiUninstallChild -InstallRoot $installRoot -Name 'custom.css'
$blobItem = Get-BlobUiUninstallChild -InstallRoot $installRoot -Name 'blob'
Assert-BlobUiUninstallTargets -CssItem $cssItem -BlobItem $blobItem

if ($null -ne $cssItem) {
    $cssItem = Get-BlobUiUninstallChild -InstallRoot $installRoot -Name 'custom.css'
    Assert-BlobUiUninstallTargets -CssItem $cssItem -BlobItem $null
    if ($null -ne $cssItem) {
        Remove-BlobUiUninstallEntry -Path $cssItem.FullName -TreeRoot $cssItem.FullName
    }
}
if ($null -ne $blobItem) {
    $blobItem = Get-BlobUiUninstallChild -InstallRoot $installRoot -Name 'blob'
    Assert-BlobUiUninstallTargets -CssItem $null -BlobItem $blobItem
    if ($null -ne $blobItem) {
        Remove-BlobUiUninstallEntry -Path $blobItem.FullName -TreeRoot $blobItem.FullName
    }
}
if (-not $isInstalledRuntime) {
    Remove-BlobUiSourceInstallPointer -SourceDirectory $sourceDirectory -UninstalledDirectory $installRoot
}

if ($null -eq $cssItem -and $null -eq $blobItem) {
    Write-Host "No BlobUI custom.css or blob folder existed at $installRoot."
}
else {
    Write-Host "BlobUI was uninstalled from $installRoot."
}
