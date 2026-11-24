$ErrorActionPreference = 'Stop'
$workspaceRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
. (Join-Path $PSScriptRoot 'Workspace-Build-Paths.ps1')
$plugin = Join-Path $workspaceRoot 'codex-plugin/src'
$archive = Join-Path $workspaceRoot 'codex-plugin/dist/anyblob.zip'
Assert-WorkspaceBuildPath $archive

foreach ($test in @('skills.cjs', 'validate.cjs')) {
    & node (Join-Path $workspaceRoot "codex-plugin/tests/$test") $plugin
    if ($LASTEXITCODE -ne 0) { throw "Anyblob package validation failed in $test." }
}

$null = [IO.Directory]::CreateDirectory((Split-Path -Parent $archive))
$temporary = [IO.Path]::ChangeExtension($archive, "$PID.zip")
Assert-WorkspaceBuildPath $temporary
try {
    [IO.Compression.ZipFile]::CreateFromDirectory($plugin, $temporary)
    Move-Item -LiteralPath $temporary -Destination $archive -Force
}
finally {
    if (Test-Path -LiteralPath $temporary) { Remove-Item -LiteralPath $temporary -Force }
}
Write-Host "Packaged Anyblob at $archive"
