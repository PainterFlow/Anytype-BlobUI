param(
    [Alias('dev')]
    [switch]$d,
    [switch]$PackageRelease
)

$ErrorActionPreference = 'Stop'
$workspaceRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$manifestPath = Join-Path $workspaceRoot 'installer\Cargo.toml'
$distributionDirectory = Join-Path $workspaceRoot 'dist'
$settingsExecutable = Join-Path $workspaceRoot 'settings-app\dist\blobui-settings.exe'
$versionSource = Join-Path $workspaceRoot 'blobui-release.json'
$changelogSource = Join-Path $workspaceRoot 'Changelogs.md'
$changelogImageManifest = Join-Path $workspaceRoot 'Changelogs.yaml'
$buildCommit = & git -C $workspaceRoot rev-parse --short HEAD
if ($LASTEXITCODE -ne 0) { throw 'Could not read the local Git commit.' }
Write-Host "Building installer for blobui-app Dev+$buildCommit"

function Assert-WorkspacePath {
    param([Parameter(Mandatory)][string]$Path)
    $fullPath = [System.IO.Path]::GetFullPath($Path)
    $prefix = $workspaceRoot.TrimEnd([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
    if (-not $fullPath.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)) { throw "Refusing to write outside the workspace: $fullPath" }
    $cursor = $workspaceRoot
    foreach ($part in $fullPath.Substring($prefix.Length).Split([System.IO.Path]::DirectorySeparatorChar, [System.StringSplitOptions]::RemoveEmptyEntries)) {
        $cursor = Join-Path $cursor $part
        $item = Get-Item -LiteralPath $cursor -Force -ErrorAction SilentlyContinue
        if ($null -ne $item -and (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0)) { throw "Refusing to write through a reparse point: $cursor" }
    }
    return $fullPath
}

function Copy-PackageTree {
    param([string]$Source, [string]$Destination)
    $item = Get-Item -LiteralPath $Source -Force
    if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) { throw "Refusing to package a reparse point: $Source" }
    if ($item.PSIsContainer) {
        [IO.Directory]::CreateDirectory($Destination) | Out-Null
        foreach ($child in Get-ChildItem -LiteralPath $Source -Force) {
            if ($child.Name.StartsWith('.blobui', [StringComparison]::OrdinalIgnoreCase)) { continue }
            if (($child.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw "Refusing to package a reparse point: $($child.FullName)" }
            Copy-PackageTree $child.FullName (Join-Path $Destination $child.Name)
        }
    } else { Copy-Item -LiteralPath $Source -Destination $Destination }
}

if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) { throw "Installer Cargo manifest was not found: $manifestPath" }
if (-not (Test-Path -LiteralPath $versionSource -PathType Leaf)) { throw "Release version source was not found: $versionSource" }
if ($PackageRelease -and (-not (Test-Path -LiteralPath $changelogSource -PathType Leaf) -or -not (Test-Path -LiteralPath $changelogImageManifest -PathType Leaf))) { throw 'Release notes must include Changelogs.md and Changelogs.yaml.' }
if ($PackageRelease -and -not (Test-Path -LiteralPath $settingsExecutable -PathType Leaf)) { throw "Build BlobUI Settings separately before packaging: scripts/Build-Settings.ps1 -s -c" }

$releaseManifest = Get-Content -LiteralPath $versionSource -Raw | ConvertFrom-Json
if ($releaseManifest.formatVersion -ne 1 -or $releaseManifest.version -cnotmatch '^[0-9][0-9A-Za-z.+-]{0,63}$') { throw 'Invalid release metadata in blobui-release.json.' }
$releaseVersion = $releaseManifest.version

$savedEnvironment = @{}
foreach ($name in @('PATH', 'CARGO_HOME', 'RUSTUP_HOME', 'TEMP', 'TMP', 'LIB', 'INCLUDE', 'CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER', 'CARGO_TARGET_DIR', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS')) { $savedEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
Push-Location $workspaceRoot
try {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        $toolchainRoot = Join-Path $workspaceRoot '.test-run\toolchain'
        $rustRoot = Join-Path $toolchainRoot 'rustup\toolchains\stable-x86_64-pc-windows-msvc'
        $rustBin = Join-Path $rustRoot 'bin'
        if (-not (Test-Path -LiteralPath (Join-Path $rustBin 'cargo.exe') -PathType Leaf)) { throw 'Cargo was not found on PATH or in the workspace toolchain.' }
        $env:CARGO_HOME = Join-Path $toolchainRoot 'cargo'
        $env:RUSTUP_HOME = Join-Path $toolchainRoot 'rustup'
        $env:TEMP = Join-Path $toolchainRoot 'temp'; $env:TMP = $env:TEMP
        [IO.Directory]::CreateDirectory($env:TEMP) | Out-Null
        $env:PATH = "$rustBin;$env:PATH"
        $visualStudioRoot = Join-Path ${env:ProgramFiles} 'Microsoft Visual Studio'
        $vcRoot = Get-ChildItem -Path (Join-Path $visualStudioRoot '*\*\VC\Tools\MSVC\*') -Directory -ErrorAction SilentlyContinue | Where-Object { Test-Path -LiteralPath (Join-Path $_.FullName 'bin\Hostx64\x64\cl.exe') } | Sort-Object Name -Descending | Select-Object -First 1
        $sdkRoot = Join-Path $toolchainRoot 'sdk'
        $sdkHeaders = Get-ChildItem -LiteralPath (Join-Path $sdkRoot 'microsoft.windows.sdk.cpp\c\Include') -Directory -ErrorAction SilentlyContinue | Sort-Object Name -Descending | Select-Object -First 1
        if (-not $vcRoot -or -not $sdkHeaders) { throw 'The workspace Rust toolchain requires Visual C++ build tools and the workspace Windows SDK.' }
        $sdkBin = Join-Path $sdkRoot "microsoft.windows.sdk.cpp\c\bin\$($sdkHeaders.Name)\x64"
        $env:PATH = "$($vcRoot.FullName)\bin\Hostx64\x64;$sdkBin;$env:PATH"
        $env:LIB = "$($vcRoot.FullName)\lib\x64;$sdkRoot\microsoft.windows.sdk.cpp.x64\c\um\x64;$sdkRoot\microsoft.windows.sdk.cpp.x64\c\ucrt\x64;$env:LIB"
        $env:INCLUDE = "$($vcRoot.FullName)\include;$($sdkHeaders.FullName)\ucrt;$($sdkHeaders.FullName)\shared;$($sdkHeaders.FullName)\um;$($sdkHeaders.FullName)\winrt;$env:INCLUDE"
        $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = Join-Path $rustRoot 'lib\rustlib\x86_64-pc-windows-msvc\bin\rust-lld.exe'
    }
    # Keep build output local and include the C runtime in the executable.
    # An explicit target keeps these flags away from host build/proc-macro crates.
    $env:CARGO_TARGET_DIR = Assert-WorkspacePath (Join-Path $workspaceRoot 'installer\target')
    $env:RUSTFLAGS = '-C target-feature=+crt-static'
    [Environment]::SetEnvironmentVariable('CARGO_ENCODED_RUSTFLAGS', $null, 'Process')
    $cargoArgs = @('build', '--locked', '--release', '--quiet', '--target', 'x86_64-pc-windows-msvc', '--manifest-path', $manifestPath)
    if ($d) { $cargoArgs += @('--features', 'development') }
    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) { throw "Native installer build failed with exit code $LASTEXITCODE." }
}
finally {
    Pop-Location
    foreach ($name in $savedEnvironment.Keys) { [Environment]::SetEnvironmentVariable($name, $savedEnvironment[$name], 'Process') }
}

$null = Assert-WorkspacePath $distributionDirectory
[IO.Directory]::CreateDirectory($distributionDirectory) | Out-Null
$targetName = if ($d) { 'blobui-installer-dev.exe' } else { 'blobui-installer.exe' }
$sourceExecutable = Join-Path $workspaceRoot 'installer\target\x86_64-pc-windows-msvc\release\blobui-installer.exe'
$destinationExecutable = Join-Path $distributionDirectory $targetName
$null = Assert-WorkspacePath $destinationExecutable
if (-not (Test-Path -LiteralPath $sourceExecutable -PathType Leaf)) { throw "Cargo did not create the installer executable: $sourceExecutable" }
Copy-Item -LiteralPath $sourceExecutable -Destination $destinationExecutable -Force
Write-Host "Native installer built: $destinationExecutable"

if ($PackageRelease) {
    $packageRoot = Join-Path $distributionDirectory ('.blobui-package-' + [Guid]::NewGuid().ToString('N'))
    $packagePath = Join-Path $distributionDirectory 'blobui-windows.zip'
    $tempArchive = Join-Path $distributionDirectory ('.blobui-windows-' + [Guid]::NewGuid().ToString('N') + '.zip')
    $null = Assert-WorkspacePath $packageRoot; $null = Assert-WorkspacePath $packagePath; $null = Assert-WorkspacePath $tempArchive
    try {
        [IO.Directory]::CreateDirectory($packageRoot) | Out-Null
        foreach ($name in @('Dev', 'assets', 'settings-app\dist')) { [IO.Directory]::CreateDirectory((Join-Path $packageRoot $name)) | Out-Null }
        Copy-PackageTree (Join-Path $workspaceRoot 'Dev') (Join-Path $packageRoot 'Dev')
        Copy-PackageTree (Join-Path $workspaceRoot 'assets') (Join-Path $packageRoot 'assets')
        Copy-Item -LiteralPath $changelogSource -Destination (Join-Path $packageRoot 'Changelogs.md')
        Copy-Item -LiteralPath $changelogImageManifest -Destination (Join-Path $packageRoot 'Changelogs.yaml')
        Copy-Item -LiteralPath $settingsExecutable -Destination (Join-Path $packageRoot 'settings-app\dist\blobui-settings.exe')
        $releaseManifest = @{ formatVersion = 1; version = $releaseVersion } | ConvertTo-Json -Compress
        [IO.File]::WriteAllText((Join-Path $packageRoot 'blobui-release.json'), $releaseManifest + "`n", [Text.UTF8Encoding]::new($false))
        & $destinationExecutable --no-pause --validate-package $packageRoot
        if ($LASTEXITCODE -ne 0) { throw 'Release notes or another packaged payload failed validation; no ZIP was created.' }
        Add-Type -AssemblyName System.IO.Compression
        Add-Type -AssemblyName System.IO.Compression.FileSystem
        $archive = [IO.Compression.ZipFile]::Open($tempArchive, [IO.Compression.ZipArchiveMode]::Create)
        try {
            $prefix = [IO.Path]::GetFullPath($packageRoot).TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
            foreach ($file in Get-ChildItem -LiteralPath $packageRoot -File -Recurse -Force) {
                if (($file.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw "Refusing to archive a reparse point: $($file.FullName)" }
                $relative = $file.FullName.Substring($prefix.Length).Replace('\', '/')
                [IO.Compression.ZipFileExtensions]::CreateEntryFromFile($archive, $file.FullName, $relative, [IO.Compression.CompressionLevel]::Optimal) | Out-Null
            }
        } finally { $archive.Dispose() }
        Move-Item -LiteralPath $tempArchive -Destination $packagePath -Force
        Write-Host "Native installer release package created: $packagePath"
    }
    finally {
        if (Test-Path -LiteralPath $packageRoot) { Remove-Item -LiteralPath $packageRoot -Recurse -Force }
        if (Test-Path -LiteralPath $tempArchive) { Remove-Item -LiteralPath $tempArchive -Force }
    }
}
