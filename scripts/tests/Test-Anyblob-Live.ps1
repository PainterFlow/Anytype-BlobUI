param(
    [Parameter(Mandatory=$true)][string]$TestExecutable,
    [string]$AnytypeDirectory=(Join-Path $env:APPDATA 'anytype')
)
$ErrorActionPreference='Stop'
$workspaceRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$testPath=[IO.Path]::GetFullPath($TestExecutable)
$workspacePrefix=$workspaceRoot.TrimEnd('\','/')+[IO.Path]::DirectorySeparatorChar
if(-not $testPath.StartsWith($workspacePrefix,[StringComparison]::OrdinalIgnoreCase)){throw 'Use a test executable built inside this workspace.'}
if(-not(Test-Path -LiteralPath $testPath -PathType Leaf)){throw 'Build the Settings native tests before invoking live validation.'}
$fixture=Join-Path $workspaceRoot '.test-run/anyblob-live-content'
. (Join-Path $workspaceRoot 'scripts/Workspace-Build-Paths.ps1')
Assert-WorkspaceBuildPath $fixture
$profile=Join-Path $fixture 'profile'
$blob=Join-Path $profile 'anytype/blob'
$credential=Join-Path $blob 'credentials/anytype-api-key.dpapi'
$names=@('APPDATA','LOCALAPPDATA','TEMP','TMP','BLOBUI_INSTALL_DIRECTORY','BLOBUI_ANYBLOB_LIVE_TEST')
$saved=@{}
foreach($name in $names){$saved[$name]=[Environment]::GetEnvironmentVariable($name,'Process')}
try {
    New-Item -ItemType Directory -Force -Path (Join-Path $blob 'credentials'),(Join-Path $profile 'local'),(Join-Path $profile 'temp') | Out-Null
    $connection=(Get-Content -LiteralPath (Join-Path $AnytypeDirectory 'blob/data.json') -Raw | ConvertFrom-Json).apiConnection
    @{apiConnection=$connection} | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $blob 'data.json') -Encoding utf8
    Copy-Item -LiteralPath (Join-Path $AnytypeDirectory 'blob/credentials/anytype-api-key.dpapi') -Destination $credential -Force
    $env:APPDATA=$profile
    $env:LOCALAPPDATA=Join-Path $profile 'local'
    $env:TEMP=Join-Path $profile 'temp';$env:TMP=$env:TEMP
    $env:BLOBUI_INSTALL_DIRECTORY=Join-Path $profile 'anytype'
    $env:BLOBUI_ANYBLOB_LIVE_TEST='1'
    $stdout=Join-Path $fixture 'stdout.txt';$stderr=Join-Path $fixture 'stderr.txt'
    $process=Start-Process -FilePath $testPath -ArgumentList @('--ignored','--exact','anyblob::live_tests::codex_test_content_live','--nocapture') -WindowStyle Hidden -PassThru -Wait -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    Get-Content -LiteralPath $stdout
    Get-Content -LiteralPath $stderr
    if($process.ExitCode -ne 0){throw "Codex Test live validation failed with exit code $($process.ExitCode). Inspect the persistent workspace ledger before retrying."}
} finally {
    if(Test-Path -LiteralPath $credential){Remove-Item -LiteralPath $credential -Force}
    foreach($name in $names){[Environment]::SetEnvironmentVariable($name,$saved[$name],'Process')}
}
