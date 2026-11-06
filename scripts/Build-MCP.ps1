param([switch]$Test)
$ErrorActionPreference = 'Stop'
$workspaceRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
. (Join-Path $PSScriptRoot 'Workspace-Build-Paths.ps1')
Assert-WorkspaceBuildPath (Join-Path $workspaceRoot 'mcp-server/target')
Assert-WorkspaceBuildPath (Join-Path $workspaceRoot 'mcp-server/dist/blobui-mcp.exe')
$savedEnvironment = @{}
foreach ($name in @('PATH','CARGO_HOME','RUSTUP_HOME','TEMP','TMP','LIB','INCLUDE','CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER','CARGO_TARGET_DIR')) { $savedEnvironment[$name] = [Environment]::GetEnvironmentVariable($name,'Process') }
try {
    . (Join-Path $PSScriptRoot 'Rust-Toolchain.ps1')
    $env:CARGO_TARGET_DIR = Join-Path $workspaceRoot 'mcp-server/target'
    $manifest = Join-Path $workspaceRoot 'mcp-server/Cargo.toml'
    if ($Test) {
        & cargo test --locked --release --manifest-path $manifest
        if ($LASTEXITCODE -ne 0) { throw 'MCP tests failed.' }
    }
    & cargo build --locked --release --manifest-path $manifest
    if ($LASTEXITCODE -ne 0) { throw 'MCP build failed.' }
    $dist = Join-Path $workspaceRoot 'mcp-server/dist'
    [IO.Directory]::CreateDirectory($dist) | Out-Null
    Copy-Item -LiteralPath (Join-Path $env:CARGO_TARGET_DIR 'release/blobui-mcp.exe') -Destination (Join-Path $dist 'blobui-mcp.exe') -Force
    Write-Host "Built $dist/blobui-mcp.exe"
} finally { foreach ($name in $savedEnvironment.Keys) { [Environment]::SetEnvironmentVariable($name,$savedEnvironment[$name],'Process') } }
