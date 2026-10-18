param([switch]$SkipBuild)
$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$root = Join-Path $repo ('.test-run/n-' + [Guid]::NewGuid().ToString('N').Substring(0, 12))
$oldAppData = $env:APPDATA
$oldLocalAppData = $env:LOCALAPPDATA
function Assert($condition, $message) { if (-not $condition) { throw $message } }
function Get-TestApps { return @(Get-ChildItem -LiteralPath $testRoot -Directory | Where-Object Name -Like 'BlobUI-*') }
function Write-TestZip([string]$Path, [string[]]$Names) {
    $archive = [IO.Compression.ZipFile]::Open($Path, [IO.Compression.ZipArchiveMode]::Create)
    try {
        foreach ($name in $Names) {
            $entry = $archive.CreateEntry($name)
            $writer = New-Object IO.StreamWriter($entry.Open())
            try { $writer.Write('fixture') } finally { $writer.Dispose() }
        }
    } finally { $archive.Dispose() }
}
try {
    [IO.Directory]::CreateDirectory($root) | Out-Null
    if (-not $SkipBuild) {
        & (Join-Path $repo 'scripts/Build-Installer.ps1') -d -PackageRelease
        & (Join-Path $repo 'scripts/Build-Installer.ps1')
    }
    $devExe = Join-Path $root 'blobui-installer-dev.exe'
    $prodExe = Join-Path $root 'blobui-installer.exe'
    Copy-Item -LiteralPath (Join-Path $repo 'dist/blobui-installer-dev.exe') -Destination $devExe
    Copy-Item -LiteralPath (Join-Path $repo 'dist/blobui-installer.exe') -Destination $prodExe
    $package = Join-Path $repo 'dist/blobui-windows.zip'
    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive = [IO.Compression.ZipFile]::OpenRead($package)
    try {
        Assert (@($archive.Entries | Where-Object { $_.FullName -match '\.(ps1|psm1|cmd)$' }).Count -eq 0) 'The release package contains legacy scripts or launchers.'
    } finally { $archive.Dispose() }
    # A development install must ignore the normal profile locations completely.
    $env:APPDATA = Join-Path $root 'protected-roaming'
    $env:LOCALAPPDATA = Join-Path $root 'protected-local'
    [IO.Directory]::CreateDirectory((Join-Path $env:APPDATA 'anytype/blob')) | Out-Null
    [IO.Directory]::CreateDirectory($env:LOCALAPPDATA) | Out-Null
    $protectedData = Join-Path $env:APPDATA 'anytype/blob/data.json'
    [IO.File]::WriteAllText($protectedData, 'do-not-touch-profile')
    & $devExe --no-pause --package $package
    Assert ($LASTEXITCODE -eq 0) 'Native development EXE installation failed.'
    $testRoot = Join-Path $root 'BlobUI-Installer-Test'
    $dataPath = Join-Path $testRoot 'Anytype/blob/data.json'
    Assert (Test-Path -LiteralPath $dataPath) 'Development EXE did not create isolated settings.'
    Assert (Test-Path -LiteralPath (Join-Path $testRoot 'Anytype/blob/runtime/settings-app.exe')) 'Native installation omitted the Settings runtime.'
    $apps = Get-TestApps
    Assert ($apps.Count -eq 1) 'First development installation did not create one app folder.'
    Assert (Test-Path -LiteralPath (Join-Path $apps[0].FullName 'blobui-settings.exe')) 'Native installation omitted the app executable.'
    Assert (Test-Path -LiteralPath (Join-Path $apps[0].FullName '.blobui-install.json')) 'Native installation omitted the install pointer.'
    Assert (-not (Test-Path -LiteralPath (Join-Path $apps[0].FullName 'scripts'))) 'Native installation included legacy scripts.'
    $releaseMetadata = Get-Content -LiteralPath (Join-Path $repo 'blobui-release.json') -Raw | ConvertFrom-Json
    $runtimeMetadata = Get-Content -LiteralPath (Join-Path $testRoot 'Anytype/blob/runtime/blobui-release.json') -Raw | ConvertFrom-Json
    Assert ($runtimeMetadata.version -eq $releaseMetadata.version) 'Installed runtime release version does not match the source.'
    $managedPath = Join-Path $testRoot 'Anytype/blob/.blobui-managed-files.json'
    $managed = Get-Content -LiteralPath $managedPath -Raw | ConvertFrom-Json
    $managed.managedFiles += 'runtime/scripts/obsolete.ps1'
    $managed | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $managedPath -Encoding UTF8
    $oldScripts = Join-Path $testRoot 'Anytype/blob/runtime/scripts'
    [IO.Directory]::CreateDirectory($oldScripts) | Out-Null
    [IO.File]::WriteAllText((Join-Path $oldScripts 'obsolete.ps1'), 'old managed script')
    [IO.File]::WriteAllText((Join-Path $oldScripts 'user-local.ps1'), 'keep local file')
    $data = Get-Content -LiteralPath $dataPath -Raw | ConvertFrom-Json
    $data.settings.accentColor = '#123456'
    $data | Add-Member NoteProperty installerTestExtra 'keep-unknown-property' -Force
    # Windows PowerShell writes a UTF-8 BOM. Existing script-generated settings use this too.
    $data | ConvertTo-Json -Depth 50 | Set-Content -LiteralPath $dataPath -Encoding UTF8
    $savedDataHash = (Get-FileHash -LiteralPath $dataPath).Hash
    $credentialPath = Join-Path $testRoot 'Anytype/blob/credentials/anytype-api-key.dpapi'
    [IO.Directory]::CreateDirectory((Split-Path -Parent $credentialPath)) | Out-Null
    [IO.File]::WriteAllText($credentialPath, 'keep-encrypted-credential')
    $bannerPath = Join-Path $testRoot 'Anytype/blob/assets/spacebanner/banner.png'
    [IO.File]::WriteAllText($bannerPath, 'keep-custom-banner')
    $unrelated = Join-Path $testRoot 'Anytype/keep.txt'
    [IO.File]::WriteAllText($unrelated, 'keep-anytype-data')
    & $devExe --no-pause --package $package
    Assert ($LASTEXITCODE -eq 0) 'Repeated native development installation failed.'
    Assert (-not (Test-Path -LiteralPath (Join-Path $oldScripts 'obsolete.ps1'))) 'Repeated installation left an obsolete managed script.'
    Assert ([IO.File]::ReadAllText((Join-Path $oldScripts 'user-local.ps1')) -eq 'keep local file') 'Repeated installation removed an unmanaged local script.'
    Assert ((Get-FileHash -LiteralPath $dataPath).Hash -eq $savedDataHash) 'Repeated installation rewrote or reset saved settings.'
    Assert ([IO.File]::ReadAllText($credentialPath) -eq 'keep-encrypted-credential') 'Repeated installation changed credentials.'
    Assert ([IO.File]::ReadAllText($bannerPath) -eq 'keep-custom-banner') 'Repeated installation changed a custom banner.'
    Assert ([IO.File]::ReadAllText($unrelated) -eq 'keep-anytype-data') 'Repeated installation changed unrelated data.'
    Assert ([IO.File]::ReadAllText((Join-Path $testRoot 'Anytype/custom.css')).Contains('#123456')) 'Saved settings were not applied to CSS.'
    Assert ((Get-TestApps).Count -eq 2) 'Repeated installs must use separate app folders.'
    Assert (@(Get-ChildItem -LiteralPath (Join-Path $testRoot 'Anytype/blob/backups') -File).Count -gt 0) 'Existing CSS was not backed up.'
    Assert (Test-Path -LiteralPath (Join-Path $testRoot 'Anytype/blob/runtime/assets/spacebanner/banner.png')) 'The runtime has no default banner for later Settings saves.'
    $runtimeExecutable = Join-Path $testRoot 'Anytype/blob/runtime/settings-app.exe'
    $lockedFile = [IO.File]::Open($runtimeExecutable, [IO.FileMode]::Open, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
    try {
        & $devExe --no-pause --package $package
        Assert ($LASTEXITCODE -ne 0) 'Installation ignored a locked runtime executable.'
    } finally { $lockedFile.Dispose() }
    Assert ((Get-TestApps).Count -eq 2) 'Failed runtime installation left a partial app.'
    Assert ((Get-FileHash -LiteralPath $dataPath).Hash -eq $savedDataHash) 'Failed runtime installation changed settings.'
    & $prodExe --no-pause --package $package
    Assert ($LASTEXITCODE -ne 0) 'Production EXE accepted a local package.'
    & $devExe --no-pause --package $package --anytype-directory $env:APPDATA
    Assert ($LASTEXITCODE -ne 0) 'Development EXE accepted a normal-profile destination override.'
    & $devExe --no-pause --package (Join-Path $root 'missing.zip')
    Assert ($LASTEXITCODE -ne 0) 'Development EXE accepted a missing package.'
    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    foreach ($case in @(
        @{ Name = 'traversal'; Entries = @('../escape.txt') },
        @{ Name = 'incomplete'; Entries = @('unrelated.txt') },
        @{ Name = 'duplicate'; Entries = @('same.txt', 'same.txt') },
        @{ Name = 'case-collision'; Entries = @('same.txt', 'SAME.txt') }
    )) {
        $badZip = Join-Path $root ($case.Name + '.zip')
        Write-TestZip -Path $badZip -Names $case.Entries
        & $devExe --no-pause --package $badZip
        Assert ($LASTEXITCODE -ne 0) "Unsafe package was accepted: $($case.Name)"
    }
    Assert ((Get-TestApps).Count -eq 2) 'Rejected packages left partial app installations.'
    Assert (-not (Test-Path -LiteralPath (Join-Path $testRoot 'escape.txt'))) 'ZIP traversal wrote outside extraction.'
    # Invalid saved state must fail without changing CSS or creating a partial app.
    [IO.File]::WriteAllText($dataPath, 'invalid-json')
    $cssHash = (Get-FileHash -LiteralPath (Join-Path $testRoot 'Anytype/custom.css')).Hash
    & $devExe --no-pause --package $package
    Assert ($LASTEXITCODE -ne 0) 'Invalid saved settings were accepted.'
    Assert ([IO.File]::ReadAllText($dataPath) -eq 'invalid-json') 'Failed installation changed saved data.'
    Assert ((Get-FileHash -LiteralPath (Join-Path $testRoot 'Anytype/custom.css')).Hash -eq $cssHash) 'Failed installation changed CSS.'
    Assert ((Get-TestApps).Count -eq 2) 'Failed installation left a partial app.'
    Assert (@(Get-ChildItem -LiteralPath $testRoot -Directory -Force | Where-Object Name -Like '.bi-*').Count -eq 0) 'Native installer left extraction staging behind.'
    Assert ([IO.File]::ReadAllText($protectedData) -eq 'do-not-touch-profile') 'Development installer touched normal-profile data.'
    Assert (@(Get-ChildItem -LiteralPath $env:LOCALAPPDATA -Force).Count -eq 0) 'Development installer touched normal local app data.'
    Write-Host 'Native installer, isolation, preservation, failure handling, and ZIP validation checks passed.'
}
finally {
    $env:APPDATA = $oldAppData
    $env:LOCALAPPDATA = $oldLocalAppData
    $expectedPrefix = $repo.TrimEnd('\') + '\.test-run\'
    if (-not ([IO.Path]::GetFullPath($root)).StartsWith($expectedPrefix, [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe test cleanup path.' }
    if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }
}
