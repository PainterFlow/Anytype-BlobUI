$ErrorActionPreference = 'Stop'
$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$testRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '.tmp'))
$testsRoot = [System.IO.Path]::GetFullPath($PSScriptRoot).TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
if (-not $testRoot.StartsWith($testsRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw 'The test output directory is outside the tests folder.'
}
if (Test-Path -LiteralPath $testRoot) {
    Remove-Item -LiteralPath $testRoot -Recurse -Force
}
[System.IO.Directory]::CreateDirectory($testRoot) | Out-Null
Import-Module (Join-Path $repoRoot 'BlobUI.Core.psm1') -Force

function Assert-True {
    param([bool]$Condition, [string]$Message)
    if (-not $Condition) {
        throw $Message
    }
}

function Assert-Equal {
    param($Expected, $Actual, [string]$Message)
    if ($Expected -is [array] -or $Actual -is [array]) {
        $expectedText = [string]::Join('|', @($Expected))
        $actualText = [string]::Join('|', @($Actual))
        if ($expectedText -cne $actualText) {
            throw "$Message Expected '$expectedText', received '$actualText'."
        }
        return
    }
    if ([string]$Expected -cne [string]$Actual) {
        throw "$Message Expected '$Expected', received '$Actual'."
    }
}

function Assert-Throws {
    param([scriptblock]$Action, [string]$Message, [string]$ExpectedText)
    $failureText = $null
    try {
        & $Action | Out-Null
    }
    catch {
        $failureText = $_.Exception.Message
    }
    if ($null -eq $failureText) {
        throw "$Message Expected an error, but the operation succeeded."
    }
    if (-not [string]::IsNullOrWhiteSpace($ExpectedText) -and $failureText -notlike "*$ExpectedText*") {
        throw "$Message Expected an error containing '$ExpectedText', received '$failureText'."
    }
}

function ConvertTo-TestApiObject {
    param($InputObject)
    $json = ConvertTo-Json -InputObject $InputObject -Depth 20 -Compress
    return ConvertFrom-Json -InputObject $json -ErrorAction Stop
}

function Get-CssSignature {
    param([string]$CssText)
    $withoutComments = [System.Text.RegularExpressions.Regex]::Replace(
        $CssText,
        '/\*.*?\*/',
        '',
        [System.Text.RegularExpressions.RegexOptions]::Singleline
    )
    return [System.Text.RegularExpressions.Regex]::Replace($withoutComments, '\s+', '')
}

function New-TestTextProperty {
    param([string]$Key, [string]$Name, [string]$Text)
    return @{
        key = $Key
        name = $Name
        format = 'text'
        object = 'property'
        text = $Text
    }
}

function New-TestSettingObject {
    param([string]$Id, [string]$Name, [object[]]$Properties)
    return @{
        id = $Id
        name = $Name
        type = @{ key = 'setting'; name = 'Setting' }
        properties = $Properties
    }
}

$apiRequests = [System.Collections.Generic.List[string]]::new()
$knownKeys = @('unit-test-key', 'bad-color-key', 'bad-font-key', 'bad-banner-key')
$apiTransport = {
    param($ApiKey, $Path)

    function ConvertTo-TestApiObject {
        param($InputObject)
        $json = ConvertTo-Json -InputObject $InputObject -Depth 20 -Compress
        return ConvertFrom-Json -InputObject $json -ErrorAction Stop
    }
    function New-TestTextProperty {
        param([string]$Key, [string]$Name, [string]$Text)
        return @{
            key = $Key
            name = $Name
            format = 'text'
            object = 'property'
            text = $Text
        }
    }
    function New-TestSettingObject {
        param([string]$Id, [string]$Name, [object[]]$Properties)
        return @{
            id = $Id
            name = $Name
            type = @{ key = 'setting'; name = 'Setting' }
            properties = $Properties
        }
    }

    $apiRequests.Add([string]$Path)
    if ($knownKeys -cnotcontains [string]$ApiKey) {
        $exception = [System.Exception]::new('Mock API rejected the key.')
        $exception.Data['StatusCode'] = 401
        throw $exception
    }

    $cleanPath = ([string]$Path).Split('?')[0]
    if ($cleanPath -eq '/v1/spaces') {
        $payload = @{
            data = @(@{ id = 'space-1'; name = 'BlobUI Settings' })
            pagination = @{ has_more = $false; limit = 1000; offset = 0 }
        }
    }
    elseif ($cleanPath -eq '/v1/spaces/space-1/objects') {
        if ([string]$Path -notmatch '\?type=setting(?:&|$)') {
            throw 'The mock API received an unfiltered settings object query.'
        }
        $payload = @{
            data = @(
                @{ id = 'general-id'; name = 'General Settings'; type = @{ key = 'setting'; name = 'Setting' } },
                @{ id = 'space-id'; name = 'Space Specific Settings'; type = @{ key = 'setting'; name = 'Setting' } }
            )
            pagination = @{ has_more = $false; limit = 1000; offset = 0 }
        }
    }
    elseif ($cleanPath -eq '/v1/spaces/space-1/objects/general-id') {
        if ($ApiKey -eq 'bad-color-key') {
            $properties = @(
                (New-TestTextProperty 'accent_color' 'Accent Color' '#de50de; color:red')
            )
        }
        elseif ($ApiKey -eq 'bad-font-key') {
            $properties = @(
                (New-TestTextProperty 'font' 'Font' 'Segoe UI; background:url(x)')
            )
        }
        else {
            $properties = @(
                (New-TestTextProperty 'font' 'Font' 'Libron'),
                (New-TestTextProperty 'code_font' 'Code Font' 'Consolas'),
                (New-TestTextProperty 'accent_color' 'Accent Color' '#DE50DE')
            )
        }
        $payload = @{ data = (New-TestSettingObject 'general-id' 'General Settings' $properties) }
    }
    elseif ($cleanPath -eq '/v1/spaces/space-1/objects/space-id') {
        if ($ApiKey -eq 'bad-banner-key') {
            $bannerValue = '..\outside.png'
        }
        else {
            $bannerValue = 'assets\spacebanner\banner.png'
        }
        $properties = @(
            (New-TestTextProperty 'banner' 'Banner' $bannerValue)
        )
        $payload = @{ data = (New-TestSettingObject 'space-id' 'Space Specific Settings' $properties) }
    }
    elseif ($cleanPath -eq '/v1/spaces/space-1/objects/empty-id') {
        $payload = @{ data = (New-TestSettingObject 'empty-id' 'Empty Settings' @()) }
    }
    else {
        $exception = [System.Exception]::new('Mock API object not found.')
        $exception.Data['StatusCode'] = 404
        throw $exception
    }
    return ConvertTo-TestApiObject $payload
}.GetNewClosure()

$unavailableApiTransport = {
    throw [System.Net.Sockets.SocketException]::new([System.Net.Sockets.SocketError]::ConnectionRefused)
}

try {
    $installDirectory = Join-Path $testRoot 'Anytype Data ü #1'
    [System.IO.Directory]::CreateDirectory($installDirectory) | Out-Null
    $oldCssPath = Join-Path $installDirectory 'custom.css'
    [System.IO.File]::WriteAllText($oldCssPath, 'old-css-sentinel')
    $blobDirectory = Join-Path $installDirectory 'blob'
    [System.IO.Directory]::CreateDirectory((Join-Path $blobDirectory 'assets')) | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $blobDirectory 'data.json'), 'old-data-sentinel')
    [System.IO.File]::WriteAllText((Join-Path $blobDirectory 'assets\user-file.txt'), 'keep-me')

    $candidateDataPath = Join-Path $testRoot 'candidate-data.json'
    $getArguments = @{
        ApiKey = 'unit-test-key'
        OutputPath = $candidateDataPath
        InstallDirectory = $installDirectory
        SelectionProvider = {
            param($Kind, $Prompt, $Items)
            if ($Kind -eq 'single') {
                return 1
            }
            return @(1, 2)
        }
        ApiTransport = $apiTransport
    }
    & (Join-Path $repoRoot 'Get-Data.ps1') @getArguments
    $fetchedData = Read-BlobUiDataFile -Path $candidateDataPath

    Assert-Equal 'space-1' $fetchedData.selectedSpace.id 'The selected space ID was not saved.'
    Assert-Equal 'BlobUI Settings' $fetchedData.selectedSpace.name 'The selected space name was not saved.'
    Assert-Equal 'Libron' $fetchedData.settings.fontFamily 'The exported Font property was not mapped.'
    Assert-Equal 'Consolas' $fetchedData.settings.codeFontFamily 'The exported Code Font property was not mapped.'
    Assert-Equal '#DE50DE' $fetchedData.settings.accentColor 'The exported Accent Color property was not mapped.'
    Assert-Equal 'spacebanner\banner.png' $fetchedData.settings.bannerPath 'The exported Banner property was not normalized.'
    Assert-Equal 'general-id|space-id' $fetchedData.settingsObjectIds 'Selected object IDs were not stored in selection order.'
    $apiRequestText = [string]::Join([Environment]::NewLine, $apiRequests.ToArray())
    Assert-True ($apiRequestText.Contains('/v1/spaces/space-1/objects?type=setting')) 'The selector did not query the exported Setting type key.'

    $emptyDataPath = Join-Path $testRoot 'empty-data.json'
    $emptyData = Get-BlobUiSettingsData -ApiKey 'unit-test-key' -OutputPath $emptyDataPath -SpaceId 'space-1' -ObjectIds @('empty-id') -InstallDirectory $installDirectory -ApiTransport $apiTransport
    Assert-Equal 'Libron' $emptyData.settings.fontFamily 'The Font default is incorrect.'
    Assert-Equal 'Consolas' $emptyData.settings.codeFontFamily 'The Code Font default is incorrect.'
    Assert-Equal '#DE50DE' $emptyData.settings.accentColor 'The Accent Color default is incorrect.'
    Assert-Equal 'spacebanner\banner.png' $emptyData.settings.bannerPath 'The Banner default is incorrect.'

    Assert-Equal 'Segoe UI' (ConvertTo-BlobUiCssPlaceholderValue -Type font -Value 'Segoe UI') 'A valid font name was rejected.'
    Assert-Throws { ConvertTo-BlobUiCssPlaceholderValue -Type font -Value 'Segoe UI; color:red' } 'Unsafe font value' 'font family'
    Assert-Equal '#12ABEF' (ConvertTo-BlobUiCssPlaceholderValue -Type color -Value '#12abef') 'A valid hex color was not normalized.'
    Assert-Throws { ConvertTo-BlobUiCssPlaceholderValue -Type color -Value 'red; color:blue' } 'Unsafe color value' 'hexadecimal'
    Assert-Equal '12.5' (ConvertTo-BlobUiCssPlaceholderValue -Type number -Value '12.5') 'A valid CSS number was rejected.'
    Assert-Throws { ConvertTo-BlobUiCssPlaceholderValue -Type number -Value '12px' } 'Unsafe number value' 'CSS number'
    Assert-Equal '1' (ConvertTo-BlobUiCssPlaceholderValue -Type boolean -Value 'true') 'A valid true value was rejected.'
    Assert-Equal '0' (ConvertTo-BlobUiCssPlaceholderValue -Type boolean -Value 'false') 'A valid false value was rejected.'
    Assert-Throws { ConvertTo-BlobUiCssPlaceholderValue -Type boolean -Value 'yes' } 'Unsafe boolean value' 'true or false'
    Assert-Throws { ConvertTo-BlobUiCssPlaceholderValue -Type url -Value 'javascript:alert(1)' } 'Unsafe URL value' 'local file URI'

    $compiledCssPath = Join-Path $testRoot 'compiled.css'
    $compileResult = Invoke-BlobUiCssCompile -DevDirectory (Join-Path $repoRoot 'Dev') -AssetSourceDirectory (Join-Path $repoRoot 'assets') -OutputPath $compiledCssPath -Data $fetchedData
    $css = [System.IO.File]::ReadAllText($compiledCssPath)
    Assert-Equal '00-settings.css|10-fonts.css|20-sidebar-banner.css|30-icons.css' ([string[]]@($compileResult.sourceFiles | ForEach-Object { Split-Path -Leaf $_ })) 'Dev CSS source ordering is incorrect.'
    Assert-True ($css.Contains('--color-blob-pink: #DE50DE;')) 'Compiled CSS omitted the default accent color.'
    Assert-True ($css.Contains('font-family: "Libron", sans-serif !important;')) 'Compiled CSS omitted the default UI font.'
    Assert-True ($css.Contains('font-family: "Consolas", "Courier New", monospace !important;')) 'Compiled CSS omitted the default code font.'
    Assert-True ($css.Contains('file:///') -and $css.Contains('Anytype%20Data%20%C3%BC%20%231')) 'The banner file URI did not escape spaces, Unicode, and the hash character.'
    Assert-True (-not $css.Contains('{{')) 'Compiled CSS contains an unresolved placeholder.'
    $baselineCss = [System.IO.File]::ReadAllText((Join-Path $PSScriptRoot 'Fixtures\legacy-custom.css'))
    $expectedBannerUri = [System.Uri]::new((Join-Path $installDirectory 'blob\assets\spacebanner\banner.png')).AbsoluteUri
    $baselineCss = $baselineCss.Replace('#de50de', '#DE50DE')
    $baselineCss = [System.Text.RegularExpressions.Regex]::Replace(
        $baselineCss,
        '(?m)^[ \t]*--header-image[ \t]*:.*$',
        '--header-image: url("' + $expectedBannerUri + '");'
    )
    Assert-Equal (Get-CssSignature $baselineCss) (Get-CssSignature $css) 'Compiled CSS changed the prior cascade or default styling.'
    foreach ($selector in @(
        '#widget-home>div>div>div>div>div.icon.settingsHome.iconColor.iconColor-red.hasSvg',
        '.sidebar .sidebarPage.pageWidget>.head.isDefault::after',
        '.sidebar .sidebarPage.pageWidgetManage>.head.isDefault::after',
        '.sidebar .sidebarPage.pageWidget>.body::-webkit-scrollbar-track',
        '.sidebar .sidebarPage .spaceName',
        '#button-header-favorite.menuActionUnfav',
        '#button-header-relation.active'
    )) {
        Assert-True ($css.Contains($selector)) "The source CSS selector was lost during compilation: $selector"
    }

    $nestedDev = Join-Path $testRoot 'Dev fixture'
    [System.IO.Directory]::CreateDirectory((Join-Path $nestedDev 'nested')) | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $nestedDev '10-later.css'), '.later { order: 2; }')
    [System.IO.File]::WriteAllText((Join-Path $nestedDev 'nested\05-first.css'), '.first { order: 1; }')
    $nestedCompilePath = Join-Path $testRoot 'nested.css'
    $nestedResult = Invoke-BlobUiCssCompile -DevDirectory $nestedDev -AssetSourceDirectory (Join-Path $repoRoot 'assets') -OutputPath $nestedCompilePath -Data $fetchedData
    Assert-Equal '10-later.css|nested\05-first.css' ([string[]]@($nestedResult.sourceFiles | ForEach-Object { $_.Substring($nestedDev.Length + 1) })) 'Recursive CSS discovery or ordinal path order failed.'
    $nestedCss = [System.IO.File]::ReadAllText($nestedCompilePath)
    Assert-True ($nestedCss.IndexOf('.10-later') -lt 0) 'Unexpected test selector was introduced.'
    Assert-True ($nestedCss.IndexOf('.later') -lt $nestedCss.IndexOf('.first')) 'Nested CSS was not concatenated in deterministic path order.'

    $badDataPath = Join-Path $testRoot 'bad-data.json'
    Assert-Throws {
        Get-BlobUiSettingsData -ApiKey 'bad-color-key' -OutputPath $badDataPath -SpaceId 'space-1' -ObjectIds @('general-id') -InstallDirectory $installDirectory -ApiTransport $apiTransport
    } 'Invalid API setting' 'Accent Color'
    Assert-True (-not (Test-Path -LiteralPath $badDataPath)) 'Invalid API settings were written to the candidate data file.'
    Assert-Equal 'old-css-sentinel' ([System.IO.File]::ReadAllText($oldCssPath)) 'A settings validation failure changed the installed CSS.'
    Assert-Equal 'old-data-sentinel' ([System.IO.File]::ReadAllText((Join-Path $blobDirectory 'data.json'))) 'A settings validation failure changed saved data.'
    Assert-Throws {
        Get-BlobUiSettingsData -ApiKey 'bad-font-key' -OutputPath $badDataPath -SpaceId 'space-1' -ObjectIds @('general-id') -InstallDirectory $installDirectory -ApiTransport $apiTransport
    } 'Invalid Font setting' 'Font'
    Assert-Throws {
        Get-BlobUiSettingsData -ApiKey 'bad-banner-key' -OutputPath $badDataPath -SpaceId 'space-1' -ObjectIds @('space-id') -InstallDirectory $installDirectory -ApiTransport $apiTransport
    } 'Invalid Banner setting' 'path segment'
    Assert-Equal 'old-css-sentinel' ([System.IO.File]::ReadAllText($oldCssPath)) 'An invalid font or banner changed the installed CSS.'
    Assert-Equal 'old-data-sentinel' ([System.IO.File]::ReadAllText((Join-Path $blobDirectory 'data.json'))) 'An invalid font or banner changed saved data.'

    Assert-Throws {
        Get-BlobUiSettingsData -ApiKey 'wrong-key' -OutputPath $badDataPath -SpaceId 'space-1' -ObjectIds @('general-id') -InstallDirectory $installDirectory -ApiTransport $apiTransport
    } 'Invalid API key' 'rejected the API key'
    Assert-Throws {
        Get-BlobUiSettingsData -ApiKey 'unit-test-key' -OutputPath $badDataPath -SpaceId 'space-1' -ObjectIds @('general-id') -InstallDirectory $installDirectory -ApiTransport $unavailableApiTransport
    } 'Unavailable API' 'Could not reach Anytype local API'
    Assert-Equal 'old-css-sentinel' ([System.IO.File]::ReadAllText($oldCssPath)) 'An API failure changed the installed CSS.'
    Assert-Equal 'old-data-sentinel' ([System.IO.File]::ReadAllText((Join-Path $blobDirectory 'data.json'))) 'An API failure changed saved data.'

    $invalidCompileData = [pscustomobject]@{
        installDirectory = $installDirectory
        settings = [pscustomobject]@{
            fontFamily = 'Bad; color:red'
            codeFontFamily = 'Consolas'
            accentColor = '#DE50DE'
            bannerPath = 'spacebanner\banner.png'
        }
    }
    Assert-Throws {
        Invoke-BlobUiCssCompile -DevDirectory (Join-Path $repoRoot 'Dev') -AssetSourceDirectory (Join-Path $repoRoot 'assets') -OutputPath (Join-Path $testRoot 'invalid-compile.css') -Data $invalidCompileData
    } 'Invalid compile value' 'font family'
    Assert-Throws {
        Invoke-BlobUiCssCompile -DevDirectory (Join-Path $repoRoot 'Dev') -AssetSourceDirectory (Join-Path $repoRoot 'assets') -OutputPath (Join-Path $repoRoot 'Dev\generated-test.css') -Data $fetchedData
    } 'Output inside Dev' 'outside the Dev directory'
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $repoRoot 'Dev\generated-test.css'))) 'The compiler created generated CSS under Dev.'
    Assert-Equal 'old-css-sentinel' ([System.IO.File]::ReadAllText($oldCssPath)) 'A compile validation failure changed the installed CSS.'
    Assert-Equal 'old-data-sentinel' ([System.IO.File]::ReadAllText((Join-Path $blobDirectory 'data.json'))) 'A compile validation failure changed saved data.'

    $protectedCredential = New-BlobUiProtectedCredential -ApiKey 'unit-test-key'
    Assert-True ($protectedCredential -cne 'unit-test-key') 'The encrypted credential file would contain the plaintext API key.'
    $installArguments = @{
        InstallDirectory = $installDirectory
        RuntimeSourceDirectory = $repoRoot
        AssetsSourceDirectory = Join-Path $repoRoot 'assets'
        CompiledCssPath = $compiledCssPath
        DataCandidatePath = $candidateDataPath
        CredentialProtectedText = $protectedCredential
    }
    $firstInstall = Invoke-BlobUiInstall @installArguments
    Assert-True (Test-Path -LiteralPath $firstInstall.cssPath -PathType Leaf) 'The CSS install did not create custom.css.'
    Assert-True (Test-Path -LiteralPath (Join-Path $firstInstall.assetsPath 'spacebanner\banner.png') -PathType Leaf) 'The banner asset was not installed.'
    Assert-Equal 'keep-me' ([System.IO.File]::ReadAllText((Join-Path $firstInstall.assetsPath 'user-file.txt'))) 'The install removed an unrelated asset.'
    $backups = @(Get-ChildItem -LiteralPath (Join-Path $blobDirectory 'backups') -Filter 'custom.css.*.bak')
    Assert-Equal 1 $backups.Count 'The first install did not create a CSS backup.'
    Assert-Equal 'old-css-sentinel' ([System.IO.File]::ReadAllText($backups[0].FullName)) 'The CSS backup does not preserve the original file.'
    Assert-Equal 'unit-test-key' (Read-BlobUiProtectedCredential -Path $firstInstall.credentialPath) 'The encrypted API key did not decrypt for the current user.'
    $savedData = Read-BlobUiDataFile -Path $firstInstall.dataPath
    Assert-Equal $installDirectory $savedData.installDirectory 'The successful install directory was not saved.'
    Assert-True (-not ([System.IO.File]::ReadAllText($firstInstall.dataPath).Contains('unit-test-key'))) 'The plaintext API key was written to data.json.'

    $repeatInstallArguments = @{
        InstallDirectory = $installDirectory
        RuntimeSourceDirectory = $repoRoot
        AssetsSourceDirectory = Join-Path $repoRoot 'assets'
        CompiledCssPath = $compiledCssPath
        DataCandidatePath = $candidateDataPath
    }
    Invoke-BlobUiInstall @repeatInstallArguments | Out-Null
    $backups = @(Get-ChildItem -LiteralPath (Join-Path $blobDirectory 'backups') -Filter 'custom.css.*.bak')
    Assert-Equal 2 $backups.Count 'A repeated install did not create a fresh CSS backup.'
    Assert-Equal 'unit-test-key' (Read-BlobUiProtectedCredential -Path $firstInstall.credentialPath) 'A repeated install damaged the encrypted API key.'

    $rebuildArguments = @{ ApiTransport = $apiTransport }
    & (Join-Path $firstInstall.runtimePath 'Rebuild.ps1') @rebuildArguments
    $backups = @(Get-ChildItem -LiteralPath (Join-Path $blobDirectory 'backups') -Filter 'custom.css.*.bak')
    Assert-Equal 3 $backups.Count 'Rebuild did not install using the saved target directory and create a CSS backup.'
    $rebuiltData = Read-BlobUiDataFile -Path $firstInstall.dataPath
    Assert-Equal $installDirectory $rebuiltData.installDirectory 'Rebuild changed the saved install directory.'
    Assert-Equal 'unit-test-key' (Read-BlobUiProtectedCredential -Path $firstInstall.credentialPath) 'Rebuild damaged the encrypted API key.'

    Write-Host 'BlobUI isolated workflow checks passed.'
}
finally {
    if (Test-Path -LiteralPath $testRoot) {
        Remove-Item -LiteralPath $testRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
