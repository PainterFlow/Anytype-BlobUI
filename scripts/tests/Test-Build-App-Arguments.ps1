$ErrorActionPreference = 'Stop'
$workspaceRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$fixtureRoot = Join-Path (Join-Path $workspaceRoot '.test-run') ('Build-App-Arguments-' + [Guid]::NewGuid().ToString('N'))
$scriptDirectory = Join-Path $fixtureRoot 'scripts'
$projectDirectory = Join-Path $fixtureRoot 'settings-app'
$fixtureScriptPath = Join-Path $scriptDirectory 'Build-App.ps1'
$oldBuildOnly = $env:BLOBUI_BUILD_ONLY
$oldTestFixture = $env:BLOBUI_BUILD_APP_TEST_FIXTURE
$oldLastExitCode = $global:LASTEXITCODE
$global:BlobUIBuildAppTestLaunches = @()
$global:BlobUIBuildAppTestGitCalls = 0
$global:BlobUIBuildAppTestNpmCalls = 0
$env:BLOBUI_BUILD_APP_TEST_FIXTURE = $fixtureRoot

function Assert([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

function git {
    $global:BlobUIBuildAppTestGitCalls++
    $global:LASTEXITCODE = 0
    return 'fixture'
}

function node { }

function npm {
    $global:BlobUIBuildAppTestNpmCalls++
    $global:LASTEXITCODE = 0
}

function cargo {
    $releaseExecutable = Join-Path $env:BLOBUI_BUILD_APP_TEST_FIXTURE 'settings-app/target/release/blobui-settings.exe'
    [IO.Directory]::CreateDirectory((Split-Path -Parent $releaseExecutable)) | Out-Null
    [IO.File]::WriteAllText($releaseExecutable, 'fixture executable')
    $global:LASTEXITCODE = 0
}

function Start-Process {
    param([string]$FilePath, [string[]]$ArgumentList, [string]$WindowStyle)
    $global:BlobUIBuildAppTestLaunches += ,([pscustomobject]@{
        FilePath = $FilePath
        ArgumentList = @($ArgumentList)
        WindowStyle = $WindowStyle
    })
}

try {
    $env:BLOBUI_BUILD_ONLY = '0'
    [IO.Directory]::CreateDirectory($scriptDirectory) | Out-Null
    [IO.Directory]::CreateDirectory($projectDirectory) | Out-Null
    [IO.File]::WriteAllText((Join-Path $projectDirectory 'Cargo.toml'), '[package]')
    Copy-Item -LiteralPath (Join-Path $workspaceRoot 'scripts/Build-App.ps1') -Destination $fixtureScriptPath
    [IO.File]::WriteAllText((Join-Path $scriptDirectory 'Build-Codex.ps1'), '')
    [IO.File]::WriteAllText((Join-Path $scriptDirectory 'Rust-Toolchain.ps1'), '')

    $backgroundRequestPath = Join-Path $fixtureRoot 'settings-app/dist/.blobui-background-next'
    $onboardingRequestPath = Join-Path $fixtureRoot 'settings-app/dist/.blobui-onboarding-next'
    $distributionExecutable = Join-Path $fixtureRoot 'settings-app/dist/blobui-settings.exe'
    & $fixtureScriptPath -b
    Assert (Test-Path -LiteralPath $distributionExecutable -PathType Leaf) '-b without -s must build the distribution executable.'
    Assert (Test-Path -LiteralPath $backgroundRequestPath -PathType Leaf) '-b without -s must queue background mode beside the distribution executable.'
    Assert ($global:BlobUIBuildAppTestLaunches.Count -eq 0) '-b without -s must not launch the Settings app.'
    Assert (-not (Test-Path -LiteralPath $onboardingRequestPath)) '-b alone must not queue onboarding.'

    [IO.File]::WriteAllText($backgroundRequestPath, 'existing request')
    & $fixtureScriptPath -b
    Assert ([IO.File]::ReadAllText($backgroundRequestPath) -ceq 'existing request') 'Repeated -b requests must preserve the pending background request.'
    Assert ($global:BlobUIBuildAppTestLaunches.Count -eq 0) 'Repeated -b requests must not launch the Settings app.'
    Remove-Item -LiteralPath $backgroundRequestPath -Force

    & $fixtureScriptPath --silent
    Assert (Test-Path -LiteralPath $backgroundRequestPath -PathType Leaf) '--silent without -s must queue background mode.'
    Assert ($global:BlobUIBuildAppTestLaunches.Count -eq 0) '--silent without -s must not launch the Settings app.'
    Remove-Item -LiteralPath $backgroundRequestPath -Force

    & $fixtureScriptPath -b -n
    Assert (Test-Path -LiteralPath $backgroundRequestPath -PathType Leaf) '-b -n must queue background mode.'
    Assert (Test-Path -LiteralPath $onboardingRequestPath -PathType Leaf) '-b -n must queue onboarding.'
    Assert ($global:BlobUIBuildAppTestLaunches.Count -eq 0) '-b -n without -s must not launch the Settings app.'
    & $fixtureScriptPath -b -n
    Assert ((Test-Path -LiteralPath $backgroundRequestPath -PathType Leaf) -and (Test-Path -LiteralPath $onboardingRequestPath -PathType Leaf)) 'Repeated -b -n requests must keep both pending requests.'
    Assert ($global:BlobUIBuildAppTestLaunches.Count -eq 0) 'Repeated -b -n requests must not launch the Settings app.'
    Remove-Item -LiteralPath $backgroundRequestPath, $onboardingRequestPath -Force

    & $fixtureScriptPath --silent -s
    Assert ($global:BlobUIBuildAppTestLaunches.Count -eq 1) '--silent -s must launch the Settings app.'
    Assert ($global:BlobUIBuildAppTestLaunches[0].ArgumentList.Count -eq 1 -and $global:BlobUIBuildAppTestLaunches[0].ArgumentList[0] -ceq '--background') '--silent -s must pass --background.'
    Assert ($global:BlobUIBuildAppTestLaunches[0].WindowStyle -ceq 'Hidden') 'Silent launches must use a hidden window.'
    Assert (-not (Test-Path -LiteralPath $backgroundRequestPath)) '--silent -s must not queue background mode.'

    $global:BlobUIBuildAppTestLaunches = @()
    & $fixtureScriptPath -b -s
    Assert ($global:BlobUIBuildAppTestLaunches.Count -eq 1) '-b -s must launch the Settings app.'
    Assert ($global:BlobUIBuildAppTestLaunches[0].ArgumentList.Count -eq 1 -and $global:BlobUIBuildAppTestLaunches[0].ArgumentList[0] -ceq '--background') '-b -s must pass --background.'
    Assert ($global:BlobUIBuildAppTestLaunches[0].WindowStyle -ceq 'Hidden') '-b -s must use a hidden window.'
    Assert (-not (Test-Path -LiteralPath $backgroundRequestPath)) '-b -s must not queue background mode.'

    $global:BlobUIBuildAppTestLaunches = @()
    & $fixtureScriptPath -s --silent -n
    Assert ($global:BlobUIBuildAppTestLaunches.Count -eq 1) '-s --silent -n must launch the Settings app.'
    Assert ($global:BlobUIBuildAppTestLaunches[0].ArgumentList.Count -eq 2) 'Silent onboarding must pass both app arguments.'
    Assert ($global:BlobUIBuildAppTestLaunches[0].ArgumentList[0] -ceq '--onboarding') '-n must continue to pass --onboarding when launching.'
    Assert ($global:BlobUIBuildAppTestLaunches[0].ArgumentList[1] -ceq '--background') '-b must pass --background when launching.'
    Assert (-not (Test-Path -LiteralPath $backgroundRequestPath)) '-s --silent -n must not queue background mode.'
    Assert (-not (Test-Path -LiteralPath $onboardingRequestPath)) '-s --silent -n must not queue onboarding.'

    $global:BlobUIBuildAppTestLaunches = @()
    & $fixtureScriptPath -n
    Assert (Test-Path -LiteralPath $onboardingRequestPath -PathType Leaf) '-n without -s must continue to create the deferred onboarding request.'
    Assert ($global:BlobUIBuildAppTestLaunches.Count -eq 0) '-n without -s must not launch the Settings app.'
    Remove-Item -LiteralPath $onboardingRequestPath -Force

    $global:BlobUIBuildAppTestLaunches = @()
    & $fixtureScriptPath -s -n
    Assert ($global:BlobUIBuildAppTestLaunches.Count -eq 1) 'Legacy -s -n must still launch the Settings app.'
    Assert ($global:BlobUIBuildAppTestLaunches[0].ArgumentList.Count -eq 1 -and $global:BlobUIBuildAppTestLaunches[0].ArgumentList[0] -ceq '--onboarding') 'Legacy -s -n must pass only --onboarding.'
    Assert ($global:BlobUIBuildAppTestLaunches[0].WindowStyle -ceq 'Normal') 'Non-silent launches must keep a normal window.'

    Write-Host 'Build-App silent and onboarding argument checks passed.'
}
finally {
    $env:BLOBUI_BUILD_ONLY = $oldBuildOnly
    $env:BLOBUI_BUILD_APP_TEST_FIXTURE = $oldTestFixture
    $global:LASTEXITCODE = $oldLastExitCode
    Remove-Variable -Name BlobUIBuildAppTestLaunches, BlobUIBuildAppTestGitCalls, BlobUIBuildAppTestNpmCalls -Scope Global -ErrorAction SilentlyContinue
    $expectedPrefix = [IO.Path]::GetFullPath((Join-Path $workspaceRoot '.test-run')) + [IO.Path]::DirectorySeparatorChar
    $resolvedFixtureRoot = [IO.Path]::GetFullPath($fixtureRoot)
    if (-not $resolvedFixtureRoot.StartsWith($expectedPrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Unsafe Build-App argument fixture cleanup path.'
    }
    if (Test-Path -LiteralPath $resolvedFixtureRoot) {
        Remove-Item -LiteralPath $resolvedFixtureRoot -Recurse -Force
    }
}
