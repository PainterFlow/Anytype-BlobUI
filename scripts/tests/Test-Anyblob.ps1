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
$env:ANYBLOB_CREDENTIAL_FILE=Join-Path $fixture 'bridge.dpapi'
$env:CARGO_NET_OFFLINE='true'
& (Join-Path $workspaceRoot 'scripts/Build-MCP.ps1') -Test
if($LASTEXITCODE -ne 0){throw 'MCP tests failed.'}
$env:CARGO_TARGET_DIR=Join-Path $workspaceRoot 'mcp-server/target'
$navigationFixture=Join-Path $workspaceRoot '.test-run/navigation'
[IO.Directory]::CreateDirectory($navigationFixture) | Out-Null
& cargo run --locked --release --quiet --manifest-path (Join-Path $workspaceRoot 'mcp-server/Cargo.toml') --example source-fixtures | Set-Content -Encoding utf8 (Join-Path $navigationFixture 'source-fixtures.json')
if($LASTEXITCODE -ne 0){throw 'Production source fixtures failed.'}
if(-not $SkipBuild){ & (Join-Path $workspaceRoot 'scripts/Build-Codex.ps1') -SkipMcpBuild }
$env:CARGO_TARGET_DIR=Join-Path $workspaceRoot 'settings-app/target'
& cargo test --locked --release --manifest-path (Join-Path $workspaceRoot 'settings-app/Cargo.toml') -- --test-threads=1 anyblob:: anyblob_content:: anyblob_scope:: marketplace:: codex:: codex_identity:: codex_projects::
if($LASTEXITCODE -ne 0){throw 'Anyblob native validation failed.'}
