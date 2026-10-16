$ErrorActionPreference = 'Stop'
$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$scriptsDirectory = Join-Path $repoRoot 'scripts'
$testRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '.tmp'))
$testsRoot = [System.IO.Path]::GetFullPath($PSScriptRoot).TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
if (-not $testRoot.StartsWith($testsRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw 'The test output directory is outside the tests folder.'
}
if (Test-Path -LiteralPath $testRoot) {
    Remove-Item -LiteralPath $testRoot -Recurse -Force
}
[System.IO.Directory]::CreateDirectory($testRoot) | Out-Null
Import-Module (Join-Path $scriptsDirectory 'BlobUI.Core.psm1') -Force

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

function New-TestSettingObject {
    param([string]$Id, [string]$Name, [System.Collections.IDictionary]$Properties)
    $documentProperties = [ordered]@{ Name = $Name }
    foreach ($key in $Properties.Keys) {
        $documentProperties[$key] = $Properties[$key]
    }
    return @{
        '$schema' = 'https://schemas.anytype.io/anyblock/2.0/object.schema.json'
        formatVersion = '2.0'
        etag = 'test-etag'
        id = $Id
        type = 'Setting'
        properties = $documentProperties
        blocks = @()
    }
}

$testSpaceId = 'bafy-space-one-cid.replication-key'
$otherSpaceId = 'bafy-space-two-cid.replication-key'
$generalObjectId = 'bafy-general-settings-object-id'
$spaceObjectId = 'bafy-space-settings-object-id'
$emptyObjectId = 'bafy-empty-settings-object-id'
$calendarBlueObjectId = 'bafy-calendar-blue-object-id'
$calendarBlobPinkObjectId = 'bafy-calendar-blob-pink-object-id'
$calendarHexObjectId = 'bafy-calendar-hex-object-id'
$calendarRgbObjectId = 'bafy-calendar-rgb-object-id'
$calendarDefaultObjectId = 'bafy-calendar-default-object-id'
$calendarUncoloredObjectId = 'bafy-calendar-uncolored-object-id'
$otherSpaceCalendarObjectId = 'bafy-other-space-calendar-object-id'
$staleCalendarObjectId = 'bafy-calendar-stale-object-id'
$calendarColorPropertyKey = 'calendar_color'
$otherSpaceCalendarColorPropertyKey = 'calendar_tint'
$apiRequests = [System.Collections.Generic.List[object]]::new()
$knownKeys = @('unit-test-key', 'default-accent-key', 'bad-color-key', 'bad-font-key', 'bad-banner-key', 'no-calendar-color-key')
$apiTransport = {
    param($ApiKey, $Method, $Path, $Body, $Headers)

    function ConvertTo-TestApiObject {
        param($InputObject)
        $json = ConvertTo-Json -InputObject $InputObject -Depth 20 -Compress
        return ConvertFrom-Json -InputObject $json -ErrorAction Stop
    }
    function New-TestSettingObject {
        param([string]$Id, [string]$Name, [System.Collections.IDictionary]$Properties)
        $documentProperties = [ordered]@{ Name = $Name }
        foreach ($key in $Properties.Keys) {
            $documentProperties[$key] = $Properties[$key]
        }
        return @{
            '$schema' = 'https://schemas.anytype.io/anyblock/2.0/object.schema.json'
            formatVersion = '2.0'
            etag = 'test-etag'
            id = $Id
            type = 'Setting'
            properties = $documentProperties
            blocks = @()
        }
    }

    $apiRequests.Add([pscustomobject]@{
        ApiKey = [string]$ApiKey
        Method = [string]$Method
        Path = [string]$Path
        Body = $Body
        Headers = $Headers
    })
    if ($Headers.ContainsKey('Anytype-Version')) {
        throw 'The v2 mock received the v1-only Anytype-Version header.'
    }
    if ($knownKeys -cnotcontains [string]$ApiKey) {
        $exception = [System.Exception]::new('Mock API rejected the key.')
        $exception.Data['StatusCode'] = 401
        throw $exception
    }

    $cleanPath = ([string]$Path).Split('?')[0]
    $offset = 0
    if ([string]$Path -match '(?:\?|&)offset=(?<offset>\d+)') {
        $offset = [int]$Matches.offset
    }

    if ($cleanPath -eq '/v2/spaces') {
        if ($Method -cne 'Get' -or [string]$Path -notmatch '(?:\?|&)ids=full(?:&|$)') {
            throw 'The mock API expected a v2 space list request with full space IDs.'
        }
        if ($offset -eq 0) {
            $payload = @{
                data = @(@{ id = $testSpaceId; name = 'BlobUI Settings' })
                has_more = $true
                has_not_granted_spaces = $false
                limit = 100
                offset = 0
                total = 2
            }
        }
        else {
            $payload = @{
                data = @(@{ id = $otherSpaceId; name = 'Other Space' })
                has_more = $false
                has_not_granted_spaces = $false
                limit = 100
                offset = $offset
                total = 2
            }
        }
    }
    elseif ($cleanPath -eq "/v2/spaces/$testSpaceId/search") {
        if ($Method -cne 'Post' -or $null -eq $Body) {
            throw 'The mock API expected a read-only v2 search POST with a JSON body.'
        }
        if ([string]$Body.type -ceq 'setting') {
            if ($offset -eq 0) {
                $payload = @{
                    data = @(@{ id = $generalObjectId; name = 'General Settings'; type = 'setting' })
                    has_more = $true
                    limit = 100
                    offset = 0
                    total = 2
                    warnings = @()
                }
            }
            else {
                $payload = @{
                    data = @(@{ id = $spaceObjectId; name = 'Space Specific Settings'; type = 'setting' })
                    has_more = $false
                    limit = 100
                    offset = $offset
                    total = 2
                    warnings = @()
                }
            }
        }
        elseif ([string]::Join(',', @($Body.fields)) -ceq $calendarColorPropertyKey) {
            if ($offset -eq 0) {
                $payload = @{
                    data = @(
                        @{
                            id = $calendarBlueObjectId
                            name = 'Blue calendar record'
                            type = 'task'
                            properties = @{ $calendarColorPropertyKey = @{ format = 'select'; select = @{ name = 'Blue' } } }
                        },
                        @{
                            id = $calendarBlobPinkObjectId
                            name = 'Blob Pink calendar record'
                            type = 'page'
                            properties = @{ $calendarColorPropertyKey = @{ format = 'select'; select = @{ name = 'Blob Pink' } } }
                        },
                        @{
                            id = $calendarHexObjectId
                            name = 'Hex calendar record'
                            type = 'page'
                            properties = @{ $calendarColorPropertyKey = @{ format = 'text'; text = '#de50de' } }
                        },
                        @{
                            id = $calendarRgbObjectId
                            name = 'RGB calendar record'
                            type = 'page'
                            properties = @{ $calendarColorPropertyKey = @{ format = 'text'; text = 'rgba(12, 34, 56, 0.5)' } }
                        }
                    )
                    has_more = $true
                    limit = 100
                    offset = 0
                    total = 6
                    warnings = @()
                }
            }
            else {
                $payload = @{
                    data = @(
                        @{
                            id = $calendarDefaultObjectId
                            name = 'Default calendar record'
                            type = 'task'
                            properties = @{ $calendarColorPropertyKey = @{ format = 'select'; select = @{ name = 'Default' } } }
                        },
                        @{
                            id = $calendarUncoloredObjectId
                            name = 'Uncolored calendar record'
                            type = 'page'
                            properties = @{}
                        }
                    )
                    has_more = $false
                    limit = 100
                    offset = $offset
                    total = 6
                    warnings = @()
                }
            }
        }
        else {
            throw 'The mock API expected either the Setting type or the Calendar Color property key.'
        }
    }
    elseif ($cleanPath -eq "/v2/spaces/$otherSpaceId/search") {
        if ([string]$Body.type -ceq 'setting') {
            throw 'Settings discovery ran outside the designated settings space.'
        }
        if ([string]::Join(',', @($Body.fields)) -ceq $otherSpaceCalendarColorPropertyKey) {
            $payload = @{
                data = @(
                    @{
                        id = $otherSpaceCalendarObjectId
                        name = 'Other space calendar record'
                        type = 'task'
                        properties = @{ $otherSpaceCalendarColorPropertyKey = @{ format = 'select'; select = @{ name = 'Green' } } }
                    }
                )
                has_more = $false
                limit = 100
                offset = 0
                total = 1
                warnings = @()
            }
        }
        else {
            throw 'The mock API expected the Calendar Color property key in the other space.'
        }
    }
    elseif ($cleanPath -eq "/v2/spaces/$testSpaceId/properties") {
        if ($ApiKey -eq 'no-calendar-color-key') {
            $payload = @{
                data = @()
                has_more = $false
                limit = 100
                offset = 0
                total = 0
            }
        }
        else {
            $payload = @{
                data = @(@{ id = 'bafy-calendar-color-property-id'; key = $calendarColorPropertyKey; name = 'Calendar Color'; format = 'select'; object = 'property' })
                has_more = $false
                limit = 100
                offset = 0
                total = 1
            }
        }
    }
    elseif ($cleanPath -eq "/v2/spaces/$otherSpaceId/properties") {
        if ($ApiKey -eq 'no-calendar-color-key') {
            $payload = @{
                data = @()
                has_more = $false
                limit = 100
                offset = 0
                total = 0
            }
        }
        else {
            $payload = @{
                data = @(@{ id = 'bafy-other-space-calendar-color-property-id'; key = $otherSpaceCalendarColorPropertyKey; name = 'Calendar Color'; format = 'select'; object = 'property' })
                has_more = $false
                limit = 100
                offset = 0
                total = 1
            }
        }
    }
    elseif ($cleanPath -eq "/v2/spaces/$testSpaceId/objects/$generalObjectId") {
        if ($Method -cne 'Get' -or [string]$Path -notmatch '\?include=properties&ids=full$') {
            throw 'The mock API expected a v2 full-ID property read.'
        }
        if ($ApiKey -eq 'default-accent-key') {
            $properties = @{ 'Accent Color' = 'Default' }
        }
        elseif ($ApiKey -eq 'bad-color-key') {
            $properties = @{ 'Accent Color' = '#de50de; color:red' }
        }
        elseif ($ApiKey -eq 'bad-font-key') {
            $properties = @{ Font = 'Segoe UI; background:url(x)' }
        }
        else {
            $properties = @{
                Font = 'Libron'
                'Code Font' = 'Consolas'
                'Accent Color' = '#DE50DE'
            }
        }
        $payload = New-TestSettingObject $generalObjectId 'General Settings' $properties
    }
    elseif ($cleanPath -eq "/v2/spaces/$testSpaceId/objects/$spaceObjectId") {
        if ($ApiKey -eq 'bad-banner-key') {
            $bannerValue = '..\outside.png'
        }
        else {
            $bannerValue = 'assets\spacebanner\banner.png'
        }
        $properties = @{ Banner = $bannerValue }
        $payload = New-TestSettingObject $spaceObjectId 'Space Specific Settings' $properties
    }
    elseif ($cleanPath -eq "/v2/spaces/$testSpaceId/objects/$emptyObjectId") {
        $payload = New-TestSettingObject $emptyObjectId 'Empty Settings' @{}
    }
    else {
        $exception = [System.Exception]::new('Mock API object not found.')
        $exception.Data['StatusCode'] = 404
        throw $exception
    }
    return ConvertTo-TestApiObject $payload
}.GetNewClosure()

$markerApiTransport = {
    param($ApiKey, $Method, $Path, $Body, $Headers)

    function New-MarkerApiObject {
        param($InputObject)
        return ConvertFrom-Json -InputObject (ConvertTo-Json -InputObject $InputObject -Depth 20 -Compress) -ErrorAction Stop
    }

    $cleanPath = ([string]$Path).Split('?')[0]
    if ($cleanPath -eq '/v2/spaces') {
        $payload = @{
            data = @(
                @{ id = $testSpaceId; name = 'BlobUI Settings' },
                @{ id = $otherSpaceId; name = 'Project Space' }
            )
            has_more = $false
        }
    }
    elseif ($cleanPath -eq "/v2/spaces/$testSpaceId/properties" -or $cleanPath -eq "/v2/spaces/$otherSpaceId/properties") {
        $payload = @{
            data = @(@{ id = 'blob-id-property'; key = 'blob_id'; name = 'BlobID'; format = 'text' })
            has_more = $false
        }
    }
    elseif ($cleanPath -eq "/v2/spaces/$testSpaceId/search" -or $cleanPath -eq "/v2/spaces/$otherSpaceId/search") {
        if ($Method -cne 'Post' -or [string]::Join(',', @($Body.fields)) -cne 'blob_id' -or [string]$Body.type -cne 'setting') {
            throw 'The marker mock expected a typed BlobID field search.'
        }
        if ($cleanPath -eq "/v2/spaces/$testSpaceId/search") {
            $rows = @(@{
                id = $generalObjectId
                type = 'setting'
                properties = @{ blob_id = @{ format = 'text'; text = 'BlobUI-global-settings' } }
            })
            if ($ApiKey -eq 'marker-duplicate-global-key') {
                $rows += @(@{
                    id = 'bafy-duplicate-global-settings-object'
                    type = 'setting'
                    properties = @{ blob_id = @{ format = 'text'; text = 'BlobUI-global-settings' } }
                })
            }
        }
        else {
            $spaceMarkerRow = @{
                id = $spaceObjectId
                type = 'setting'
                properties = @{ blob_id = @{ format = 'text'; text = 'BlobUI-space-settings' } }
            }
            $rows = @($spaceMarkerRow)
            if ($ApiKey -eq 'marker-duplicate-space-key') {
                $rows += @(@{
                    id = 'bafy-duplicate-space-settings-object'
                    type = 'setting'
                    properties = @{ blob_id = @{ format = 'text'; text = 'BlobUI-space-settings' } }
                })
            }
        }
        $payload = @{ data = $rows; has_more = $false }
    }
    elseif ($cleanPath -match '^/v2/spaces/(?<space>[^/]+)/objects/(?<id>[^/]+)$') {
        $objectId = [string]$Matches.id
        if ($objectId -eq $generalObjectId -or $objectId -eq 'bafy-duplicate-global-settings-object') {
            $payload = @{
                id = $objectId
                type = 'setting'
                name = 'A name that does not control discovery'
                properties = @{
                    blob_id = @{ format = 'text'; text = 'BlobUI-global-settings' }
                    font = @{ format = 'text'; text = 'Aptos' }
                    code_font = @{ format = 'text'; text = 'Cascadia Mono' }
                    accent_color = @{ format = 'text'; text = '#336699' }
                    banner = @{ format = 'text'; text = 'assets/spacebanner/nested/global-banner' }
                }
            }
        }
        elseif ($objectId -eq $spaceObjectId -or $objectId -eq 'bafy-duplicate-space-settings-object') {
            $spaceBanner = if ($ApiKey -eq 'marker-empty-space-key') { '' } elseif ($ApiKey -eq 'marker-traversal-space-key') { '..\\escape.png' } else { 'nested/space-banner' }
            $payload = @{
                id = $objectId
                type = 'setting'
                name = 'Any name is allowed here'
                properties = @{
                    blob_id = @{ format = 'text'; text = 'BlobUI-space-settings' }
                    banner_2 = @{ format = 'text'; text = $spaceBanner }
                }
            }
        }
        else {
            $exception = [System.Exception]::new('Marker mock object not found.')
            $exception.Data['StatusCode'] = 404
            throw $exception
        }
    }
    else {
        throw "Unexpected marker mock request: $Method $Path"
    }
    return New-MarkerApiObject $payload
}.GetNewClosure()

$unavailableApiTransport = {
    param($ApiKey, $Method, $Path, $Body, $Headers)
    throw [System.Net.Sockets.SocketException]::new([System.Net.Sockets.SocketError]::ConnectionRefused)
}

Push-Location $testRoot
try {
    $installDirectory = Join-Path $testRoot 'Anytype Data ü #1'
    [System.IO.Directory]::CreateDirectory($installDirectory) | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $installDirectory 'unrelated-anytype-file.txt'), 'keep-anytype-file')
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
    & (Join-Path $scriptsDirectory 'Get-Data.ps1') @getArguments
    $fetchedData = Read-BlobUiDataFile -Path $candidateDataPath

    Assert-Equal $testSpaceId $fetchedData.selectedSpace.id 'The selected space ID was not saved in its full v2 form.'
    Assert-Equal 'BlobUI Settings' $fetchedData.selectedSpace.name 'The selected space name was not saved.'
    Assert-Equal 'v2' $fetchedData.apiVersion 'The saved API version was not updated.'
    Assert-Equal 'Libron' $fetchedData.settings.fontFamily 'The exported Font property was not mapped.'
    Assert-Equal 'Consolas' $fetchedData.settings.codeFontFamily 'The exported Code Font property was not mapped.'
    Assert-Equal '#DE50DE' $fetchedData.settings.accentColor 'The exported Accent Color property was not mapped.'
    Assert-Equal 'BlobID' $fetchedData.settingsSource 'A new install did not use marker-based settings discovery.'
    Assert-True ($null -eq $fetchedData.settings.PSObject.Properties['bannerPath']) 'The new settings data retained a banner setting.'
    Assert-Equal 0 @($fetchedData.settingsObjectIds).Count 'Marker-mode settings unexpectedly depended on manually selected object IDs.'

    $markerInstallDirectory = Join-Path $testRoot 'Marker Anytype Data'
    [System.IO.Directory]::CreateDirectory($markerInstallDirectory) | Out-Null
    $markerDataPath = Join-Path $testRoot 'marker-settings.json'
    Get-BlobUiSettingsData -ApiKey 'marker-key' -OutputPath $markerDataPath -SpaceId $testSpaceId -InstallDirectory $markerInstallDirectory -ApiTransport $markerApiTransport | Out-Null
    $markerData = Read-BlobUiDataFile -Path $markerDataPath
    Assert-Equal 'BlobID' $markerData.settingsSource 'New settings did not record marker-based discovery.'
    Assert-Equal $generalObjectId ([string]$markerData.settingsObjectIds[0]) 'BlobUI selected settings by object name instead of the global BlobID marker.'
    Assert-Equal 'Aptos' $markerData.settings.fontFamily 'The marked global settings object was not read.'
    Assert-True ($null -eq $markerData.settings.PSObject.Properties['bannerPath']) 'BlobUI retained a configurable global banner setting.'
    Assert-Equal 1 @($markerData.spaceSettings).Count 'The marked per-space settings object was not associated with its space.'
    Assert-Equal $otherSpaceId $markerData.spaceSettings[0].id 'Per-space settings were not keyed by the actual Anytype space ID.'
    Assert-Equal 'Project Space' $markerData.spaceSettings[0].name 'The per-space display label was not retained for diagnostics.'
    Assert-True ($null -eq $markerData.spaceSettings[0].PSObject.Properties['bannerPath']) 'BlobUI retained a per-space Banner setting.'
    $markerBadDataPath = Join-Path $testRoot 'marker-invalid-settings.json'
    Assert-Throws {
        Get-BlobUiSettingsData -ApiKey 'marker-duplicate-global-key' -OutputPath $markerBadDataPath -SpaceId $testSpaceId -InstallDirectory $markerInstallDirectory -ApiTransport $markerApiTransport
    } 'Duplicate global settings' 'Multiple BlobUI-global-settings objects'
    Assert-Throws {
        Get-BlobUiSettingsData -ApiKey 'marker-duplicate-space-key' -OutputPath $markerBadDataPath -SpaceId $testSpaceId -InstallDirectory $markerInstallDirectory -ApiTransport $markerApiTransport
    } 'Duplicate per-space settings' 'Multiple BlobUI-space-settings objects'
    Get-BlobUiSettingsData -ApiKey 'marker-traversal-space-key' -OutputPath $markerBadDataPath -SpaceId $testSpaceId -InstallDirectory $markerInstallDirectory -ApiTransport $markerApiTransport | Out-Null
    $markerTraversalData = Read-BlobUiDataFile -Path $markerBadDataPath
    Assert-Equal 1 @($markerTraversalData.spaceSettings).Count 'An imported per-space Banner value affected settings discovery.'

    $markerEmptyDataPath = Join-Path $testRoot 'marker-empty-space-settings.json'
    Get-BlobUiSettingsData -ApiKey 'marker-empty-space-key' -OutputPath $markerEmptyDataPath -SpaceId $testSpaceId -InstallDirectory $markerInstallDirectory -ApiTransport $markerApiTransport | Out-Null
    $markerEmptyData = Read-BlobUiDataFile -Path $markerEmptyDataPath
    $markerAssetsDirectory = Join-Path $testRoot 'Marker assets'
    [System.IO.Directory]::CreateDirectory((Join-Path $markerAssetsDirectory 'spacebanner')) | Out-Null
    [System.IO.File]::Copy((Join-Path $repoRoot 'assets\spacebanner\banner.png'), (Join-Path $markerAssetsDirectory 'spacebanner\banner.png'))
    $markerCompilePath = Join-Path $testRoot 'marker-compiled.css'
    $markerCompile = Invoke-BlobUiCssCompile -DevDirectory (Join-Path $repoRoot 'Dev') -AssetSourceDirectory $markerAssetsDirectory -OutputPath $markerCompilePath -Data $markerEmptyData
    Assert-Equal ([System.Uri]::new((Join-Path $markerInstallDirectory 'blob\assets\spacebanner\banner.png')).AbsoluteUri) $markerCompile.bannerUri 'The compiler did not use the fixed banner filename.'
    $markerLegacyBannerData = ConvertFrom-Json -InputObject (ConvertTo-Json -InputObject $markerData -Depth 20)
    $markerLegacyBannerData.settings | Add-Member -NotePropertyName bannerPath -NotePropertyValue '..\outside.png' -Force
    $markerLegacyBannerData.spaceSettings[0] | Add-Member -NotePropertyName bannerPath -NotePropertyValue '..\outside.png' -Force
    $markerLegacyBannerCompile = Invoke-BlobUiCssCompile -DevDirectory (Join-Path $repoRoot 'Dev') -AssetSourceDirectory (Join-Path $repoRoot 'assets') -OutputPath (Join-Path $testRoot 'marker-legacy-banner.css') -Data $markerLegacyBannerData
    Assert-Equal ([System.Uri]::new((Join-Path $markerInstallDirectory 'blob\assets\spacebanner\banner.png')).AbsoluteUri) $markerLegacyBannerCompile.bannerUri 'A saved per-space Banner value blocked compilation or changed the fixed image.'

    Assert-Equal 'Blue' $fetchedData.calendarObjectColors.$calendarBlueObjectId 'The Calendar Color property was not read from the independent space search.'
    Assert-Equal 'Blob Pink' $fetchedData.calendarObjectColors.$calendarBlobPinkObjectId 'Blob Pink was not preserved as a canonical calendar color.'
    Assert-Equal '#DE50DE' $fetchedData.calendarObjectColors.$calendarHexObjectId 'A hexadecimal Calendar Color value was not normalized.'
    Assert-Equal 'rgba(12, 34, 56, 0.5)' $fetchedData.calendarObjectColors.$calendarRgbObjectId 'An rgba() Calendar Color value was not preserved.'
    Assert-Equal 'Green' $fetchedData.calendarObjectColors.$otherSpaceCalendarObjectId 'Calendar Color discovery did not include another API-accessible space.'
    Assert-True ($null -eq $fetchedData.calendarObjectColors.PSObject.Properties[$calendarDefaultObjectId]) 'Default did not remove its calendar color override.'
    Assert-True ($null -eq $fetchedData.calendarObjectColors.PSObject.Properties[$calendarUncoloredObjectId]) 'An object without Calendar Color received an override.'
    Assert-True ($fetchedData.calendarObjectColors.PSObject.Properties[$calendarBlueObjectId] -and $fetchedData.settingsObjectIds -notcontains $calendarBlueObjectId) 'Calendar color collection reused selected theme settings objects instead of the space object search.'
    $settingsSearchCalls = @($apiRequests | Where-Object { $_.Method -eq 'Post' -and [string]$_.Body.type -ceq 'setting' })
    Assert-Equal 0 $settingsSearchCalls.Count 'Settings discovery searched by object type without resolving the BlobID property first.'
    $spaceListCalls = @($apiRequests | Where-Object { $_.Method -eq 'Get' -and $_.Path.StartsWith('/v2/spaces?') })
    Assert-True ($spaceListCalls.Count -ge 2) 'The v2 space list did not follow has_more across pages.'
    Assert-True ($spaceListCalls[0].Path -match '[?&]ids=full&limit=100&offset=0$') 'The v2 space list did not request full IDs and pagination.'
    Assert-True ($spaceListCalls[1].Path -match '[?&]offset=1$') 'The second v2 space page did not advance by the returned item count.'
    Assert-True (@($apiRequests | Where-Object { $_.Headers.ContainsKey('Anytype-Version') }).Count -eq 0) 'A v1-only Anytype-Version header was sent to API v2.'
    Assert-True (@($apiRequests | Where-Object { $_.Path -notmatch '^/v2/' }).Count -eq 0) 'The API transport made a request outside v2.'
    Assert-Equal 0 @($apiRequests | Where-Object { $_.Path -match '/objects/[^?]+\?include=properties&ids=full$' }).Count 'Marker mode read unmarked objects by name or position.'
    $calendarSearchCalls = @($apiRequests | Where-Object { $_.Method -eq 'Post' -and $_.Path.StartsWith("/v2/spaces/$testSpaceId/search") -and [string]::Join(',', @($_.Body.fields)) -ceq $calendarColorPropertyKey })
    Assert-Equal 2 $calendarSearchCalls.Count 'The Calendar Color object search did not follow has_more across pages.'
    Assert-Equal $calendarColorPropertyKey ([string]$calendarSearchCalls[0].Body.fields[0]) 'The Calendar Color search did not request the API property key resolved by name.'
    $otherSpaceCalendarSearchCalls = @($apiRequests | Where-Object { $_.Method -eq 'Post' -and $_.Path.StartsWith("/v2/spaces/$otherSpaceId/search") -and [string]::Join(',', @($_.Body.fields)) -ceq $otherSpaceCalendarColorPropertyKey })
    Assert-Equal 1 $otherSpaceCalendarSearchCalls.Count 'Calendar Color was not searched in every accessible space where the property exists.'

    $emptyDataPath = Join-Path $testRoot 'empty-data.json'
    $emptyData = Get-BlobUiSettingsData -ApiKey 'unit-test-key' -OutputPath $emptyDataPath -SpaceId $testSpaceId -ObjectIds @($emptyObjectId) -InstallDirectory $installDirectory -ApiTransport $apiTransport
    Assert-Equal 'Libron' $emptyData.settings.fontFamily 'The Font default is incorrect.'
    Assert-Equal 'Consolas' $emptyData.settings.codeFontFamily 'The Code Font default is incorrect.'
    Assert-Equal '#DE50DE' $emptyData.settings.accentColor 'The Accent Color default is incorrect.'
    Assert-True ($null -eq $emptyData.settings.PSObject.Properties['bannerPath']) 'The default settings data retained a banner setting.'

    $defaultAccentDataPath = Join-Path $testRoot 'default-accent-data.json'
    $defaultAccentData = Get-BlobUiSettingsData -ApiKey 'default-accent-key' -OutputPath $defaultAccentDataPath -SpaceId $testSpaceId -ObjectIds @($generalObjectId) -InstallDirectory $installDirectory -ApiTransport $apiTransport
    Assert-Equal 'var(--color-system-accent-100)' $defaultAccentData.settings.accentColor 'Accent Color Default did not resolve to Anytype system accent blue.'
    Assert-Throws { ConvertTo-BlobUiCssPlaceholderValue -Type color -Value 'Default' } 'Default is unavailable to generic color settings.' 'does not accept Default'
    Assert-Throws { ConvertTo-BlobUiCssPlaceholderValue -Type color -Value 'var(--color-system-accent-100)' } 'Anytype system accent blue is unavailable to generic color settings.' 'three or six digit hexadecimal'
    $defaultAccentCssPath = Join-Path $testRoot 'default-accent.css'
    Invoke-BlobUiCssCompile -DevDirectory (Join-Path $repoRoot 'Dev') -AssetSourceDirectory (Join-Path $repoRoot 'assets') -OutputPath $defaultAccentCssPath -Data $defaultAccentData | Out-Null
    $defaultAccentCss = [System.IO.File]::ReadAllText($defaultAccentCssPath)
    Assert-True ($defaultAccentCss.Contains('--color-blob-accent: var(--color-system-accent-100);')) 'Compiled CSS did not preserve Anytype system accent blue for Accent Color Default.'
    Assert-True (-not $defaultAccentCss.Contains('html .switch.active.blue')) 'Accent Color Default overrides Anytype''s native toggle color.'
    Assert-True (-not $defaultAccentCss.Contains('#widget-home>div>div>div>div>div.icon.settingsHome.iconColor.iconColor-red.hasSvg')) 'Accent Color Default still overrides Anytype''s original home icon color.'
    $literalDefaultAccentData = ConvertFrom-Json -InputObject (ConvertTo-Json -InputObject $defaultAccentData -Depth 20)
    $literalDefaultAccentData.settings.accentColor = '  dEfAuLt  '
    $literalDefaultAccentCssPath = Join-Path $testRoot 'literal-default-accent.css'
    Invoke-BlobUiCssCompile -DevDirectory (Join-Path $repoRoot 'Dev') -AssetSourceDirectory (Join-Path $repoRoot 'assets') -OutputPath $literalDefaultAccentCssPath -Data $literalDefaultAccentData | Out-Null
    $literalDefaultAccentCss = [System.IO.File]::ReadAllText($literalDefaultAccentCssPath)
    Assert-True ($literalDefaultAccentCss.Contains('--color-blob-accent: var(--color-system-accent-100);')) 'A raw Default accent did not normalize to Anytype system accent blue.'
    Assert-True (-not $literalDefaultAccentCss.Contains('html .switch.active.blue')) 'A raw Default accent overrides Anytype''s native toggle color.'
    Assert-True (-not $literalDefaultAccentCss.Contains('#widget-home>div>div>div>div>div.icon.settingsHome.iconColor.iconColor-red.hasSvg')) 'A raw Default accent still overrides Anytype''s original home icon color.'

    $legacyDataPath = Join-Path $testRoot 'legacy-data.json'
    Write-BlobUiAtomicJson -Path $legacyDataPath -InputObject ([ordered]@{
        schemaVersion = 1
        apiVersion = '2025-11-08'
        selectedSpace = [ordered]@{ id = $testSpaceId; name = 'BlobUI Settings' }
        settingsObjectIds = @($generalObjectId, $spaceObjectId)
        settings = $fetchedData.settings
        installDirectory = $installDirectory
    })
    $legacyBannerPath = Join-Path $installDirectory 'blob\assets\spacebanner\banner.png'
    [System.IO.Directory]::CreateDirectory((Split-Path -Parent $legacyBannerPath)) | Out-Null
    [System.IO.File]::WriteAllText($legacyBannerPath, 'legacy-user-banner')
    $legacyDataForBanner = Read-BlobUiDataFile -Path $legacyDataPath
    $legacyDataForBanner.settings | Add-Member -NotePropertyName bannerPath -NotePropertyValue 'spacebanner\custom.png' -Force
    Write-BlobUiAtomicJson -Path $legacyDataPath -InputObject $legacyDataForBanner
    $legacyRefreshPath = Join-Path $testRoot 'legacy-refresh.json'
    $legacyRefresh = Get-BlobUiSettingsData -ApiKey 'unit-test-key' -OutputPath $legacyRefreshPath -SpaceId $testSpaceId -ObjectIds @($generalObjectId, $spaceObjectId) -ExistingDataPath $legacyDataPath -ApiTransport $apiTransport
    Assert-Equal $testSpaceId $legacyRefresh.selectedSpace.id 'A saved settings record from the previous API version lost its full space ID.'
    Assert-Equal "$generalObjectId|$spaceObjectId" $legacyRefresh.settingsObjectIds 'Saved object IDs from the previous API version were not accepted by v2.'
    Assert-True ($null -eq $legacyRefresh.settings.PSObject.Properties['bannerPath']) 'Refreshing legacy data retained a banner setting.'
    Assert-Equal $installDirectory $legacyRefresh.installDirectory 'A previous settings record did not preserve its install directory during refresh.'

    $staleCalendarDataPath = Join-Path $testRoot 'stale-calendar-data.json'
    Write-BlobUiAtomicJson -Path $staleCalendarDataPath -InputObject ([ordered]@{
        schemaVersion = 1
        apiVersion = 'v2'
        selectedSpace = [ordered]@{ id = $testSpaceId; name = 'BlobUI Settings' }
        settingsObjectIds = @($generalObjectId, $spaceObjectId)
        settings = $fetchedData.settings
        calendarObjectColors = [ordered]@{ $staleCalendarObjectId = 'Red' }
        installDirectory = $installDirectory
    })
    $withoutCalendarPropertyPath = Join-Path $testRoot 'without-calendar-property.json'
    $withoutCalendarProperty = Get-BlobUiSettingsData -ApiKey 'no-calendar-color-key' -OutputPath $withoutCalendarPropertyPath -SpaceId $testSpaceId -ObjectIds @($generalObjectId, $spaceObjectId) -ExistingDataPath $staleCalendarDataPath -ApiTransport $apiTransport
    Assert-Equal 0 $withoutCalendarProperty.calendarObjectColors.Count 'A removed Calendar Color property left stale object overrides in refreshed data.'
    Assert-Equal $installDirectory $withoutCalendarProperty.installDirectory 'Refreshing after Calendar Color removal lost the saved install directory.'

    Assert-Equal 'Segoe UI' (ConvertTo-BlobUiCssPlaceholderValue -Type font -Value 'Segoe UI') 'A valid font name was rejected.'
    Assert-Throws { ConvertTo-BlobUiCssPlaceholderValue -Type font -Value 'Segoe UI; color:red' } 'Unsafe font value' 'font family'
    Assert-Equal '#12ABEF' (ConvertTo-BlobUiCssPlaceholderValue -Type color -Value '#12abef') 'A valid hex color was not normalized.'
    Assert-Equal 'rgb(12, 34, 56)' (ConvertTo-BlobUiCssPlaceholderValue -Type color -Value 'rgb(12, 34, 56)') 'A comma-form rgb() color was rejected.'
    Assert-Equal 'rgba(12 34 56 / 50%)' (ConvertTo-BlobUiCssPlaceholderValue -Type color -Value 'rgba(12 34 56 / 50%)') 'A modern space/slash rgba() color was rejected.'
    Assert-Equal 'rgb(100% 0 0)' (ConvertTo-BlobUiCssPlaceholderValue -Type color -Value 'rgb(100% 0 0)') 'Modern rgb() did not accept valid mixed channel units.'
    Assert-Equal '#FFFFFF' (ConvertTo-BlobUiCssPlaceholderValue -Type color -Value 'White') 'General CSS color settings lost White compatibility.'
    $whiteAccentValues = @(
        'White',
        '#fff',
        '#fFf',
        '#ffffff',
        'rgb(255, 255, 255)',
        'rgb(255 255 255)',
        'rgb(255.0 255.00 255)',
        'rgb(100% 100% 100%)',
        'rgb(100% 255 100%)',
        'rgba(255 255 255 / 50%)',
        'rgba(255, 255, 255, 1)'
    )
    foreach ($whiteAccentValue in $whiteAccentValues) {
        $whiteAccentData = ConvertFrom-Json -InputObject (ConvertTo-Json -InputObject $fetchedData -Depth 20)
        $whiteAccentData.settings.accentColor = $whiteAccentValue
        $whiteAccentCssPath = Join-Path $testRoot ("white-accent-{0}.css" -f [array]::IndexOf($whiteAccentValues, $whiteAccentValue))
        Assert-Throws {
            Invoke-BlobUiCssCompile -DevDirectory (Join-Path $repoRoot 'Dev') -AssetSourceDirectory (Join-Path $repoRoot 'assets') -OutputPath $whiteAccentCssPath -Data $whiteAccentData | Out-Null
        } "White Accent Color '$whiteAccentValue' was accepted." 'not supported'
    }
    $nearWhiteAccentData = ConvertFrom-Json -InputObject (ConvertTo-Json -InputObject $fetchedData -Depth 20)
    $nearWhiteAccentData.settings.accentColor = '#FEFFFF'
    $nearWhiteAccentCssPath = Join-Path $testRoot 'near-white-accent.css'
    Invoke-BlobUiCssCompile -DevDirectory (Join-Path $repoRoot 'Dev') -AssetSourceDirectory (Join-Path $repoRoot 'assets') -OutputPath $nearWhiteAccentCssPath -Data $nearWhiteAccentData | Out-Null
    $nonWhiteRgbAccentData = ConvertFrom-Json -InputObject (ConvertTo-Json -InputObject $fetchedData -Depth 20)
    $nonWhiteRgbAccentData.settings.accentColor = 'rgb(254 255 255)'
    $nonWhiteRgbAccentCssPath = Join-Path $testRoot 'non-white-rgb-accent.css'
    Invoke-BlobUiCssCompile -DevDirectory (Join-Path $repoRoot 'Dev') -AssetSourceDirectory (Join-Path $repoRoot 'assets') -OutputPath $nonWhiteRgbAccentCssPath -Data $nonWhiteRgbAccentData | Out-Null
    Assert-Throws { ConvertTo-BlobUiCssPlaceholderValue -Type color -Value 'red; color:blue' } 'Unsafe color value' 'hexadecimal'
    Assert-Throws { ConvertTo-BlobUiCssPlaceholderValue -Type color -Value 'rgb(256, 0, 0)' } 'Out-of-range RGB channel was accepted.' 'channel outside'
    Assert-Throws { ConvertTo-BlobUiCssPlaceholderValue -Type color -Value 'rgb(100%, 0, 0)' } 'Legacy comma rgb() accepted mixed channel units.' 'mixed rgb()/rgba() channel units'
    Assert-Throws { ConvertTo-BlobUiCssPlaceholderValue -Type color -Value 'rgba(0 0 0 / 1.1)' } 'Out-of-range alpha was accepted.' 'alpha outside'
    Assert-Throws { ConvertTo-BlobUiCssPlaceholderValue -Type color -Value 'rgb(1, 2, 3)); color: red;/*' } 'Injected CSS color was accepted.' 'hexadecimal'
    Assert-Equal '12.5' (ConvertTo-BlobUiCssPlaceholderValue -Type number -Value '12.5') 'A valid CSS number was rejected.'
    Assert-Throws { ConvertTo-BlobUiCssPlaceholderValue -Type number -Value '12px' } 'Unsafe number value' 'CSS number'
    Assert-Equal '1' (ConvertTo-BlobUiCssPlaceholderValue -Type boolean -Value 'true') 'A valid true value was rejected.'
    Assert-Equal '0' (ConvertTo-BlobUiCssPlaceholderValue -Type boolean -Value 'false') 'A valid false value was rejected.'
    Assert-Throws { ConvertTo-BlobUiCssPlaceholderValue -Type boolean -Value 'yes' } 'Unsafe boolean value' 'true or false'
    Assert-Throws { ConvertTo-BlobUiCssPlaceholderValue -Type url -Value 'javascript:alert(1)' } 'Unsafe URL value' 'local file URI'

    $compiledCssPath = Join-Path $testRoot 'compiled.css'
    $compileResult = Invoke-BlobUiCssCompile -DevDirectory (Join-Path $repoRoot 'Dev') -AssetSourceDirectory (Join-Path $repoRoot 'assets') -OutputPath $compiledCssPath -Data $fetchedData
    $css = [System.IO.File]::ReadAllText($compiledCssPath)
    $scriptCompiledCssPath = Join-Path $testRoot 'script-compiled.css'
    & (Join-Path $scriptsDirectory 'Compile.ps1') -DataPath $candidateDataPath -OutputPath $scriptCompiledCssPath
    Assert-True (Test-Path -LiteralPath $scriptCompiledCssPath -PathType Leaf) 'Compile.ps1 did not find Dev and assets from another working directory.'
    Assert-Equal $css ([System.IO.File]::ReadAllText($scriptCompiledCssPath)) 'Compile.ps1 changed the compiled CSS when invoked from another working directory.'
    Assert-Equal '00-settings.css|10-fonts.css|20-sidebar-banner.css|30-icons.css|31-publish-accent.css|32-membership-switch-accent.css|33-membership-join-accent.css|34-toggle-accent.css|35-shortcuts-accent.css|36-dataview-hover-accent.css|37-properties-accent.css|38-date-picker-accent.css|40-calendar.css|50-text-effects.css|60-tab-hover-preview.css' ([string[]]@($compileResult.sourceFiles | ForEach-Object { Split-Path -Leaf $_ })) 'Dev CSS source ordering is incorrect.'
    Assert-True ($css.Contains('--color-blob-pink: #DE50DE;')) 'Compiled CSS omitted the default accent color.'
    Assert-True ($css.Contains('#widget-home>div>div>div>div>div.icon.settingsHome.iconColor.iconColor-red.hasSvg')) 'A custom Accent Color no longer tints the home icon.'
    Assert-True ($css.Contains('html .switch.active.blue')) 'A custom Accent Color no longer tints active blue toggles.'
    $whiteAccentData = ConvertFrom-Json -InputObject (ConvertTo-Json -InputObject $fetchedData -Depth 20)
    $whiteAccentData.settings.accentColor = 'White'
    $whiteAccentCssPath = Join-Path $testRoot 'white-accent.css'
    Assert-Throws { Invoke-BlobUiCssCompile -DevDirectory (Join-Path $repoRoot 'Dev') -AssetSourceDirectory (Join-Path $repoRoot 'assets') -OutputPath $whiteAccentCssPath -Data $whiteAccentData | Out-Null } 'White Accent Color was accepted.' 'White is not supported for Accent Color'
    Assert-True ($css.Contains('[data-view-type="4"][data-drop-type="record"][data-id="' + $calendarBlueObjectId + '"]')) 'Compiled CSS omitted the same-element calendar record selector.'
    Assert-True ($css.Contains('.viewContent.viewCalendar .record[id="record-' + $calendarBlueObjectId + '"]')) 'Compiled CSS omitted the calendar wrapper selector used by non-draggable records.'
    $calendarColorRulesStart = $css.IndexOf('/* BlobUI Calendar Color overrides */', [System.StringComparison]::Ordinal)
    Assert-True ($calendarColorRulesStart -ge 0) 'Compiled CSS omitted the Calendar Color override section.'
    $calendarColorRules = $css.Substring($calendarColorRulesStart)
    Assert-True (-not ($calendarColorRules -match '(?m)^\s*(display|align-items|padding|min-height|height|box-sizing)\s*:')) 'Calendar color overrides changed the event row layout.'
    $calendarMenuRowSelector = '.content > .wrap > .items > .item[id="item-' + $calendarBlueObjectId + '"]:has(> .iconObject[id="menuCalendarDay-' + $calendarBlueObjectId + '-icon"])'
    Assert-True ($calendarColorRules.Contains($calendarMenuRowSelector)) 'Compiled CSS omitted the object-specific calendar day menu row selector.'
    Assert-True (-not ($calendarColorRules.Contains($calendarMenuRowSelector + ' > .iconObject'))) 'Calendar menu color overrides changed the object icon color.'
    Assert-True (-not ($calendarColorRules -match '(?m)^\s*\.menus\s+\.menu')) 'Calendar menu colors were emitted for generic menus.'
    Assert-True ($css.Contains('background-color: var(--color-blue) !important;')) 'Compiled CSS did not use the existing Anytype Blue palette color.'
    Assert-True ($css.Contains('[data-id="' + $calendarBlobPinkObjectId + '"]') -and $css.Contains('background-color: #DE50DE !important;')) 'Compiled CSS did not map Blob Pink to the existing Blob Pink color.'
    Assert-True ($css.Contains('[data-id="' + $calendarHexObjectId + '"]') -and $css.Contains('background-color: #DE50DE !important;')) 'Compiled CSS omitted the hexadecimal Calendar Color value.'
    Assert-True ($css.Contains('[data-id="' + $calendarRgbObjectId + '"]') -and $css.Contains('background-color: rgba(12, 34, 56, 0.5) !important;')) 'Compiled CSS omitted the rgba() Calendar Color value.'
    Assert-True ($css.Contains('[data-id="' + $otherSpaceCalendarObjectId + '"]') -and $css.Contains('background-color: var(--color-green, var(--color-lime)) !important;')) 'Compiled CSS omitted the Calendar Color value from another accessible space.'
    Assert-True (-not $css.Contains($calendarDefaultObjectId)) 'Compiled CSS emitted a selector for Default.'
    Assert-True (-not ($css -match '\[data-id="[^"]+"\]\s+[^\{]*checkbox')) 'Calendar color CSS unexpectedly targets task checkboxes.'
    Assert-True ($css.Contains('font-family: "Libron", sans-serif !important;')) 'Compiled CSS omitted the default UI font.'
    Assert-True ($css.Contains('font-family: "Consolas", "Courier New", monospace !important;')) 'Compiled CSS omitted the default code font.'
    Assert-True ($css.Contains('file:///') -and $css.Contains('Anytype%20Data%20%C3%BC%20%231')) 'The banner file URI did not escape spaces, Unicode, and the hash character.'
    Assert-True (-not $css.Contains('{{')) 'Compiled CSS contains an unresolved placeholder.'

    $paletteObjectColors = [ordered]@{
        'bafy-calendar-palette-grey' = 'Grey'
        'bafy-calendar-palette-yellow' = 'Yellow'
        'bafy-calendar-palette-amber' = 'Amber'
        'bafy-calendar-palette-red' = 'Red'
        'bafy-calendar-palette-pink' = 'Pink'
        'bafy-calendar-palette-purple' = 'Purple'
        'bafy-calendar-palette-blue' = 'Blue'
        'bafy-calendar-palette-sky' = 'Sky'
        'bafy-calendar-palette-teal' = 'Teal'
        'bafy-calendar-palette-green' = 'Green'
        'bafy-calendar-palette-blob-pink' = 'Blob Pink'
        'bafy-calendar-palette-default' = 'Default'
    }
    $paletteData = [ordered]@{
        installDirectory = $installDirectory
        settings = $fetchedData.settings
        calendarObjectColors = $paletteObjectColors
    }
    $paletteCssPath = Join-Path $testRoot 'palette-compiled.css'
    Invoke-BlobUiCssCompile -DevDirectory (Join-Path $repoRoot 'Dev') -AssetSourceDirectory (Join-Path $repoRoot 'assets') -OutputPath $paletteCssPath -Data $paletteData | Out-Null
    $paletteCss = [System.IO.File]::ReadAllText($paletteCssPath)
    foreach ($expectedColor in @(
        'var(--color-grey)',
        'var(--color-yellow)',
        'var(--color-amber, var(--color-orange))',
        'var(--color-red)',
        'var(--color-pink)',
        'var(--color-purple)',
        'var(--color-blue)',
        'var(--color-sky, var(--color-ice))',
        'var(--color-teal)',
        'var(--color-green, var(--color-lime))',
        '#DE50DE'
    )) {
        Assert-True ($paletteCss.Contains('background-color: ' + $expectedColor + ' !important;')) "Calendar palette mapping omitted '$expectedColor'."
    }
    Assert-True (-not $paletteCss.Contains('bafy-calendar-palette-default')) 'The compiler emitted an override for Default.'

    $badCalendarColorData = [ordered]@{
        installDirectory = $installDirectory
        settings = $fetchedData.settings
        calendarObjectColors = [ordered]@{ 'bafy-calendar-bad-color' = 'red; color:blue' }
    }
    $badCalendarColorCssPath = Join-Path $testRoot 'bad-calendar-color.css'
    Assert-Throws {
        Invoke-BlobUiCssCompile -DevDirectory (Join-Path $repoRoot 'Dev') -AssetSourceDirectory (Join-Path $repoRoot 'assets') -OutputPath $badCalendarColorCssPath -Data $badCalendarColorData
    } 'An unsafe calendar property color was accepted.' 'Calendar Color'
    Assert-True (-not (Test-Path -LiteralPath $badCalendarColorCssPath)) 'Invalid calendar color data wrote a CSS file.'

    $badCalendarIdData = [ordered]@{
        installDirectory = $installDirectory
        settings = $fetchedData.settings
        calendarObjectColors = [ordered]@{ 'bafy-calendar-id"]{color:red}/*' = 'Blue' }
    }
    $badCalendarIdCssPath = Join-Path $testRoot 'bad-calendar-id.css'
    Assert-Throws {
        Invoke-BlobUiCssCompile -DevDirectory (Join-Path $repoRoot 'Dev') -AssetSourceDirectory (Join-Path $repoRoot 'assets') -OutputPath $badCalendarIdCssPath -Data $badCalendarIdData
    } 'An unsafe calendar object ID was accepted.' 'invalid Anytype object ID'
    Assert-True (-not (Test-Path -LiteralPath $badCalendarIdCssPath)) 'Invalid calendar object ID data wrote a CSS file.'

    $baselineCss = [System.IO.File]::ReadAllText((Join-Path $PSScriptRoot 'Fixtures\legacy-custom.css'))
    $expectedBannerUri = [System.Uri]::new((Join-Path $installDirectory 'blob\assets\spacebanner\banner.png')).AbsoluteUri
    $baselineCss = $baselineCss.Replace('#de50de', '#DE50DE')
    $baselineCss = [System.Text.RegularExpressions.Regex]::Replace(
        $baselineCss,
        '(?m)^[ \t]*--header-image[ \t]*:.*$',
        '--header-image: url("' + $expectedBannerUri + '");'
    )
    $cssWithoutCalendarOverrides = [System.Text.RegularExpressions.Regex]::Replace($css, '(?s)/\* BlobUI Calendar Color overrides \*/.*$', '')
    # Compare the unchanged sources with the legacy baseline, excluding the new accent fixes.
    $cssWithoutCalendarOverrides = [System.Text.RegularExpressions.Regex]::Replace(
        $cssWithoutCalendarOverrides,
        '(?s)/\* BlobUI source: 3[1-8]-[^*]+\.css \*/.*?(?=/\* BlobUI source: |\z)',
        ''
    )
    Assert-Equal (Get-CssSignature $baselineCss) (Get-CssSignature $cssWithoutCalendarOverrides) 'Compiled CSS changed the prior cascade or default styling.'
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
        Get-BlobUiSettingsData -ApiKey 'bad-color-key' -OutputPath $badDataPath -SpaceId $testSpaceId -ObjectIds @($generalObjectId) -InstallDirectory $installDirectory -ApiTransport $apiTransport
    } 'Invalid API setting' 'Accent Color'
    Assert-True (-not (Test-Path -LiteralPath $badDataPath)) 'Invalid API settings were written to the candidate data file.'
    Assert-Equal 'old-css-sentinel' ([System.IO.File]::ReadAllText($oldCssPath)) 'A settings validation failure changed the installed CSS.'
    Assert-Equal 'old-data-sentinel' ([System.IO.File]::ReadAllText((Join-Path $blobDirectory 'data.json'))) 'A settings validation failure changed saved data.'
    Assert-Throws {
        Get-BlobUiSettingsData -ApiKey 'bad-font-key' -OutputPath $badDataPath -SpaceId $testSpaceId -ObjectIds @($generalObjectId) -InstallDirectory $installDirectory -ApiTransport $apiTransport
    } 'Invalid Font setting' 'Font'
    Get-BlobUiSettingsData -ApiKey 'bad-banner-key' -OutputPath $badDataPath -SpaceId $testSpaceId -ObjectIds @($spaceObjectId) -InstallDirectory $installDirectory -ApiTransport $apiTransport
    $bannerIgnoredData = Read-BlobUiDataFile -Path $badDataPath
    Assert-True ($null -eq $bannerIgnoredData.settings.PSObject.Properties['bannerPath']) 'A legacy unmarked Banner property was retained as a setting.'
    Assert-Equal 'old-css-sentinel' ([System.IO.File]::ReadAllText($oldCssPath)) 'A settings fetch changed the installed CSS.'
    Assert-Equal 'old-data-sentinel' ([System.IO.File]::ReadAllText((Join-Path $blobDirectory 'data.json'))) 'A settings fetch changed saved data.'

    Assert-Throws {
        Get-BlobUiSettingsData -ApiKey 'wrong-key' -OutputPath $badDataPath -SpaceId $testSpaceId -ObjectIds @($generalObjectId) -InstallDirectory $installDirectory -ApiTransport $apiTransport
    } 'Invalid API key' 'rejected the API key'

    $module = Get-Module BlobUI.Core
    $httpErrorCases = @(
        @{ Status = 401; Body = '{"message":"invalid API key"}'; Expected = 'HTTP 401' },
        @{ Status = 403; Body = '{"message":"space grant is read only; Authorization: Bearer secret-key"}'; Expected = 'HTTP 403' },
        @{ Status = 404; Body = '{"message":"unknown route"}'; Expected = 'HTTP 404' },
        @{ Status = 400; Body = 'malformed secret-key and Authorization: Bearer secret-key'; Expected = 'HTTP 400' },
        @{ Status = 500; Body = '{"message":"server failure"}'; Expected = 'HTTP 500' }
    )
    foreach ($httpErrorCase in $httpErrorCases) {
        $caseStatus = $httpErrorCase.Status
        $caseBody = $httpErrorCase.Body
        $failingTransport = {
            param($ApiKey, $Method, $Path, $Body, $Headers)
            $exception = [System.Exception]::new('Mock HTTP failure.')
            $exception.Data['StatusCode'] = $caseStatus
            $exception.Data['ResponseBody'] = $caseBody
            throw $exception
        }.GetNewClosure()
        $caughtMessage = $null
        try {
            & $module { param($ApiKey, $Transport) Invoke-BlobUiApiRequest -ApiKey $ApiKey -Path '/v2/spaces?ids=full' -ApiTransport $Transport } 'secret-key' $failingTransport | Out-Null
        }
        catch {
            $caughtMessage = $_.Exception.Message
        }
        Assert-True ($caughtMessage -like "*$($httpErrorCase.Expected)*") "HTTP $caseStatus was not preserved in the API error."
        Assert-True (-not $caughtMessage.Contains('secret-key')) "HTTP $caseStatus exposed the API key from an echoed error body."
        if ($caseStatus -eq 403) {
            Assert-True ($caughtMessage -like '*space grant is read only*') 'The server message was omitted from a 403 error.'
        }
        if ($caseStatus -eq 404) {
            Assert-True ($caughtMessage -like '*Anytype build provides API v2*') 'A missing v2 route did not report the API v2 prerequisite.'
        }
        if ($caseStatus -eq 500) {
            Assert-True ($caughtMessage -like '*server failure*') 'The server message was omitted from a 500 error.'
        }
    }

    Assert-Throws {
        Get-BlobUiSettingsData -ApiKey 'unit-test-key' -OutputPath $badDataPath -SpaceId $testSpaceId -ObjectIds @($generalObjectId) -InstallDirectory $installDirectory -ApiTransport $unavailableApiTransport
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
    Assert-True (Test-Path -LiteralPath (Join-Path $firstInstall.assetsPath 'spacebanner\banner.png') -PathType Leaf) 'The bundled default banner was not installed.'
    Assert-True (Test-Path -LiteralPath (Join-Path $firstInstall.runtimePath 'scripts\Update.ps1') -PathType Leaf) 'The runtime install did not copy Update.ps1.'
    Assert-True (Test-Path -LiteralPath (Join-Path $firstInstall.runtimePath 'scripts\Uninstall.ps1') -PathType Leaf) 'The runtime install did not copy Uninstall.ps1.'
    Assert-Equal 'keep-me' ([System.IO.File]::ReadAllText((Join-Path $firstInstall.assetsPath 'user-file.txt'))) 'The install removed an unrelated asset.'
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $blobDirectory 'backups'))) 'The install created a persistent backup directory.'
    $backupArtifacts = @(Get-ChildItem -LiteralPath $installDirectory -Recurse -File -Filter '*.bak')
    Assert-Equal 0 $backupArtifacts.Count 'The install left a backup file behind.'
    Assert-Equal 'unit-test-key' (Read-BlobUiProtectedCredential -Path $firstInstall.credentialPath) 'The encrypted API key did not decrypt for the current user.'
    $savedData = Read-BlobUiDataFile -Path $firstInstall.dataPath
    Assert-Equal $installDirectory $savedData.installDirectory 'The successful install directory was not saved.'
    Assert-True (-not ([System.IO.File]::ReadAllText($firstInstall.dataPath).Contains('unit-test-key'))) 'The plaintext API key was written to data.json.'
    $installedManifest = ConvertFrom-Json -InputObject ([System.IO.File]::ReadAllText((Join-Path $blobDirectory '.blobui-managed-files.json')))
    Assert-Equal 0 @($installedManifest.managedFiles | Where-Object { ([string]$_).StartsWith('assets/spacebanner/', [System.StringComparison]::OrdinalIgnoreCase) }).Count 'Banner assets were recorded as package-owned files.'

    $repeatInstallArguments = @{
        InstallDirectory = $installDirectory
        RuntimeSourceDirectory = $repoRoot
        AssetsSourceDirectory = Join-Path $repoRoot 'assets'
        CompiledCssPath = $compiledCssPath
        DataCandidatePath = $candidateDataPath
    }
    Invoke-BlobUiInstall @repeatInstallArguments | Out-Null
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $blobDirectory 'backups'))) 'A repeated install created a persistent backup directory.'
    $backupArtifacts = @(Get-ChildItem -LiteralPath $installDirectory -Recurse -File -Filter '*.bak')
    Assert-Equal 0 $backupArtifacts.Count 'A repeated install left a backup file behind.'
    Assert-Equal 'unit-test-key' (Read-BlobUiProtectedCredential -Path $firstInstall.credentialPath) 'A repeated install damaged the encrypted API key.'

    $installedBannerPath = Join-Path $firstInstall.assetsPath 'spacebanner\banner.png'
    [System.IO.File]::WriteAllBytes($installedBannerPath, [System.Text.Encoding]::UTF8.GetBytes('custom-banner-sentinel'))
    $rebuildArguments = @{ ApiTransport = $apiTransport }
    & (Join-Path $firstInstall.runtimePath 'scripts\Rebuild.ps1') @rebuildArguments
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $blobDirectory 'backups'))) 'Rebuild created a persistent backup directory.'
    $backupArtifacts = @(Get-ChildItem -LiteralPath $installDirectory -Recurse -File -Filter '*.bak')
    Assert-Equal 0 $backupArtifacts.Count 'Rebuild left a backup file behind.'
    $rebuiltData = Read-BlobUiDataFile -Path $firstInstall.dataPath
    Assert-Equal $installDirectory $rebuiltData.installDirectory 'Rebuild changed the saved install directory.'
    Assert-Equal 'unit-test-key' (Read-BlobUiProtectedCredential -Path $firstInstall.credentialPath) 'Rebuild damaged the encrypted API key.'
    Assert-Equal 'custom-banner-sentinel' ([System.Text.Encoding]::UTF8.GetString([System.IO.File]::ReadAllBytes($installedBannerPath))) 'Rebuild replaced the installed custom banner.'

    $rollbackInstallDirectory = Join-Path $testRoot 'Rollback Anytype Data'
    [System.IO.Directory]::CreateDirectory($rollbackInstallDirectory) | Out-Null
    $rollbackBlobDirectory = Join-Path $rollbackInstallDirectory 'blob'
    $rollbackCredentialDirectory = Join-Path $rollbackBlobDirectory 'credentials'
    [System.IO.Directory]::CreateDirectory($rollbackCredentialDirectory) | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $rollbackInstallDirectory 'custom.css'), 'rollback-css-sentinel')
    [System.IO.File]::WriteAllText((Join-Path $rollbackBlobDirectory 'data.json'), 'rollback-data-sentinel')
    $rollbackRuntimeScripts = Join-Path $rollbackBlobDirectory 'runtime\scripts'
    $rollbackRuntimeAssetDirectory = Join-Path $rollbackBlobDirectory 'runtime\assets\spacebanner'
    [System.IO.Directory]::CreateDirectory($rollbackRuntimeScripts) | Out-Null
    [System.IO.Directory]::CreateDirectory($rollbackRuntimeAssetDirectory) | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $rollbackRuntimeScripts 'Rebuild.ps1'), 'rollback-runtime-script-sentinel')
    [System.IO.File]::WriteAllText((Join-Path $rollbackRuntimeAssetDirectory 'banner.png'), 'rollback-runtime-asset-sentinel')
    [System.IO.Directory]::CreateDirectory((Join-Path $rollbackCredentialDirectory 'anytype-api-key.dpapi')) | Out-Null
    $rollbackCandidatePath = Join-Path $testRoot 'rollback-data.json'
    $rollbackCandidate = ConvertFrom-Json -InputObject ([System.IO.File]::ReadAllText($candidateDataPath))
    $rollbackCandidate.installDirectory = $rollbackInstallDirectory
    Write-BlobUiAtomicJson -Path $rollbackCandidatePath -InputObject $rollbackCandidate
    $rollbackArguments = @{
        InstallDirectory = $rollbackInstallDirectory
        RuntimeSourceDirectory = $repoRoot
        AssetsSourceDirectory = Join-Path $repoRoot 'assets'
        CompiledCssPath = $compiledCssPath
        DataCandidatePath = $rollbackCandidatePath
        CredentialProtectedText = $protectedCredential
    }
    Assert-Throws { Invoke-BlobUiInstall @rollbackArguments } 'A credential write failure was accepted.' ''
    Assert-Equal 'rollback-css-sentinel' ([System.IO.File]::ReadAllText((Join-Path $rollbackInstallDirectory 'custom.css'))) 'A failed install did not restore the existing CSS.'
    Assert-Equal 'rollback-data-sentinel' ([System.IO.File]::ReadAllText((Join-Path $rollbackBlobDirectory 'data.json'))) 'A failed install did not restore existing settings data.'
    Assert-Equal 'rollback-runtime-script-sentinel' ([System.IO.File]::ReadAllText((Join-Path $rollbackRuntimeScripts 'Rebuild.ps1'))) 'A failed install did not restore the runtime script.'
    Assert-Equal 'rollback-runtime-asset-sentinel' ([System.IO.File]::ReadAllText((Join-Path $rollbackRuntimeAssetDirectory 'banner.png'))) 'A failed install did not restore the runtime asset.'
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $rollbackBlobDirectory '.blobui-managed-files.json'))) 'A failed install recorded a release version.'
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $rollbackBlobDirectory 'assets\spacebanner\banner.png'))) 'A failed install left its newly copied banner behind.'
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $rollbackBlobDirectory 'backups'))) 'A failed install created a persistent backup directory.'
    $backupArtifacts = @(Get-ChildItem -LiteralPath $rollbackInstallDirectory -Recurse -File -Filter '*.bak')
    Assert-Equal 0 $backupArtifacts.Count 'A failed install left a backup file behind.'

    $sourceFixture = Join-Path $testRoot 'Source Folder With Spaces'
    $sourceScriptsFixture = Join-Path $sourceFixture 'scripts'
    [System.IO.Directory]::CreateDirectory($sourceScriptsFixture) | Out-Null
    foreach ($sourceFile in @('BlobUI.Core.psm1', 'Get-Data.ps1', 'Compile.ps1', 'Install.ps1', 'Rebuild.ps1', 'Update.ps1', 'Uninstall.ps1')) {
        Copy-Item -LiteralPath (Join-Path $scriptsDirectory $sourceFile) -Destination $sourceScriptsFixture
    }
    Copy-Item -LiteralPath (Join-Path $repoRoot 'Dev') -Destination $sourceFixture -Recurse
    Copy-Item -LiteralPath (Join-Path $repoRoot 'assets') -Destination $sourceFixture -Recurse
    [System.IO.File]::WriteAllText((Join-Path $sourceFixture 'Dev\obsolete-managed.css'), '.blobui-obsolete { color: pink; }')
    Write-BlobUiInstallPointer -SourceDirectory $sourceFixture -InstallDirectory $installDirectory
    $pointerPath = Get-BlobUiInstallPointerPath -SourceDirectory $sourceFixture
    $sourceRebuildArguments = @{ ApiTransport = $apiTransport }

    $releaseUpdateDirectory = Join-Path (Join-Path $testRoot 'Update target\rElEaSeS') 'Anytype Data'
    $releaseUpdateBlob = Join-Path $releaseUpdateDirectory 'blob'
    [System.IO.Directory]::CreateDirectory($releaseUpdateBlob) | Out-Null
    $releaseUpdateDataPath = Join-Path $releaseUpdateBlob 'data.json'
    $releaseUpdateData = Read-BlobUiDataFile -Path $firstInstall.dataPath
    $releaseUpdateData.installDirectory = [System.IO.Path]::GetFullPath($releaseUpdateDirectory)
    Write-BlobUiAtomicJson -Path $releaseUpdateDataPath -InputObject $releaseUpdateData
    $releaseUpdateDataBefore = [System.IO.File]::ReadAllText($releaseUpdateDataPath)
    $apiRequests.Clear()
    Assert-Throws {
        & (Join-Path $sourceScriptsFixture 'Update.ps1') -InstallDirectory $releaseUpdateDirectory -ApiTransport $apiTransport
    } 'An update accepted an explicit target beneath a Releases path.' 'Releases path'
    Assert-Equal 0 $apiRequests.Count 'An update contacted the API before rejecting a Releases target.'
    Assert-Equal $releaseUpdateDataBefore ([System.IO.File]::ReadAllText($releaseUpdateDataPath)) 'A rejected explicit Releases update changed its settings.'
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $releaseUpdateDirectory 'custom.css'))) 'A rejected explicit Releases update created or overwrote custom.css.'
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $releaseUpdateBlob 'staging'))) 'A rejected explicit Releases update created staging files.'

    Copy-Item -LiteralPath $firstInstall.runtimePath -Destination (Join-Path $releaseUpdateBlob 'runtime') -Recurse
    $apiRequests.Clear()
    Assert-Throws {
        & (Join-Path $releaseUpdateBlob 'runtime\scripts\Update.ps1') -ApiTransport $apiTransport
    } 'An installed runtime update accepted a target beneath a Releases path.' 'Releases path'
    Assert-Equal 0 $apiRequests.Count 'An installed runtime update contacted the API before rejecting a Releases target.'
    Assert-Equal $releaseUpdateDataBefore ([System.IO.File]::ReadAllText($releaseUpdateDataPath)) 'A rejected installed runtime Releases update changed its settings.'
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $releaseUpdateDirectory 'custom.css'))) 'A rejected installed runtime Releases update created or overwrote custom.css.'
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $releaseUpdateBlob 'staging'))) 'A rejected installed runtime Releases update created staging files.'

    Write-BlobUiInstallPointer -SourceDirectory $sourceFixture -InstallDirectory $releaseUpdateDirectory
    $apiRequests.Clear()
    Assert-Throws {
        & (Join-Path $sourceScriptsFixture 'Update.ps1') -ApiTransport $apiTransport
    } 'An update accepted a source-pointer target beneath a Releases path.' 'Releases path'
    Assert-Equal 0 $apiRequests.Count 'An update contacted the API before rejecting a source-pointer Releases target.'
    Assert-Equal $releaseUpdateDataBefore ([System.IO.File]::ReadAllText($releaseUpdateDataPath)) 'A rejected source-pointer Releases update changed its settings.'
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $releaseUpdateDirectory 'custom.css'))) 'A rejected source-pointer Releases update created or overwrote custom.css.'
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $releaseUpdateBlob 'staging'))) 'A rejected source-pointer Releases update created staging files.'

    Write-BlobUiInstallPointer -SourceDirectory $sourceFixture -InstallDirectory $installDirectory
    & (Join-Path $sourceScriptsFixture 'Rebuild.ps1') @sourceRebuildArguments
    $pointerData = ConvertFrom-Json -InputObject ([System.IO.File]::ReadAllText($pointerPath))
    Assert-Equal $installDirectory $pointerData.installDirectory 'The source rebuild did not resolve the saved custom install path with spaces.'
    Assert-True (Test-Path -LiteralPath (Join-Path $firstInstall.runtimePath 'Dev\obsolete-managed.css') -PathType Leaf) 'The first package install did not copy its managed Dev file.'

    $apiRequests.Clear()
    & (Join-Path $sourceScriptsFixture 'Update.ps1') -ApiTransport $apiTransport
    $updatedData = Read-BlobUiDataFile -Path $firstInstall.dataPath
    $updatedManifest = ConvertFrom-Json -InputObject ([System.IO.File]::ReadAllText((Join-Path $blobDirectory '.blobui-managed-files.json')))
    Assert-Equal 0 @($updatedData.settingsObjectIds).Count 'Update made marker-mode configuration depend on saved object IDs.'
    Assert-Equal $installDirectory $updatedData.installDirectory 'Update changed the saved install directory.'
    Assert-Equal 1 $updatedData.schemaVersion 'Update changed the data schema version while recording the release.'
    Assert-Equal '1.1.0' $updatedManifest.releaseVersion 'Update did not record the installed release version.'
    Assert-Equal 'unit-test-key' (Read-BlobUiProtectedCredential -Path $firstInstall.credentialPath) 'Update damaged or replaced the saved API key.'
    Assert-Equal 'custom-banner-sentinel' ([System.Text.Encoding]::UTF8.GetString([System.IO.File]::ReadAllBytes($installedBannerPath))) 'Update replaced the installed custom banner.'
    $updateObjectReads = @($apiRequests | Where-Object { $_.Path -match '/objects/[^?]+\?include=properties&ids=full$' })
    Assert-Equal 0 $updateObjectReads.Count 'Marker-mode update fetched settings objects by their saved IDs instead of rediscovering BlobID markers.'
    Assert-Equal 0 @($apiRequests | Where-Object { $_.Method -eq 'Post' -and [string]$_.Body.type -ceq 'setting' }).Count 'Update searched for settings markers in spaces without a BlobID property.'
    Assert-True (Test-Path -LiteralPath (Join-Path $firstInstall.runtimePath 'scripts\Update.ps1') -PathType Leaf) 'Update did not install its bundled updater.'

    $installedUserCss = Join-Path $firstInstall.runtimePath 'Dev\user-local.css'
    [System.IO.File]::WriteAllText($installedUserCss, '.blobui-local { color: lime; }')
    $installedNestedUserCss = Join-Path $firstInstall.runtimePath 'Dev\local-folder\nested.css'
    [System.IO.Directory]::CreateDirectory((Split-Path -Parent $installedNestedUserCss)) | Out-Null
    [System.IO.File]::WriteAllText($installedNestedUserCss, '.blobui-nested-local { color: teal; }')
    [System.IO.File]::WriteAllText((Join-Path $sourceFixture 'Dev\update-collision.css'), '.blobui-collision-release { color: blue; }')
    $installedCollisionCss = Join-Path $firstInstall.runtimePath 'Dev\update-collision.css'
    [System.IO.File]::WriteAllText($installedCollisionCss, '.blobui-collision-local { color: red; }')
    Remove-Item -LiteralPath (Join-Path $sourceFixture 'Dev\obsolete-managed.css') -Force
    $installedUserRuntimeAsset = Join-Path $firstInstall.runtimePath 'assets\user-local.txt'
    [System.IO.File]::WriteAllText($installedUserRuntimeAsset, 'keep-runtime-asset')
    $installedUserBlobAsset = Join-Path $firstInstall.assetsPath 'user-added.txt'
    [System.IO.File]::WriteAllText($installedUserBlobAsset, 'keep-installed-asset')
    & (Join-Path $firstInstall.runtimePath 'scripts\Rebuild.ps1') @rebuildArguments
    $rebuiltManifest = ConvertFrom-Json -InputObject ([System.IO.File]::ReadAllText((Join-Path $blobDirectory '.blobui-managed-files.json')))
    Assert-True (-not (@($rebuiltManifest.managedFiles) -contains 'runtime/Dev/user-local.css')) 'Installed rebuild adopted a local Dev file into the ownership manifest.'
    Assert-True (-not (@($rebuiltManifest.managedFiles) -contains 'runtime/assets/user-local.txt')) 'Installed rebuild adopted a local runtime asset into the ownership manifest.'
    Assert-True (-not (@($rebuiltManifest.managedFiles) -contains 'assets/user-local.txt')) 'Installed rebuild adopted a local runtime asset as an installed asset.'
    [System.IO.File]::WriteAllText($firstInstall.cssPath, 'pre-update-css-sentinel')
    $apiRequests.Clear()
    & (Join-Path $sourceScriptsFixture 'Update.ps1') -ApiTransport $apiTransport
    $updatedCss = [System.IO.File]::ReadAllText($firstInstall.cssPath)
    Assert-True ($updatedCss.Contains('.blobui-local { color: lime; }')) 'Update omitted an unowned CSS file retained under runtime/Dev.'
    Assert-True ($updatedCss.Contains('.blobui-nested-local { color: teal; }')) 'Update omitted an unowned nested CSS file retained under runtime/Dev.'
    Assert-True ($updatedCss.Contains('.blobui-collision-release { color: blue; }')) 'Update omitted a new release CSS file that collided with an unowned local file.'
    Assert-True (-not $updatedCss.Contains('.blobui-collision-local')) 'A same-path local CSS file overrode the release CSS.'
    Assert-True (-not $updatedCss.Contains('.blobui-obsolete')) 'Update compiled stale CSS that was listed in the ownership manifest.'
    Assert-Equal '.blobui-collision-release { color: blue; }' ([System.IO.File]::ReadAllText($installedCollisionCss)) 'Update did not replace a same-path unowned local file with the release file.'
    $updatedManifest = ConvertFrom-Json -InputObject ([System.IO.File]::ReadAllText((Join-Path $blobDirectory '.blobui-managed-files.json')))
    Assert-True (-not (@($updatedManifest.managedFiles) -contains 'runtime/Dev/user-local.css')) 'Update adopted a top-level local CSS file into the ownership manifest.'
    Assert-True (-not (@($updatedManifest.managedFiles) -contains 'runtime/Dev/local-folder/nested.css')) 'Update adopted a nested local CSS file into the ownership manifest.'
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $firstInstall.runtimePath 'Dev\obsolete-managed.css'))) 'Update left a stale file listed in the ownership manifest.'
    Assert-Equal '.blobui-local { color: lime; }' ([System.IO.File]::ReadAllText($installedUserCss)) 'Update removed a user file that was not listed in the ownership manifest.'
    Assert-Equal 'keep-runtime-asset' ([System.IO.File]::ReadAllText($installedUserRuntimeAsset)) 'Update removed a user file under runtime/assets that was not listed in the ownership manifest.'
    Assert-Equal 'keep-installed-asset' ([System.IO.File]::ReadAllText($installedUserBlobAsset)) 'Update removed an untracked file from the installed assets directory.'
    Assert-Equal 'keep-me' ([System.IO.File]::ReadAllText((Join-Path $firstInstall.assetsPath 'user-file.txt'))) 'Update removed an untracked asset file.'
    Assert-Equal 'custom-banner-sentinel' ([System.Text.Encoding]::UTF8.GetString([System.IO.File]::ReadAllBytes($installedBannerPath))) 'A later update replaced the installed custom banner.'

    $transitionRuntime = Join-Path $testRoot 'Managed Path Transition Source'
    Copy-Item -LiteralPath $sourceFixture -Destination $transitionRuntime -Recurse
    $transitionInstallDirectory = Join-Path $testRoot 'Managed Path Transition Anytype'
    [System.IO.Directory]::CreateDirectory($transitionInstallDirectory) | Out-Null
    $transitionSourcePath = Join-Path $transitionRuntime 'Dev\transition'
    [System.IO.Directory]::CreateDirectory($transitionSourcePath) | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $transitionSourcePath 'child.css'), '.transition-child { color: orange; }')
    $transitionCandidatePath = Join-Path $testRoot 'transition-data.json'
    $transitionCandidate = ConvertFrom-Json -InputObject ([System.IO.File]::ReadAllText($candidateDataPath))
    $transitionCandidate.installDirectory = $transitionInstallDirectory
    Write-BlobUiAtomicJson -Path $transitionCandidatePath -InputObject $transitionCandidate
    $transitionArguments = @{
        InstallDirectory = $transitionInstallDirectory
        RuntimeSourceDirectory = $transitionRuntime
        AssetsSourceDirectory = Join-Path $transitionRuntime 'assets'
        CompiledCssPath = $compiledCssPath
        DataCandidatePath = $transitionCandidatePath
    }
    $null = Invoke-BlobUiInstall @transitionArguments
    $transitionBlobRuntime = Join-Path $transitionInstallDirectory 'blob\runtime\Dev\transition'
    $transitionCredentialConflict = Join-Path $transitionInstallDirectory 'blob\credentials\anytype-api-key.dpapi'

    Remove-Item -LiteralPath $transitionSourcePath -Recurse -Force
    [System.IO.File]::WriteAllText($transitionSourcePath, '.transition-file { color: purple; }')
    [System.IO.Directory]::CreateDirectory($transitionCredentialConflict) | Out-Null
    $transitionArguments.CredentialProtectedText = $protectedCredential
    Assert-Throws { Invoke-BlobUiInstall @transitionArguments } 'Directory-to-file rollback did not fail at the injected credential write.' ''
    Assert-True (Test-Path -LiteralPath $transitionBlobRuntime -PathType Container) 'Directory-to-file rollback did not restore the prior directory.'
    Assert-Equal '.transition-child { color: orange; }' ([System.IO.File]::ReadAllText((Join-Path $transitionBlobRuntime 'child.css'))) 'Directory-to-file rollback did not restore its managed child.'
    Remove-Item -LiteralPath (Join-Path $transitionInstallDirectory 'blob\credentials') -Recurse -Force
    $transitionArguments.Remove('CredentialProtectedText')
    $null = Invoke-BlobUiInstall @transitionArguments
    Assert-True (Test-Path -LiteralPath $transitionBlobRuntime -PathType Leaf) 'Directory-to-file update did not replace the obsolete managed tree.'
    Assert-Equal '.transition-file { color: purple; }' ([System.IO.File]::ReadAllText($transitionBlobRuntime)) 'Directory-to-file update copied the wrong managed content.'

    Remove-Item -LiteralPath $transitionSourcePath -Force
    [System.IO.Directory]::CreateDirectory($transitionSourcePath) | Out-Null
    $transitionNestedSourceDirectory = Join-Path $transitionSourcePath 'nested\folder'
    [System.IO.Directory]::CreateDirectory($transitionNestedSourceDirectory) | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $transitionNestedSourceDirectory 'child.css'), '.transition-tree { color: teal; }')
    [System.IO.Directory]::CreateDirectory($transitionCredentialConflict) | Out-Null
    $transitionArguments.CredentialProtectedText = $protectedCredential
    Assert-Throws { Invoke-BlobUiInstall @transitionArguments } 'File-to-directory rollback did not fail at the injected credential write.' ''
    Assert-True (Test-Path -LiteralPath $transitionBlobRuntime -PathType Leaf) 'File-to-directory rollback did not restore the prior managed file.'
    Assert-Equal '.transition-file { color: purple; }' ([System.IO.File]::ReadAllText($transitionBlobRuntime)) 'File-to-directory rollback restored the wrong prior bytes.'
    Remove-Item -LiteralPath (Join-Path $transitionInstallDirectory 'blob\credentials') -Recurse -Force
    $transitionArguments.Remove('CredentialProtectedText')
    $null = Invoke-BlobUiInstall @transitionArguments
    Assert-True (Test-Path -LiteralPath $transitionBlobRuntime -PathType Container) 'File-to-directory update did not replace the obsolete managed file.'
    $transitionNestedInstalledFile = Join-Path $transitionBlobRuntime 'nested\folder\child.css'
    Assert-Equal '.transition-tree { color: teal; }' ([System.IO.File]::ReadAllText($transitionNestedInstalledFile)) 'File-to-directory update copied the wrong managed content.'

    [System.IO.File]::WriteAllText((Join-Path $transitionBlobRuntime 'nested\folder\user-added.txt'), 'keep-transition-user-file')
    Remove-Item -LiteralPath $transitionSourcePath -Recurse -Force
    [System.IO.File]::WriteAllText($transitionSourcePath, '.transition-replacement { color: navy; }')
    Assert-Throws { Invoke-BlobUiInstall @transitionArguments } 'A non-empty unowned directory was replaced by a managed file.' 'unowned or non-empty directory'
    Assert-True (Test-Path -LiteralPath $transitionBlobRuntime -PathType Container) 'The collision failure removed the unowned directory.'
    Assert-Equal 'keep-transition-user-file' ([System.IO.File]::ReadAllText((Join-Path $transitionBlobRuntime 'nested\folder\user-added.txt'))) 'The collision failure removed an unowned user file.'
    Assert-Equal '.transition-tree { color: teal; }' ([System.IO.File]::ReadAllText($transitionNestedInstalledFile)) 'The collision failure did not roll back the stale managed file.'

    $migrationFixture = [pscustomobject]@{ schemaVersion = 1; localValue = 'keep' }
    $migrationResult = Invoke-BlobUiLocalDataMigrations -Data $migrationFixture -FromReleaseVersion '0.9.0' -ToReleaseVersion (Get-BlobUiReleaseVersion)
    Assert-Equal 'keep' $migrationResult.localValue 'The local migration hook changed data without a migration.'
    Assert-Equal 1 $migrationResult.schemaVersion 'The local migration hook changed schemaVersion.'

    $stalePointerText = [System.IO.File]::ReadAllText($pointerPath)
    Write-BlobUiInstallPointer -SourceDirectory $sourceFixture -InstallDirectory (Join-Path $testRoot 'missing install')
    $failedInstallPointerText = [System.IO.File]::ReadAllText($pointerPath)
    Assert-Throws { & (Join-Path $sourceScriptsFixture 'Rebuild.ps1') @sourceRebuildArguments } 'A stale source install pointer was accepted.' 'no longer exists'
    Assert-Equal $failedInstallPointerText ([System.IO.File]::ReadAllText($pointerPath)) 'A failed source rebuild changed the saved install pointer.'
    Write-BlobUiAtomicText -Path $pointerPath -Content $stalePointerText

    Remove-Item -LiteralPath $pointerPath -Force
    & (Join-Path $sourceScriptsFixture 'Rebuild.ps1') @sourceRebuildArguments -InstallDirectory $installDirectory
    $oldAppData = $env:APPDATA
    $legacyAppData = Join-Path $testRoot 'Legacy APPDATA'
    $legacyInstallDirectory = Join-Path $legacyAppData 'anytype'
    [System.IO.Directory]::CreateDirectory($legacyInstallDirectory) | Out-Null
    $legacyData = Read-BlobUiDataFile -Path $firstInstall.dataPath
    $legacyData.installDirectory = $legacyInstallDirectory
    $legacyBlobDirectory = Join-Path $legacyInstallDirectory 'blob'
    [System.IO.Directory]::CreateDirectory((Join-Path $legacyBlobDirectory 'credentials')) | Out-Null
    Write-BlobUiAtomicJson -Path (Join-Path $legacyBlobDirectory 'data.json') -InputObject $legacyData
    Copy-Item -LiteralPath $firstInstall.credentialPath -Destination (Join-Path $legacyBlobDirectory 'credentials\anytype-api-key.dpapi')
    $env:APPDATA = $legacyAppData
    try {
        & (Join-Path $sourceScriptsFixture 'Rebuild.ps1') @sourceRebuildArguments
    }
    finally {
        $env:APPDATA = $oldAppData
    }
    Assert-True (Test-Path -LiteralPath (Join-Path $legacyInstallDirectory 'custom.css') -PathType Leaf) 'The source rebuild did not discover a valid legacy default installation without a pointer.'

    $defaultsInstallDirectory = Join-Path $testRoot 'Legacy Update Missing Fields'
    $defaultsBlobDirectory = Join-Path $defaultsInstallDirectory 'blob'
    $defaultsCredentialDirectory = Join-Path $defaultsBlobDirectory 'credentials'
    $defaultsLegacyDevDirectory = Join-Path $defaultsBlobDirectory 'runtime\Dev'
    [System.IO.Directory]::CreateDirectory($defaultsCredentialDirectory) | Out-Null
    [System.IO.Directory]::CreateDirectory($defaultsLegacyDevDirectory) | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $defaultsLegacyDevDirectory 'legacy-untracked.css'), '.legacy { color: blue; }')
    $defaultsSavedData = [ordered]@{
        schemaVersion = 1
        apiVersion = 'v2'
        selectedSpace = [ordered]@{ id = $testSpaceId; name = 'BlobUI Settings' }
        settingsObjectIds = @($emptyObjectId)
        settings = [ordered]@{
            fontFamily = 'Saved Local Font'
            codeFontFamily = 'Saved Local Code Font'
            accentColor = '#123456'
            bannerPath = 'spacebanner\banner.png'
        }
        installDirectory = [System.IO.Path]::GetFullPath($defaultsInstallDirectory)
    }
    Write-BlobUiAtomicJson -Path (Join-Path $defaultsBlobDirectory 'data.json') -InputObject $defaultsSavedData
    Copy-Item -LiteralPath $firstInstall.credentialPath -Destination (Join-Path $defaultsCredentialDirectory 'anytype-api-key.dpapi')
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $defaultsBlobDirectory '.blobui-managed-files.json'))) 'A legacy update fixture unexpectedly had an ownership manifest.'
    $defaultsRuntimeDirectory = Join-Path $defaultsBlobDirectory 'runtime'
    [System.IO.Directory]::CreateDirectory($defaultsRuntimeDirectory) | Out-Null
    Copy-Item -Path (Join-Path $firstInstall.runtimePath '*') -Destination $defaultsRuntimeDirectory -Recurse -Force
    $legacyRuntimeUserAsset = Join-Path $defaultsRuntimeDirectory 'assets\legacy-user.txt'
    [System.IO.File]::WriteAllText($legacyRuntimeUserAsset, 'keep-legacy-runtime-asset')
    & (Join-Path $defaultsRuntimeDirectory 'scripts\Rebuild.ps1') -ApiTransport $apiTransport
    $defaultsRebuiltManifest = ConvertFrom-Json -InputObject ([System.IO.File]::ReadAllText((Join-Path $defaultsBlobDirectory '.blobui-managed-files.json')))
    Assert-Equal 0 @($defaultsRebuiltManifest.managedFiles).Count 'Installed rebuild claimed runtime files from a legacy install without an ownership manifest.'
    Assert-Equal 'keep-legacy-runtime-asset' ([System.IO.File]::ReadAllText($legacyRuntimeUserAsset)) 'Installed rebuild changed a runtime asset in a legacy install without an ownership manifest.'
    $apiRequests.Clear()
    & (Join-Path $sourceScriptsFixture 'Update.ps1') -InstallDirectory $defaultsInstallDirectory -ApiTransport $apiTransport
    $defaultsUpdatedData = Read-BlobUiDataFile -Path (Join-Path $defaultsBlobDirectory 'data.json')
    $defaultsUpdatedCss = [System.IO.File]::ReadAllText((Join-Path $defaultsInstallDirectory 'custom.css'))
    Assert-True ($defaultsUpdatedCss.Contains('.legacy { color: blue; }')) 'Update omitted legacy local CSS when no ownership manifest existed.'
    Assert-Equal 'Libron' $defaultsUpdatedData.settings.fontFamily 'Update retained a stale local Font value instead of the release default for a missing Anytype field.'
    Assert-Equal 'Consolas' $defaultsUpdatedData.settings.codeFontFamily 'Update retained a stale local Code Font value instead of the release default for a missing Anytype field.'
    Assert-Equal '#DE50DE' $defaultsUpdatedData.settings.accentColor 'Update retained a stale local Accent Color instead of the release default for a missing Anytype field.'
    Assert-Equal "$emptyObjectId" $defaultsUpdatedData.settingsObjectIds 'Update changed the legacy selected settings object.'
    Assert-Equal '.legacy { color: blue; }' ([System.IO.File]::ReadAllText((Join-Path $defaultsLegacyDevDirectory 'legacy-untracked.css'))) 'Update deleted an untracked file from a legacy install without a manifest.'
    Assert-Equal 'keep-legacy-runtime-asset' ([System.IO.File]::ReadAllText($legacyRuntimeUserAsset)) 'Update deleted a runtime asset in a legacy install without a manifest.'
    Assert-Equal '1.1.0' (ConvertFrom-Json -InputObject ([System.IO.File]::ReadAllText((Join-Path $defaultsBlobDirectory '.blobui-managed-files.json')))).releaseVersion 'A legacy update did not record its release version.'
    Assert-Equal 1 @($apiRequests | Where-Object { $_.Path -match ("/objects/" + [System.Text.RegularExpressions.Regex]::Escape($emptyObjectId) + "\?include=properties&ids=full$") }).Count 'Update did not reuse the selected object ID from the legacy data file.'
    Assert-Equal 0 @($apiRequests | Where-Object { $_.Method -eq 'Post' -and [string]$_.Body.type -ceq 'setting' }).Count 'A legacy update rediscovered settings objects.'

    $noInstallFixture = Join-Path $testRoot 'No Install Source'
    Copy-Item -LiteralPath $sourceFixture -Destination $noInstallFixture -Recurse
    $noInstallPointer = Get-BlobUiInstallPointerPath -SourceDirectory $noInstallFixture
    if (Test-Path -LiteralPath $noInstallPointer) { Remove-Item -LiteralPath $noInstallPointer -Force }
    $env:APPDATA = Join-Path $testRoot 'Empty APPDATA'
    try {
        Assert-Throws { & (Join-Path $noInstallFixture 'scripts\Rebuild.ps1') @sourceRebuildArguments } 'A source rebuild without an installation did not fail clearly.' 'No BlobUI installation was found'
    }
    finally {
        $env:APPDATA = $oldAppData
    }

    $sourceUninstallScript = Join-Path $sourceScriptsFixture 'Uninstall.ps1'
    Assert-True (Test-Path -LiteralPath $sourceUninstallScript -PathType Leaf) 'The source fixture did not include Uninstall.ps1.'
    $emptyAppData = Join-Path $testRoot 'Empty APPDATA'
    $env:APPDATA = $emptyAppData
    try {
        Assert-Throws { & $sourceUninstallScript } 'An uninstall without a pointer or default install was accepted.' 'No BlobUI installation was found'
    }
    finally {
        $env:APPDATA = $oldAppData
    }

    $staleUninstallDirectory = Join-Path $testRoot 'Missing uninstall target'
    Write-BlobUiInstallPointer -SourceDirectory $sourceFixture -InstallDirectory $staleUninstallDirectory
    $staleUninstallPointerText = [System.IO.File]::ReadAllText($pointerPath)
    $env:APPDATA = $emptyAppData
    try {
        Assert-Throws { & $sourceUninstallScript } 'An uninstall with a stale source pointer was accepted.' 'no longer exists'
    }
    finally {
        $env:APPDATA = $oldAppData
    }
    Assert-Equal $staleUninstallPointerText ([System.IO.File]::ReadAllText($pointerPath)) 'A stale uninstall pointer was removed or changed.'

    $pointedUninstallDirectory = Join-Path $testRoot 'Pointer Anytype Data With Spaces'
    $explicitUninstallDirectory = Join-Path $testRoot 'Explicit Anytype Data With Spaces'
    foreach ($uninstallDirectory in @($pointedUninstallDirectory, $explicitUninstallDirectory)) {
        [System.IO.Directory]::CreateDirectory($uninstallDirectory) | Out-Null
        [System.IO.Directory]::CreateDirectory((Join-Path $uninstallDirectory 'blob\backups')) | Out-Null
        [System.IO.Directory]::CreateDirectory((Join-Path $uninstallDirectory 'blob\runtime\scripts')) | Out-Null
        [System.IO.Directory]::CreateDirectory((Join-Path $uninstallDirectory 'blob\assets')) | Out-Null
        [System.IO.Directory]::CreateDirectory((Join-Path $uninstallDirectory 'blob\staging\run')) | Out-Null
        [System.IO.File]::WriteAllText((Join-Path $uninstallDirectory 'custom.css'), 'uninstall-css')
        [System.IO.File]::WriteAllText((Join-Path $uninstallDirectory 'blob\backups\old-custom.css.bak'), 'old-backup')
        [System.IO.File]::WriteAllText((Join-Path $uninstallDirectory 'blob\runtime\scripts\Rebuild.ps1'), 'runtime-script')
        [System.IO.File]::WriteAllText((Join-Path $uninstallDirectory 'blob\assets\user-asset.txt'), 'asset')
        [System.IO.File]::WriteAllText((Join-Path $uninstallDirectory 'blob\staging\run\candidate.css'), 'staged-css')
        [System.IO.File]::WriteAllText((Join-Path $uninstallDirectory 'unrelated-anytype-file.txt'), 'keep-anytype-file')
    }
    $releaseCssPath = Join-Path $pointedUninstallDirectory 'Releases\custom.css'
    [System.IO.Directory]::CreateDirectory((Split-Path -Parent $releaseCssPath)) | Out-Null
    [System.IO.File]::WriteAllText($releaseCssPath, 'preserve-release-css')
    Write-BlobUiInstallPointer -SourceDirectory $sourceFixture -InstallDirectory $pointedUninstallDirectory
    & $sourceUninstallScript -InstallDirectory $explicitUninstallDirectory
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $explicitUninstallDirectory 'custom.css'))) 'An explicit uninstall left the selected custom.css file.'
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $explicitUninstallDirectory 'blob'))) 'An explicit uninstall left files or old backups in the selected blob folder.'
    Assert-Equal 'keep-anytype-file' ([System.IO.File]::ReadAllText((Join-Path $explicitUninstallDirectory 'unrelated-anytype-file.txt'))) 'An explicit uninstall removed an unrelated Anytype file.'
    Assert-True (Test-Path -LiteralPath $pointerPath -PathType Leaf) 'An explicit uninstall removed a source pointer for a different installation.'
    $remainingPointer = ConvertFrom-Json -InputObject ([System.IO.File]::ReadAllText($pointerPath))
    Assert-Equal $pointedUninstallDirectory $remainingPointer.installDirectory 'An explicit uninstall changed a source pointer for a different installation.'

    & $sourceUninstallScript
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $pointedUninstallDirectory 'custom.css'))) 'A source-pointer uninstall left the selected custom.css file.'
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $pointedUninstallDirectory 'blob'))) 'A source-pointer uninstall left the selected blob folder.'
    Assert-Equal 'keep-anytype-file' ([System.IO.File]::ReadAllText((Join-Path $pointedUninstallDirectory 'unrelated-anytype-file.txt'))) 'A source-pointer uninstall removed an unrelated Anytype file.'
    Assert-Equal 'preserve-release-css' ([System.IO.File]::ReadAllText($releaseCssPath)) 'A source-pointer uninstall removed a custom.css file in Releases.'
    Assert-True (-not (Test-Path -LiteralPath $pointerPath)) 'A successful source-pointer uninstall left its matching source pointer.'
    & $sourceUninstallScript -InstallDirectory $pointedUninstallDirectory
    Assert-True (Test-Path -LiteralPath (Join-Path $pointedUninstallDirectory 'unrelated-anytype-file.txt') -PathType Leaf) 'A repeated uninstall removed unrelated Anytype data.'

    $legacyUninstallCss = Join-Path $legacyInstallDirectory 'Releases\custom.css'
    [System.IO.Directory]::CreateDirectory((Split-Path -Parent $legacyUninstallCss)) | Out-Null
    [System.IO.File]::WriteAllText($legacyUninstallCss, 'preserve-legacy-release-css')
    [System.IO.File]::WriteAllText((Join-Path $legacyInstallDirectory 'unrelated-anytype-file.txt'), 'keep-legacy-anytype-file')
    $env:APPDATA = $legacyAppData
    try {
        & $sourceUninstallScript
    }
    finally {
        $env:APPDATA = $oldAppData
    }
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $legacyInstallDirectory 'custom.css'))) 'The validated default fallback left custom.css installed.'
    Assert-True (Test-Path -LiteralPath (Join-Path $legacyInstallDirectory 'blob\assets\spacebanner\banner.png') -PathType Leaf) 'The validated default fallback removed the bundled user banner asset.'
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $legacyInstallDirectory 'blob\data.json'))) 'The validated default fallback left BlobUI settings data beside the preserved banner.'
    Assert-Equal 'keep-legacy-anytype-file' ([System.IO.File]::ReadAllText((Join-Path $legacyInstallDirectory 'unrelated-anytype-file.txt'))) 'The validated default fallback removed another Anytype file.'
    Assert-Equal 'preserve-legacy-release-css' ([System.IO.File]::ReadAllText($legacyUninstallCss)) 'The validated default fallback removed a custom.css file in Releases.'

    $releaseRoot = Join-Path $testRoot 'Releases'
    [System.IO.Directory]::CreateDirectory((Join-Path $releaseRoot 'blob')) | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $releaseRoot 'custom.css'), 'release-root-css')
    Assert-Throws { & $sourceUninstallScript -InstallDirectory $releaseRoot } 'An uninstall accepted a Releases folder as the Anytype directory.' 'Releases path'
    Assert-Equal 'release-root-css' ([System.IO.File]::ReadAllText((Join-Path $releaseRoot 'custom.css'))) 'The Releases folder guard changed custom.css.'
    Assert-True (Test-Path -LiteralPath (Join-Path $releaseRoot 'blob') -PathType Container) 'The Releases folder guard changed the blob directory.'

    $nestedReleaseRoot = Join-Path (Join-Path $testRoot 'Nested uninstall target\rElEaSeS') 'Anytype Data'
    $nestedReleaseSentinel = Join-Path $nestedReleaseRoot 'blob\keep.txt'
    [System.IO.Directory]::CreateDirectory((Split-Path -Parent $nestedReleaseSentinel)) | Out-Null
    [System.IO.File]::WriteAllText($nestedReleaseSentinel, 'preserve-nested-release-data')
    Assert-Throws { & $sourceUninstallScript -InstallDirectory $nestedReleaseRoot } 'An uninstall accepted a nested Releases target.' 'Releases path'
    Assert-Equal 'preserve-nested-release-data' ([System.IO.File]::ReadAllText($nestedReleaseSentinel)) 'A rejected nested Releases uninstall changed blob data.'
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $nestedReleaseRoot 'custom.css'))) 'A rejected nested Releases uninstall created or removed custom.css.'

    $outsideReparseTarget = Join-Path $testRoot 'Outside reparse target'
    [System.IO.Directory]::CreateDirectory((Join-Path $outsideReparseTarget 'blob')) | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $outsideReparseTarget 'custom.css'), 'outside-css')
    [System.IO.File]::WriteAllText((Join-Path $outsideReparseTarget 'blob\outside-sentinel.txt'), 'preserve-outside')
    $selectedRootJunction = Join-Path $testRoot 'Selected Anytype Junction'
    $junctionSupported = $false
    try {
        New-Item -ItemType Junction -Path $selectedRootJunction -Target $outsideReparseTarget -ErrorAction Stop | Out-Null
        $junctionSupported = $true
    }
    catch {
        Write-Warning "Junction safety checks were skipped because this host could not create a junction: $($_.Exception.Message)"
    }
    if ($junctionSupported) {
        Assert-Throws { & $sourceUninstallScript -InstallDirectory $selectedRootJunction } 'An uninstall accepted a reparse-point Anytype root.' 'reparse point'
        Assert-Equal 'outside-css' ([System.IO.File]::ReadAllText((Join-Path $outsideReparseTarget 'custom.css'))) 'A rejected root junction changed the external CSS.'
        Assert-Equal 'preserve-outside' ([System.IO.File]::ReadAllText((Join-Path $outsideReparseTarget 'blob\outside-sentinel.txt'))) 'A rejected root junction changed external blob contents.'
        Remove-Item -LiteralPath $selectedRootJunction -Force

        $nestedJunctionRoot = Join-Path $testRoot 'Anytype Data With Nested Junction'
        $nestedBlob = Join-Path $nestedJunctionRoot 'blob'
        [System.IO.Directory]::CreateDirectory($nestedBlob) | Out-Null
        [System.IO.File]::WriteAllText((Join-Path $nestedJunctionRoot 'custom.css'), 'nested-junction-css')
        $nestedJunctionPath = Join-Path $nestedBlob 'external'
        New-Item -ItemType Junction -Path $nestedJunctionPath -Target $outsideReparseTarget -ErrorAction Stop | Out-Null
        & $sourceUninstallScript -InstallDirectory $nestedJunctionRoot
        Assert-True (-not (Test-Path -LiteralPath $nestedJunctionRoot)) 'Uninstall left a blob tree containing a junction.'
        Assert-Equal 'preserve-outside' ([System.IO.File]::ReadAllText((Join-Path $outsideReparseTarget 'blob\outside-sentinel.txt'))) 'Uninstall traversed a junction and removed external blob contents.'
    }

    $runtimeUninstallScript = Join-Path $firstInstall.runtimePath 'scripts\Uninstall.ps1'
    & $runtimeUninstallScript
    Assert-True (-not (Test-Path -LiteralPath $firstInstall.cssPath)) 'The installed runtime uninstaller left custom.css.'
    Assert-Equal 'custom-banner-sentinel' ([System.IO.File]::ReadAllText($installedBannerPath)) 'The installed runtime uninstaller removed a user-owned banner.'
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $blobDirectory 'data.json'))) 'The installed runtime uninstaller left BlobUI settings data beside user banners.'
    Assert-Equal 'keep-anytype-file' ([System.IO.File]::ReadAllText((Join-Path $installDirectory 'unrelated-anytype-file.txt'))) 'The installed runtime uninstaller removed an unrelated Anytype file.'

    Write-Host 'BlobUI isolated workflow checks passed.'
}
finally {
    Pop-Location
    if (Test-Path -LiteralPath $testRoot) {
        Remove-Item -LiteralPath $testRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
