param()

$ErrorActionPreference = 'Stop'
$workspaceRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$projectDirectory = Join-Path $workspaceRoot 'settings-app'
$manifestPath = Join-Path $projectDirectory 'Cargo.toml'
$releaseExecutable = Join-Path $projectDirectory 'target\release\blobui-settings.exe'
$distributionDirectory = Join-Path $projectDirectory 'dist'
$distributionExecutable = Join-Path $distributionDirectory 'blobui-settings.exe'

if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
    throw "The BlobUI Settings Rust project was not found: $manifestPath"
}

Push-Location $workspaceRoot
try {
    & cargo build --locked --release --manifest-path $manifestPath
    if ($LASTEXITCODE -ne 0) {
        throw "The BlobUI Settings build failed with exit code $LASTEXITCODE."
    }
}
finally {
    Pop-Location
}

if (-not (Test-Path -LiteralPath $releaseExecutable -PathType Leaf)) {
    throw "Cargo reported success but the settings executable was not created: $releaseExecutable"
}
$distributionItem = Get-Item -LiteralPath $distributionDirectory -Force -ErrorAction SilentlyContinue
if ($null -ne $distributionItem -and
    (($distributionItem.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0)) {
    throw "The settings app distribution folder is a reparse point: $distributionDirectory"
}
[System.IO.Directory]::CreateDirectory($distributionDirectory) | Out-Null
Copy-Item -LiteralPath $releaseExecutable -Destination $distributionExecutable -Force
Write-Host "BlobUI Settings was built at $distributionExecutable"
Write-Host 'Run scripts/Install.ps1 or scripts/Update.ps1 to package it with BlobUI, or launch it with scripts/Settings.ps1.'
