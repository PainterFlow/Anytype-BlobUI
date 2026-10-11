param()

$ErrorActionPreference = 'Stop'

function Get-InstallDirectory {
    $defaultDirectory = $null
    if (-not [string]::IsNullOrWhiteSpace($env:APPDATA)) {
        $defaultDirectory = Join-Path $env:APPDATA 'anytype'
    }

    while ($true) {
        if ($defaultDirectory) {
            Write-Host "A) Automatic (default): $defaultDirectory"
        }
        else {
            Write-Host 'A) Automatic (default): unavailable because APPDATA is not set'
        }
        Write-Host 'M) Enter a directory manually'

        $selection = (Read-Host 'Install location [A/M] (default A)').Trim().ToUpperInvariant()
        if ([string]::IsNullOrWhiteSpace($selection)) {
            $selection = 'A'
        }

        switch ($selection) {
            'A' {
                if (-not $defaultDirectory) {
                    Write-Host 'Automatic detection needs APPDATA. Select M to enter the Anytype directory.'
                    continue
                }

                if (Test-Path -LiteralPath $defaultDirectory -PathType Container) {
                    return $defaultDirectory
                }

                Write-Host "The detected directory does not exist: $defaultDirectory"
                Write-Host 'Select M to enter the Anytype directory manually.'
            }
            'M' {
                $manualDirectory = (Read-Host 'Enter the full Anytype data directory path').Trim().Trim('"')
                if ([string]::IsNullOrWhiteSpace($manualDirectory)) {
                    throw 'No directory was entered.'
                }
                return $manualDirectory
            }
            default {
                Write-Host 'Choose A or M.'
            }
        }
    }
}

function Get-UniqueBackupPath {
    param(
        [Parameter(Mandatory = $true)][string]$Directory,
        [Parameter(Mandatory = $true)][string]$FileName
    )

    $stamp = Get-Date -Format 'yyyyMMdd-HHmmssfff'
    $candidate = Join-Path $Directory "$FileName.$stamp.bak"
    $counter = 1
    while (Test-Path -LiteralPath $candidate) {
        $candidate = Join-Path $Directory "$FileName.$stamp-$counter.bak"
        $counter++
    }
    return $candidate
}

function Install-BlobUi {
    param(
        [Parameter(Mandatory = $true)][string]$InstallDirectory,
        [Parameter(Mandatory = $true)][string]$SourceDirectory
    )

    $installDirectory = [System.IO.Path]::GetFullPath($InstallDirectory)
    if (-not (Test-Path -LiteralPath $installDirectory -PathType Container)) {
        throw "The selected Anytype directory does not exist: $installDirectory"
    }

    $cssSource = Join-Path $SourceDirectory 'Dev\custom.css'
    $assetsSource = Join-Path $SourceDirectory 'assets'
    $bannerSource = Join-Path $assetsSource 'spacebanner\banner.png'

    if (-not (Test-Path -LiteralPath $cssSource -PathType Leaf)) {
        throw "Development CSS was not found: $cssSource"
    }
    if (-not (Test-Path -LiteralPath $assetsSource -PathType Container)) {
        throw "Assets directory was not found: $assetsSource"
    }
    if (-not (Test-Path -LiteralPath $bannerSource -PathType Leaf)) {
        throw "Space banner asset was not found: $bannerSource"
    }

    $cssDestination = Join-Path $installDirectory 'custom.css'
    $blobDirectory = Join-Path $installDirectory 'blob'
    $assetDestination = Join-Path $blobDirectory 'assets'
    $bannerDestination = Join-Path $assetDestination 'spacebanner\banner.png'
    $bannerUri = [System.Uri]::new($bannerDestination).AbsoluteUri

    $cssContent = [System.IO.File]::ReadAllText($cssSource)
    $headerPattern = '(?m)^(?<prefix>[ \t]*--header-image[ \t]*:[ \t]*url\()[^;]*(?<suffix>\);[ \t]*\r?)$'
    $headerMatches = [regex]::Matches($cssContent, $headerPattern)
    if ($headerMatches.Count -ne 1) {
        throw "Expected one --header-image URL in Dev\custom.css; found $($headerMatches.Count)."
    }

    # Use an absolute file URI so Anytype can resolve the banner from its data directory.
    $generatedCss = [regex]::Replace(
        $cssContent,
        $headerPattern,
        ('${prefix}"' + $bannerUri + '"${suffix}')
    )

    $backupDirectory = Join-Path $blobDirectory 'backups'

    New-Item -ItemType Directory -Path $blobDirectory -Force | Out-Null
    if (Test-Path -LiteralPath $cssDestination -PathType Leaf) {
        New-Item -ItemType Directory -Path $backupDirectory -Force | Out-Null
        $backupPath = Get-UniqueBackupPath -Directory $backupDirectory -FileName 'custom.css'
        Copy-Item -LiteralPath $cssDestination -Destination $backupPath | Out-Null
        Write-Host "Existing CSS backed up to: $backupPath"
    }

    New-Item -ItemType Directory -Path $assetDestination -Force | Out-Null
    $assetItems = @(Get-ChildItem -LiteralPath $assetsSource -Force)
    if ($assetItems.Count -eq 0) {
        throw "Assets directory is empty: $assetsSource"
    }
    foreach ($assetItem in $assetItems) {
        Copy-Item -LiteralPath $assetItem.FullName -Destination $assetDestination -Recurse -Force | Out-Null
    }

    if (-not (Test-Path -LiteralPath $bannerDestination -PathType Leaf)) {
        throw "Banner copy failed: $bannerDestination"
    }

    $utf8WithoutBom = [System.Text.UTF8Encoding]::new($false)
    [System.IO.File]::WriteAllText($cssDestination, $generatedCss, $utf8WithoutBom)

    Write-Host "Installed CSS: $cssDestination"
    Write-Host "Installed assets: $assetDestination"
}

Write-Host 'Anytype Instillation'
try {
    $installDirectory = Get-InstallDirectory
    Install-BlobUi -InstallDirectory $installDirectory -SourceDirectory $PSScriptRoot
    Write-Host 'Installation complete.'
}
catch {
    Write-Host "Installation failed: $($_.Exception.Message)" -ForegroundColor Red
    throw
}
