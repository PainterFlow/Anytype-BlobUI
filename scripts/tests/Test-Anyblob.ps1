param([switch]$SkipBuild)
$ErrorActionPreference='Stop'
$workspaceRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
. (Join-Path $workspaceRoot 'scripts/Rust-Toolchain.ps1')
$fixture=Join-Path $workspaceRoot '.test-run/anyblob-validation'
[IO.Directory]::CreateDirectory($fixture) | Out-Null
$env:APPDATA=Join-Path $fixture 'roaming'
$env:LOCALAPPDATA=Join-Path $fixture 'local'
$env:TEMP=Join-Path $fixture 'temp';$env:TMP=$env:TEMP
foreach($folder in @($env:APPDATA,$env:LOCALAPPDATA,$env:TEMP)) { [IO.Directory]::CreateDirectory($folder) | Out-Null }
$env:ANYBLOB_BRIDGE_INSTANCE='fixture-'+[Guid]::NewGuid().ToString('N')
$env:CARGO_NET_OFFLINE='true'
& (Join-Path $workspaceRoot 'scripts/Build-MCP.ps1') -Test
if($LASTEXITCODE -ne 0){throw 'MCP tests failed.'}
if(-not $SkipBuild){ & (Join-Path $workspaceRoot 'scripts/Build-Codex.ps1') }
& cargo test --locked --release --manifest-path (Join-Path $workspaceRoot 'settings-app/Cargo.toml') anyblob:: -- --test-threads=1
if($LASTEXITCODE -ne 0){throw 'Anyblob native tests failed.'}
& cargo test --locked --release --manifest-path (Join-Path $workspaceRoot 'settings-app/Cargo.toml') codex:: -- --test-threads=1
if($LASTEXITCODE -ne 0){throw 'Codex permission persistence tests failed.'}
