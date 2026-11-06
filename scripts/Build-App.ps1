param(
    [switch]$s,
    [switch]$c,
    [Alias('n')]
    [switch]$Onboarding,
    [switch]$b,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$RemainingArguments
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Restore-ProcessEnvironment.ps1')
if ($null -ne $RemainingArguments) {
    foreach ($argument in $RemainingArguments) {
        if ($argument -ceq '--silent') {
            $b = $true
            continue
        }
        throw "Unknown Build-App argument: $argument"
    }
}

$workspaceRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$projectDirectory = Join-Path $workspaceRoot 'settings-app'
$manifestPath = Join-Path $projectDirectory 'Cargo.toml'
$releaseExecutable = Join-Path $projectDirectory 'target\release\blobui-settings.exe'
$distributionDirectory = Join-Path $projectDirectory 'dist'
$distributionExecutable = Join-Path $distributionDirectory 'blobui-settings.exe'
$buildCommit = & git -C $workspaceRoot rev-parse --short HEAD
if ($LASTEXITCODE -ne 0) { throw 'Could not read the local Git commit.' }
Write-Host "Building blobui-app Dev+$buildCommit"

if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
    throw "The BlobUI Settings Rust project was not found: $manifestPath"
}

if ($c -and $env:BLOBUI_BUILD_ONLY -ne '1') {
    $oldInstances = @(Get-Process -Name 'blobui-settings', 'settings-app' -ErrorAction SilentlyContinue)
    $closedInstances = 0
    foreach ($instance in $oldInstances) {
        $instancePath = $null
        try { $instancePath = $instance.Path } catch { }
        if ([string]::IsNullOrWhiteSpace($instancePath)) {
            Write-Host "Skipped Settings process $($instance.Id) because its executable path could not be verified."
            continue
        }
        $resolvedInstancePath = [System.IO.Path]::GetFullPath($instancePath)
        $workspacePrefix = $workspaceRoot.TrimEnd([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
        if (-not $resolvedInstancePath.StartsWith($workspacePrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
            Write-Host "Skipped Settings process $($instance.Id) outside this workspace."
            continue
        }
        Stop-Process -Id $instance.Id -Force -ErrorAction Stop
        $instance.WaitForExit()
        $closedInstances++
    }
    if ($closedInstances -gt 0) {
        Write-Host "Closed $closedInstances existing workspace BlobUI Settings instance(s)."
    }
}

& (Join-Path $PSScriptRoot 'Build-Codex.ps1')
$nodeExecutable = Get-Command node -ErrorAction SilentlyContinue
$npmExecutable = Get-Command npm -ErrorAction SilentlyContinue
if ($null -eq $nodeExecutable -or $null -eq $npmExecutable) {
    throw 'Node.js and npm are required to build the Settings frontend.'
}
Push-Location $projectDirectory
try {
    $workspaceNpmCache = Join-Path $workspaceRoot '.test-run\npm-cache'
    & npm ci --prefer-offline --no-audit --no-fund --cache $workspaceNpmCache
    if ($LASTEXITCODE -ne 0) {
        throw "The Settings frontend dependency install failed with exit code $LASTEXITCODE."
    }
    & npm run build
    if ($LASTEXITCODE -ne 0) {
        throw "The Settings frontend build failed with exit code $LASTEXITCODE."
    }
}
finally {
    Pop-Location
}
$buildEnvironment = @{}
foreach ($name in @('PATH', 'CARGO_HOME', 'RUSTUP_HOME', 'TEMP', 'TMP', 'LIB', 'INCLUDE', 'CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER', 'CARGO_TARGET_DIR')) {
    $buildEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}
Push-Location $workspaceRoot
try {
    . (Join-Path $PSScriptRoot 'Rust-Toolchain.ps1')
    $env:CARGO_TARGET_DIR = Join-Path $projectDirectory 'target'
    & cargo build --locked --release --quiet --manifest-path $manifestPath
    if ($LASTEXITCODE -ne 0) {
        throw "The BlobUI Settings build failed with exit code $LASTEXITCODE."
    }
}
finally {
    Pop-Location
    Restore-ProcessEnvironment -SavedEnvironment $buildEnvironment
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
try {
    Copy-Item -LiteralPath $releaseExecutable -Destination $distributionExecutable -Force
}
catch [System.IO.IOException] {
    throw "The app compiled, but its distribution executable could not be replaced. Fully quit BlobUI Settings with Ctrl+Q and rerun this script. Built executable: $releaseExecutable. $($_.Exception.Message)"
}
Write-Host "BlobUI Settings was built at $distributionExecutable"
Write-Host 'Run settings-app/dist/blobui-settings.exe, or package it with scripts/Build-Installer.ps1 -PackageRelease.'

if ($b -and -not $s) {
    $backgroundRequestPath = Join-Path $distributionDirectory '.blobui-background-next'
    $backgroundRequest = Get-Item -LiteralPath $backgroundRequestPath -Force -ErrorAction SilentlyContinue
    if ($null -ne $backgroundRequest -and
        (($backgroundRequest.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0)) {
        throw "The pending background request is a reparse point: $backgroundRequestPath"
    }
    if ($null -ne $backgroundRequest -and $backgroundRequest.PSIsContainer) {
        throw "The pending background request is not a file: $backgroundRequestPath"
    }
    if ($null -eq $backgroundRequest) {
        [System.IO.File]::WriteAllText($backgroundRequestPath, '')
    }
    Write-Host 'The next BlobUI Settings launch will start in the background.'
}

if ($Onboarding -and -not $s) {
    $onboardingRequestPath = Join-Path $distributionDirectory '.blobui-onboarding-next'
    $onboardingRequest = Get-Item -LiteralPath $onboardingRequestPath -Force -ErrorAction SilentlyContinue
    if ($null -ne $onboardingRequest -and
        (($onboardingRequest.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0)) {
        throw "The pending onboarding request is a reparse point: $onboardingRequestPath"
    }
    if ($null -ne $onboardingRequest -and $onboardingRequest.PSIsContainer) {
        throw "The pending onboarding request is not a file: $onboardingRequestPath"
    }
    if ($null -eq $onboardingRequest) {
        [System.IO.File]::WriteAllText($onboardingRequestPath, '')
    }
    Write-Host 'The next BlobUI Settings launch will open onboarding.'
}

if ($s -and $env:BLOBUI_BUILD_ONLY -ne '1') {
    $launchArguments = @{
        FilePath = $distributionExecutable
        WindowStyle = 'Normal'
    }
    if ($b) { $launchArguments.WindowStyle = 'Hidden' }
    $applicationArguments = @()
    if ($Onboarding) { $applicationArguments += '--onboarding' }
    if ($b) { $applicationArguments += '--background' }
    if ($applicationArguments.Count -gt 0) {
        $launchArguments.ArgumentList = $applicationArguments
    }
    Start-Process @launchArguments | Out-Null
}
