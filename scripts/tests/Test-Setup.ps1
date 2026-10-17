$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$root = Join-Path $PSScriptRoot ('.tmp-setup-' + [Guid]::NewGuid().ToString('N'))
$anytype = Join-Path $root 'Anytype Data'
$app = Join-Path $root 'BlobUI App'
function Assert($condition, $message) { if (-not $condition) { throw $message } }
try {
    $fixture = Join-Path $root 'Download'
    [IO.Directory]::CreateDirectory($fixture) | Out-Null
    foreach ($name in @('scripts', 'Dev', 'assets', 'BlobUI Settings Space', 'BlobUI.cmd')) {
        if ($name -eq 'scripts') {
            [IO.Directory]::CreateDirectory((Join-Path $fixture 'scripts')) | Out-Null
            Get-ChildItem (Join-Path $repo 'scripts') -File | Copy-Item -Destination (Join-Path $fixture 'scripts')
        } else { Copy-Item -LiteralPath (Join-Path $repo $name) -Destination $fixture -Recurse }
    }
    [IO.Directory]::CreateDirectory($anytype) | Out-Null
    [IO.File]::WriteAllText((Join-Path $anytype 'custom.css'), 'old-css')
    [IO.File]::WriteAllText((Join-Path $anytype 'unrelated.txt'), 'keep')
    & (Join-Path $fixture 'scripts/Setup.ps1') -AnytypeDirectory $anytype -AppDirectory $app -NoShortcut
    Assert (Test-Path (Join-Path $app 'BlobUI.cmd')) 'App launcher was not installed.'
    Assert (Test-Path (Join-Path $app 'scripts/App.ps1')) 'App UI was not installed.'
    Assert (Test-Path (Join-Path $app 'BlobUI Settings Space')) 'Settings import was not packaged.'
    $css = [IO.File]::ReadAllText((Join-Path $anytype 'custom.css'))
    Assert ($css.Contains('#DE50DE')) 'Default accent was not compiled.'
    Assert (-not $css.Contains('{{')) 'CSS contains unresolved placeholders.'
    Assert ((Get-ChildItem (Join-Path $anytype 'blob/backups') -Recurse -File | Where-Object { [IO.File]::ReadAllText($_.FullName) -eq 'old-css' }).Count -gt 0) 'Old CSS was not backed up.'
    & (Join-Path $app 'scripts/Rebuild.ps1')
    Assert ([IO.File]::ReadAllText((Join-Path $anytype 'custom.css')) -eq $css) 'Default rebuild changed CSS.'
    & (Join-Path $anytype 'blob/runtime/scripts/Rebuild.ps1')
    Assert ([IO.File]::ReadAllText((Join-Path $anytype 'custom.css')) -eq $css) 'Installed runtime rebuild changed CSS.'
    Assert ([IO.File]::ReadAllText((Join-Path $anytype 'unrelated.txt')) -eq 'keep') 'Anytype data was changed.'
    $failed = $false
    try { & (Join-Path $fixture 'scripts/Setup.ps1') -AnytypeDirectory $anytype -AppDirectory $app -NoShortcut } catch { $failed = $true }
    Assert $failed 'Non-empty app directory was accepted.'
    Assert (Test-Path (Join-Path $app 'BlobUI.cmd')) 'Rejected installation damaged existing app.'
    $badApp = Join-Path $anytype 'Nested BlobUI'
    $failed = $false
    try { & (Join-Path $fixture 'scripts/Setup.ps1') -AnytypeDirectory $anytype -AppDirectory $badApp -NoShortcut } catch { $failed = $true }
    Assert $failed 'Nested app directory was accepted.'
    Assert (-not (Test-Path $badApp)) 'Rejected install left files.'
    # A second app install reuses saved local settings rather than resetting them.
    $dataPath = Join-Path $anytype 'blob/data.json'
    $data = Get-Content -LiteralPath $dataPath -Raw | ConvertFrom-Json
    $data.settings.accentColor = '#123456'
    $data | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $dataPath
    $secondApp = Join-Path $root 'Second App'
    & (Join-Path $fixture 'scripts/Setup.ps1') -AnytypeDirectory $anytype -AppDirectory $secondApp -NoShortcut
    Assert ([IO.File]::ReadAllText((Join-Path $anytype 'custom.css')).Contains('#123456')) 'Existing settings were reset.'
    # Invalid saved state fails after app staging and removes the incomplete app.
    [IO.File]::WriteAllText($dataPath, 'invalid json')
    $failedApp = Join-Path $root 'Failed App'
    $failed = $false
    try { & (Join-Path $fixture 'scripts/Setup.ps1') -AnytypeDirectory $anytype -AppDirectory $failedApp -NoShortcut } catch { $failed = $true }
    Assert $failed 'Invalid saved state was accepted.'
    Assert (-not (Test-Path $failedApp)) 'Failed installation left a partial app.'
    Assert ([IO.File]::ReadAllText((Join-Path $anytype 'custom.css')).Contains('#123456')) 'Failed installation damaged CSS.'
    Write-Host 'Installer workflow checks passed.'
}
finally { if (Test-Path $root) { Remove-Item -LiteralPath $root -Recurse -Force } }
