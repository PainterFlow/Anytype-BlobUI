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
    $packagedApp = Join-Path $fixture 'settings-app/dist/blobui-settings.exe'
    [IO.Directory]::CreateDirectory((Split-Path -Parent $packagedApp)) | Out-Null
    [IO.File]::WriteAllText($packagedApp, 'fixture-executable-not-for-launch')
    [IO.Directory]::CreateDirectory($anytype) | Out-Null
    [IO.File]::WriteAllText((Join-Path $anytype 'custom.css'), 'old-css')
    [IO.File]::WriteAllText((Join-Path $anytype 'unrelated.txt'), 'keep')
    & (Join-Path $fixture 'scripts/Setup.ps1') -AnytypeDirectory $anytype -AppDirectory $app -NoShortcut
    Assert (Test-Path (Join-Path $app 'settings-app/dist/blobui-settings.exe')) 'The packaged Settings app was not installed.'
    Assert (Test-Path (Join-Path $anytype 'blob/runtime/settings-app.exe')) 'The Settings runtime executable was not installed.'
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
    # Opting out must ignore even an existing key/cache and continue to rebuild locally.
    $saved = Get-Content -LiteralPath $dataPath -Raw | ConvertFrom-Json
    $saved.calendarObjectColors = [pscustomobject]@{ 'test-calendar-object' = 'Red' }
    $saved | ConvertTo-Json -Depth 30 | Set-Content -LiteralPath $dataPath
    & (Join-Path $secondApp 'scripts/Rebuild.ps1') -InstallDirectory $anytype -ConnectionOverride ([pscustomobject]@{ enabled = $false; port = $null; apiKey = '' }) -ApiTransport { throw 'Disabled mode contacted the API.' }
    $disabled = Get-Content -LiteralPath $dataPath -Raw | ConvertFrom-Json
    Assert (-not $disabled.apiConnection.enabled) 'No API mode was not saved.'
    Assert ($disabled.apiConnection.port -eq 31009) 'The default port was not saved.'
    Assert (@($disabled.calendarObjectColors.PSObject.Properties).Count -eq 0) 'No API mode retained calendar colors.'
    & (Join-Path $secondApp 'scripts/Rebuild.ps1') -InstallDirectory $anytype -ApiTransport { throw 'Disabled rebuild contacted the API.' }
    Assert ([IO.File]::ReadAllText((Join-Path $anytype 'custom.css')).Contains('#123456')) 'No API rebuild lost local CSS settings.'
    $unchangedCss = [IO.File]::ReadAllText((Join-Path $anytype 'custom.css'))
    $invalidPortFailed = $false
    try { & (Join-Path $secondApp 'scripts/Rebuild.ps1') -InstallDirectory $anytype -ConnectionOverride ([pscustomobject]@{ enabled = $true; port = 65536; apiKey = 'test-key' }) } catch { $invalidPortFailed = $true }
    Assert $invalidPortFailed 'An invalid API port was accepted.'
    Assert ([IO.File]::ReadAllText((Join-Path $anytype 'custom.css')) -eq $unchangedCss) 'Invalid connection setup modified CSS.'
    if ($env:OS -eq 'Windows_NT') {
        $successfulApi = {
            param($key, $method, $path, $body, $headers)
            if ($path.StartsWith('/v2/spaces?')) { return [pscustomobject]@{ data = @([pscustomobject]@{ id = 'test-space'; name = 'Test' }); has_more = $false } }
            return [pscustomobject]@{ data = @(); has_more = $false }
        }
        & (Join-Path $secondApp 'scripts/Rebuild.ps1') -InstallDirectory $anytype -ConnectionOverride ([pscustomobject]@{ enabled = $true; port = 31010; apiKey = 'test-only-key' }) -ApiTransport $successfulApi
        $enabled = Get-Content -LiteralPath $dataPath -Raw | ConvertFrom-Json
        Assert ($enabled.apiConnection.enabled -and $enabled.apiConnection.port -eq 31010) 'API connection or custom port was not saved.'
        Assert (-not ([IO.File]::ReadAllText($dataPath).Contains('test-only-key'))) 'Plaintext key leaked into data.json.'
        $keyFile = Join-Path $anytype 'blob/credentials/anytype-api-key.dpapi'
        Assert ((Read-BlobUiProtectedCredential -Path $keyFile) -eq 'test-only-key') 'Saved key did not decrypt.'
        $calendarFailureApi = {
            param($key, $method, $path, $body, $headers)
            if ($path -eq '/v2/spaces?limit=1') { return [pscustomobject]@{ data = @(); has_more = $false } }
            throw 'Simulated optional calendar failure.'
        }
        & (Join-Path $secondApp 'scripts/Rebuild.ps1') -InstallDirectory $anytype -ConnectionOverride ([pscustomobject]@{ enabled = $true; port = 31011; apiKey = 'rotated-test-key' }) -ApiTransport $calendarFailureApi
        Assert ((Read-BlobUiProtectedCredential -Path $keyFile) -eq 'rotated-test-key') 'A valid replacement key was rejected because calendar refresh failed.'
        $beforeFailure = [IO.File]::ReadAllText($dataPath)
        $failedConnection = $false
        try { & (Join-Path $secondApp 'scripts/Rebuild.ps1') -InstallDirectory $anytype -ConnectionOverride ([pscustomobject]@{ enabled = $true; port = 31011; apiKey = 'bad-test-key' }) -ApiTransport { throw 'Simulated API failure.' } } catch { $failedConnection = $true }
        Assert $failedConnection 'Failed API validation was accepted.'
        Assert ([IO.File]::ReadAllText($dataPath) -eq $beforeFailure) 'Failed connection changed saved data.'
        Assert ((Read-BlobUiProtectedCredential -Path $keyFile) -eq 'rotated-test-key') 'Failed connection changed the encrypted key.'
    }
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
