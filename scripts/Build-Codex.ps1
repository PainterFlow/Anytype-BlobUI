param([switch]$SkipMcpBuild)
$ErrorActionPreference = 'Stop'
$workspaceRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
. (Join-Path $PSScriptRoot 'Workspace-Build-Paths.ps1')
Assert-WorkspaceBuildPath (Join-Path $workspaceRoot 'codex-plugin/dist/anyblob')
Assert-WorkspaceBuildPath (Join-Path $workspaceRoot 'codex-plugin/dist/anyblob.zip')
Assert-WorkspaceBuildPath (Join-Path $workspaceRoot 'codex-plugin/dist/marketplace-payload.json')
if (-not $SkipMcpBuild) {
    & (Join-Path $PSScriptRoot 'Build-MCP.ps1')
    if ($LASTEXITCODE -ne 0) { throw 'MCP build failed.' }
}
$dist = Join-Path $workspaceRoot 'codex-plugin/dist'
$package = Join-Path $dist 'anyblob'
if(Test-Path -LiteralPath $package){Remove-Item -LiteralPath $package -Recurse -Force}
[IO.Directory]::CreateDirectory($package) | Out-Null
Copy-Item -Path (Join-Path $workspaceRoot 'codex-plugin/src/*') -Destination $package -Recurse -Force
[IO.Directory]::CreateDirectory((Join-Path $package 'bin')) | Out-Null
[IO.Directory]::CreateDirectory((Join-Path $package 'assets')) | Out-Null
Copy-Item -LiteralPath (Join-Path $workspaceRoot 'mcp-server/dist/blobui-mcp.exe') -Destination (Join-Path $package 'bin/blobui-mcp.exe') -Force
# ICO entries can hold PNG frames. Copy the largest original frame byte for byte.
$ico = [IO.File]::ReadAllBytes((Join-Path $workspaceRoot 'settings-app/icons/icon.ico'))
$frames = @()
for ($i=0; $i -lt [BitConverter]::ToUInt16($ico,4); $i++) {
    $entry=6+16*$i; $size=[BitConverter]::ToUInt32($ico,$entry+8); $offset=[BitConverter]::ToUInt32($ico,$entry+12)
    if ($offset+$size -gt $ico.Length) { throw 'Invalid app icon frame.' }
    if ($ico[$offset] -eq 137 -and $ico[$offset+1] -eq 80 -and $ico[$offset+2] -eq 78 -and $ico[$offset+3] -eq 71) {
        $width=if($ico[$entry] -eq 0){256}else{[int]$ico[$entry]}; $height=if($ico[$entry+1] -eq 0){256}else{[int]$ico[$entry+1]}
        $frames += @{size=$size;offset=$offset;area=$width*$height}
    }
}
$frame=$frames | Sort-Object { $_.area } -Descending | Select-Object -First 1
if (-not $frame) { throw 'The existing app icon has no PNG frame.' }
[IO.File]::WriteAllBytes((Join-Path $package 'assets/icon.png'),[byte[]]$ico[$frame.offset..($frame.offset+$frame.size-1)])
& node (Join-Path $workspaceRoot 'codex-plugin/tests/skills.cjs') $package
if ($LASTEXITCODE -ne 0) { throw 'Anyblob skill validation failed.' }
& node (Join-Path $workspaceRoot 'codex-plugin/tests/validate.cjs') $package
if ($LASTEXITCODE -ne 0) { throw 'Anyblob package validation failed.' }
$zip=Join-Path $dist 'anyblob.zip'
if (Test-Path -LiteralPath $zip) { Remove-Item -LiteralPath $zip -Force }
Compress-Archive -Path (Join-Path $package '*') -DestinationPath $zip
$archive=[IO.Compression.ZipFile]::OpenRead($zip)
try {
    $expected=@(
        'plugin.json',
        'mcp.json',
        '.codex-plugin/plugin.json',
        '.mcp.json',
        'README.md',
        'assets/icon.png',
        'bin/blobui-mcp.exe',
        'references/workflow-rules.md',
        'skills/search-and-answer/SKILL.md',
        'skills/search-and-answer/agents/openai.yaml',
        'skills/capture-and-organize/SKILL.md',
        'skills/capture-and-organize/agents/openai.yaml',
        'skills/maintain-pages/SKILL.md',
        'skills/maintain-pages/agents/openai.yaml',
        'skills/plan-projects/SKILL.md',
        'skills/plan-projects/agents/openai.yaml',
        'skills/review-project-progress/SKILL.md',
        'skills/review-project-progress/agents/openai.yaml',
        'skills/process-meeting-notes/SKILL.md',
        'skills/process-meeting-notes/agents/openai.yaml',
        'skills/manage-types-and-templates/SKILL.md',
        'skills/manage-types-and-templates/agents/openai.yaml',
        'skills/build-queries-and-views/SKILL.md',
        'skills/build-queries-and-views/agents/openai.yaml',
        'skills/configure-kanban-calculations/SKILL.md',
        'skills/configure-kanban-calculations/agents/openai.yaml',
        'skills/configure-calendar-colors/SKILL.md',
        'skills/configure-calendar-colors/agents/openai.yaml',
        'skills/audit-organization/SKILL.md',
        'skills/audit-organization/agents/openai.yaml'
    )
    $actual=@($archive.Entries | Where-Object { -not $_.FullName.EndsWith('/') } | ForEach-Object { $_.FullName.Replace('\','/') })
    if(@(Compare-Object $expected $actual).Count -ne 0){throw 'The ZIP does not contain the exact validated plugin files.'}
} finally {$archive.Dispose()}
Write-Host "Packaged Anyblob at $package and $zip"
# A fixed manifest is embedded in Settings, so installation needs no external runtime.
$files = [ordered]@{}
foreach ($relative in $expected) {
    $files[$relative] = [Convert]::ToBase64String([IO.File]::ReadAllBytes((Join-Path $package $relative)))
}
$payload = [ordered]@{
    marketplace = (Get-Content -LiteralPath (Join-Path $workspaceRoot 'codex-plugin/marketplace/marketplace.json') -Raw | ConvertFrom-Json)
    files = $files
}
$payloadPath = Join-Path $dist 'marketplace-payload.json'
$payloadText = $payload | ConvertTo-Json -Depth 20
if (-not (Test-Path -LiteralPath $payloadPath) -or [IO.File]::ReadAllText($payloadPath) -cne $payloadText) {
    [IO.File]::WriteAllText($payloadPath, $payloadText, [Text.UTF8Encoding]::new($false))
}
