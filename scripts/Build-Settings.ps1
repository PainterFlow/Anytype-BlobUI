param(
    [switch]$s,
    [switch]$c,
    [Alias('n')]
    [switch]$Onboarding
)

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

if ($c) {
    $oldInstances = @(Get-Process -Name 'blobui-settings', 'settings-app' -ErrorAction SilentlyContinue)
    foreach ($instance in $oldInstances) {
        Stop-Process -Id $instance.Id -Force -ErrorAction Stop
        $instance.WaitForExit()
    }
    if ($oldInstances.Count -gt 0) {
        Write-Host "Closed $($oldInstances.Count) existing BlobUI Settings instance(s)."
    }
}

$buildEnvironment = @{}
foreach ($name in @('PATH', 'CARGO_HOME', 'RUSTUP_HOME', 'TEMP', 'TMP', 'LIB', 'INCLUDE', 'CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER')) {
    $buildEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}
Push-Location $workspaceRoot
try {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        $toolchainRoot = Join-Path $workspaceRoot '.test-run\toolchain'
        $rustRoot = Join-Path $toolchainRoot 'rustup\toolchains\stable-x86_64-pc-windows-msvc'
        $rustBin = Join-Path $rustRoot 'bin'
        if (-not (Test-Path -LiteralPath (Join-Path $rustBin 'cargo.exe') -PathType Leaf)) {
            throw 'Cargo was not found on PATH or in the workspace toolchain. Install Rust or add its cargo bin folder to PATH, then reopen PowerShell.'
        }
        $env:CARGO_HOME = Join-Path $toolchainRoot 'cargo'
        $env:RUSTUP_HOME = Join-Path $toolchainRoot 'rustup'
        $env:TEMP = Join-Path $toolchainRoot 'temp'
        $env:TMP = $env:TEMP
        [System.IO.Directory]::CreateDirectory($env:TEMP) | Out-Null
        $env:PATH = "$rustBin;$env:PATH"

        # The workspace toolchain uses the installed Visual C++ compiler and its local SDK.
        $visualStudioRoot = Join-Path ${env:ProgramFiles} 'Microsoft Visual Studio'
        $vcRoot = Get-ChildItem -Path (Join-Path $visualStudioRoot '*\*\VC\Tools\MSVC\*') -Directory -ErrorAction SilentlyContinue |
            Where-Object { Test-Path -LiteralPath (Join-Path $_.FullName 'bin\Hostx64\x64\cl.exe') } |
            Sort-Object Name -Descending | Select-Object -First 1
        $sdkRoot = Join-Path $toolchainRoot 'sdk'
        $sdkHeaders = Get-ChildItem -LiteralPath (Join-Path $sdkRoot 'microsoft.windows.sdk.cpp\c\Include') -Directory -ErrorAction SilentlyContinue |
            Sort-Object Name -Descending | Select-Object -First 1
        if (-not $vcRoot -or -not $sdkHeaders) {
            throw 'The workspace Rust toolchain requires Visual C++ build tools and the workspace Windows SDK, but one is missing.'
        }
        $sdkBin = Join-Path $sdkRoot "microsoft.windows.sdk.cpp\c\bin\$($sdkHeaders.Name)\x64"
        $env:PATH = "$($vcRoot.FullName)\bin\Hostx64\x64;$sdkBin;$env:PATH"
        $env:LIB = "$($vcRoot.FullName)\lib\x64;$sdkRoot\microsoft.windows.sdk.cpp.x64\c\um\x64;$sdkRoot\microsoft.windows.sdk.cpp.x64\c\ucrt\x64;$env:LIB"
        $env:INCLUDE = "$($vcRoot.FullName)\include;$($sdkHeaders.FullName)\ucrt;$($sdkHeaders.FullName)\shared;$($sdkHeaders.FullName)\um;$($sdkHeaders.FullName)\winrt;$env:INCLUDE"
        $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = Join-Path $rustRoot 'lib\rustlib\x86_64-pc-windows-msvc\bin\rust-lld.exe'
        Write-Host 'Using the workspace Rust toolchain.'
    }
    & cargo build --locked --release --manifest-path $manifestPath
    if ($LASTEXITCODE -ne 0) {
        throw "The BlobUI Settings build failed with exit code $LASTEXITCODE."
    }
}
finally {
    Pop-Location
    foreach ($name in $buildEnvironment.Keys) {
        [Environment]::SetEnvironmentVariable($name, $buildEnvironment[$name], 'Process')
    }
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

if ($s) {
    $launchArguments = @{
        FilePath = $distributionExecutable
        WindowStyle = 'Normal'
    }
    if ($Onboarding) { $launchArguments.ArgumentList = @('--onboarding') }
    Start-Process @launchArguments | Out-Null
}
