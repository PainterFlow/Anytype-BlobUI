param(
    [Alias('start')]
    [switch]$s,
    [Alias('close')]
    [switch]$c
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Restore-ProcessEnvironment.ps1')
$workspaceRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$workspacePrefix = $workspaceRoot.TrimEnd('\', '/') + [System.IO.Path]::DirectorySeparatorChar

function Assert-WorkspaceBuildPath([string]$Path) {
    $fullPath = [System.IO.Path]::GetFullPath($Path)
    if (-not $fullPath.StartsWith($workspacePrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Build output must stay inside the workspace: $fullPath"
    }
    $cursor = $fullPath
    while ($cursor.Length -ge $workspaceRoot.Length) {
        $item = Get-Item -LiteralPath $cursor -Force -ErrorAction SilentlyContinue
        if ($null -ne $item -and ($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint)) {
            throw "Build output cannot use a reparse point: $cursor"
        }
        $cursor = [System.IO.Path]::GetDirectoryName($cursor)
    }
    return $fullPath
}

$targetDir = Assert-WorkspaceBuildPath (Join-Path $workspaceRoot 'settings-app\target')
$source = Assert-WorkspaceBuildPath (Join-Path $targetDir 'release\blob-ui.exe')
$output = Assert-WorkspaceBuildPath (Join-Path $workspaceRoot 'settings-app\dist\headless-cli\blob-ui.exe')
$manifest = Join-Path $workspaceRoot 'settings-app\Cargo.toml'

if ($c) {
    foreach ($instance in @(Get-Process -Name 'blob-ui' -ErrorAction SilentlyContinue)) {
        $instancePath = $null
        try { $instancePath = $instance.Path } catch { }
        if ([string]::IsNullOrEmpty($instancePath)) { continue }
        try { $verifiedPath = Assert-WorkspaceBuildPath $instancePath } catch { continue }
        if ([System.IO.Path]::GetFileName($verifiedPath) -ine 'blob-ui.exe') { continue }
        Stop-Process -InputObject $instance
        if (-not $instance.WaitForExit(5000)) { throw "BlobUI Headless process $($instance.Id) did not exit." }
    }
}

$buildEnvironment = @{}
foreach ($name in @('PATH', 'CARGO_HOME', 'RUSTUP_HOME', 'TEMP', 'TMP', 'LIB', 'INCLUDE', 'CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER', 'CARGO_TARGET_DIR')) {
    $buildEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}
Push-Location $workspaceRoot
try {
    . (Join-Path $PSScriptRoot 'Rust-Toolchain.ps1')
    $env:CARGO_TARGET_DIR = $targetDir
    & cargo build --locked --release --no-default-features --features headless-cli --bin blob-ui --manifest-path $manifest
    if ($LASTEXITCODE -ne 0) { throw "The BlobUI Headless build failed with exit code $LASTEXITCODE." }
}
finally {
    Pop-Location
    Restore-ProcessEnvironment -SavedEnvironment $buildEnvironment
}

[System.IO.Directory]::CreateDirectory([System.IO.Path]::GetDirectoryName($output)) | Out-Null
[System.IO.File]::Copy($source, $output, $true)
Write-Host "Built BlobUI Headless at $output"

if ($s) {
    if ([string]::IsNullOrEmpty($env:LOCALAPPDATA)) { throw 'LOCALAPPDATA is unavailable.' }
    $binDirectory = Join-Path $env:LOCALAPPDATA 'Programs\BlobUI-Headless\bin'
    [System.IO.Directory]::CreateDirectory($binDirectory) | Out-Null
    [System.IO.File]::Copy($output, (Join-Path $binDirectory 'blob-ui.exe'), $true)
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $entries = @($userPath -split ';' | Where-Object { $_ })
    $expandedEntries = @($entries | ForEach-Object { [Environment]::ExpandEnvironmentVariables($_).TrimEnd('\', '/') })
    if ($expandedEntries -notcontains $binDirectory.TrimEnd('\', '/')) {
        [Environment]::SetEnvironmentVariable('Path', (($entries + $binDirectory) -join ';'), 'User')
    }
    Write-Host "Installed blob-ui at $binDirectory. Restart Codex so it can use your updated PATH."
}
