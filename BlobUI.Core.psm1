Set-StrictMode -Version Latest

$script:BlobUiApiVersion = '2025-11-08'
$script:BlobUiApiBaseUri = 'http://127.0.0.1:31009'
$script:BlobUiDefaultSettings = @{
    fontFamily = 'Libron'
    codeFontFamily = 'Consolas'
    accentColor = '#DE50DE'
    bannerPath = 'spacebanner\banner.png'
}

function Get-BlobUiHttpStatusCode {
    param([Parameter(Mandatory = $true)]$ErrorRecord)

    $exception = $ErrorRecord.Exception
    $responseProperty = $exception.PSObject.Properties['Response']
    $response = if ($null -ne $responseProperty) { $responseProperty.Value } else { $null }
    if ($null -ne $response) {
        try { return [int]$response.StatusCode } catch {}
    }
    try {
        $statusCodeProperty = $exception.PSObject.Properties['StatusCode']
        if ($null -ne $statusCodeProperty -and $null -ne $statusCodeProperty.Value) {
            return [int]$statusCodeProperty.Value
        }
    }
    catch {}
    try {
        if ($null -ne $exception.Data['StatusCode']) {
            return [int]$exception.Data['StatusCode']
        }
    }
    catch {}
    return 0
}

function Invoke-BlobUiApiRequest {
    param(
        [Parameter(Mandatory = $true)][string]$ApiKey,
        [Parameter(Mandatory = $true)][string]$Path,
        [string]$ApiBaseUri = $script:BlobUiApiBaseUri,
        [scriptblock]$ApiTransport
    )

    if ([string]::IsNullOrWhiteSpace($ApiKey)) {
        throw 'An Anytype API key is required.'
    }

    $uri = $ApiBaseUri.TrimEnd('/') + $Path
    $headers = @{
        Authorization = 'Bearer ' + $ApiKey
        'Anytype-Version' = $script:BlobUiApiVersion
        Accept = 'application/json'
    }

    try {
        if ($null -ne $ApiTransport) {
            return & $ApiTransport $ApiKey $Path
        }
        return Invoke-RestMethod -Method Get -Uri $uri -Headers $headers -TimeoutSec 15 -ErrorAction Stop
    }
    catch {
        $statusCode = Get-BlobUiHttpStatusCode -ErrorRecord $_
        if ($statusCode -eq 401) {
            throw 'Anytype rejected the API key. Check the key and confirm it has access to the selected space.'
        }
        if ($statusCode -eq 404) {
            throw 'Anytype could not find the selected space or settings object.'
        }
        if ($statusCode -eq 400) {
            throw 'Anytype rejected the settings query. Confirm the local API supports version 2025-11-08 and the Setting type key is available.'
        }
        throw 'Could not reach Anytype local API at 127.0.0.1:31009. Open Anytype and confirm its local API is enabled.'
    }
}

function Get-BlobUiResponseItems {
    param([Parameter(Mandatory = $true)]$Response)

    $dataProperty = $Response.PSObject.Properties['data']
    if ($null -ne $dataProperty) {
        return @($dataProperty.Value)
    }
    if ($Response -is [System.Array]) {
        return @($Response)
    }
    return @($Response)
}

function Get-BlobUiPagedApiItems {
    param(
        [Parameter(Mandatory = $true)][string]$ApiKey,
        [Parameter(Mandatory = $true)][string]$Path,
        [string]$ApiBaseUri = $script:BlobUiApiBaseUri,
        [scriptblock]$ApiTransport
    )

    $limit = 1000
    $offset = 0
    $allItems = [System.Collections.Generic.List[object]]::new()

    while ($true) {
        $separator = '?'
        if ($Path.Contains('?')) {
            $separator = '&'
        }
        $pagePath = $Path + $separator + 'limit=' + $limit + '&offset=' + $offset
        $response = Invoke-BlobUiApiRequest -ApiKey $ApiKey -Path $pagePath -ApiBaseUri $ApiBaseUri -ApiTransport $ApiTransport
        $items = @(Get-BlobUiResponseItems -Response $response)
        foreach ($item in $items) {
            if ($null -ne $item) {
                $allItems.Add($item)
            }
        }

        $paginationProperty = $response.PSObject.Properties['pagination']
        $hasMore = $false
        if ($null -ne $paginationProperty -and $null -ne $paginationProperty.Value) {
            $hasMoreProperty = $paginationProperty.Value.PSObject.Properties['has_more']
            if ($null -ne $hasMoreProperty) {
                $hasMore = [bool]$hasMoreProperty.Value
            }
        }

        if ($hasMore) {
            if ($items.Count -eq 0) {
                throw 'Anytype returned an empty page while more results were reported.'
            }
            $offset += $items.Count
            continue
        }

        if ($items.Count -lt $limit) {
            break
        }
        $offset += $items.Count
    }

    return $allItems.ToArray()
}

function Get-BlobUiObjectTypeKey {
    param([Parameter(Mandatory = $true)]$Object)

    $typeProperty = $Object.PSObject.Properties['type']
    if ($null -ne $typeProperty -and $null -ne $typeProperty.Value) {
        $keyProperty = $typeProperty.Value.PSObject.Properties['key']
        if ($null -ne $keyProperty) {
            return [string]$keyProperty.Value
        }
    }

    $typeKeyProperty = $Object.PSObject.Properties['type_key']
    if ($null -ne $typeKeyProperty) {
        return [string]$typeKeyProperty.Value
    }
    return ''
}

function Get-BlobUiObjectPropertyText {
    param(
        [Parameter(Mandatory = $true)]$Object,
        [Parameter(Mandatory = $true)][string]$Key,
        [Parameter(Mandatory = $true)][string]$DisplayName
    )

    $propertiesProperty = $Object.PSObject.Properties['properties']
    if ($null -eq $propertiesProperty -or $null -eq $propertiesProperty.Value) {
        return $null
    }

    $properties = $propertiesProperty.Value
    if ($properties -is [System.Collections.IDictionary]) {
        $raw = $properties[$Key]
        if ($null -eq $raw) {
            $raw = $properties[$DisplayName]
        }
        if ($null -eq $raw) {
            return $null
        }
        if ($raw -is [string]) {
            return [string]$raw
        }
        $textProperty = $raw.PSObject.Properties['text']
        if ($null -ne $textProperty) {
            return [string]$textProperty.Value
        }
        return $null
    }

    $matches = @($properties | Where-Object {
        ($_.PSObject.Properties['key'] -and [string]$_.key -ceq $Key) -or
        ($_.PSObject.Properties['name'] -and [string]$_.name -ceq $DisplayName)
    })
    if ($matches.Count -eq 0) {
        return $null
    }
    if ($matches.Count -gt 1) {
        $matches = @($matches | Where-Object {
            $_.PSObject.Properties['key'] -and [string]$_.key -ceq $Key
        })
    }
    if ($matches.Count -eq 0) {
        return $null
    }

    $property = $matches[0]
    if ($property.PSObject.Properties['format'] -and [string]$property.format -ne 'text') {
        throw "The '$DisplayName' setting must use the text property format."
    }
    if ($property.PSObject.Properties['text']) {
        return [string]$property.text
    }
    return $null
}

function ConvertTo-BlobUiCssPlaceholderValue {
    param(
        [Parameter(Mandatory = $true)][ValidateSet('font', 'color', 'url', 'number', 'boolean')][string]$Type,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Value,
        [string]$Name = 'CSS setting'
    )

    $trimmed = $Value.Trim()
    switch ($Type) {
        'font' {
            if ($trimmed.Length -lt 1 -or $trimmed.Length -gt 80 -or $trimmed -notmatch '^[\p{L}\p{N}][\p{L}\p{N} _-]{0,79}$') {
                throw "'$Name' must be a font family name with letters, numbers, spaces, underscores, or hyphens only."
            }
            return $trimmed
        }
        'color' {
            if ($trimmed -notmatch '^#(?:[0-9A-Fa-f]{3}|[0-9A-Fa-f]{6})$') {
                throw "'$Name' must be a three or six digit hexadecimal CSS color."
            }
            return $trimmed.ToUpperInvariant()
        }
        'url' {
            $uri = $null
            if (-not [System.Uri]::TryCreate($trimmed, [System.UriKind]::Absolute, [ref]$uri) -or
                $uri.Scheme -ne [System.Uri]::UriSchemeFile -or
                $trimmed.Contains('"') -or $trimmed.Contains("'") -or
                $trimmed.Contains([string][char]13) -or $trimmed.Contains([string][char]10)) {
                throw "'$Name' must be an absolute local file URI."
            }
            return $uri.AbsoluteUri
        }
        'number' {
            $number = [decimal]0
            $parsed = [decimal]::TryParse(
                $trimmed,
                [System.Globalization.NumberStyles]::Float,
                [System.Globalization.CultureInfo]::InvariantCulture,
                [ref]$number
            )
            if (-not $parsed -or $number -lt -10000 -or $number -gt 10000) {
                throw "'$Name' must be a finite CSS number between -10000 and 10000."
            }
            return $number.ToString('0.############################', [System.Globalization.CultureInfo]::InvariantCulture)
        }
        'boolean' {
            if ($trimmed -ieq 'true') {
                return '1'
            }
            if ($trimmed -ieq 'false') {
                return '0'
            }
            throw "'$Name' must be true or false."
        }
    }
}

function ConvertTo-BlobUiBannerRelativePath {
    param([AllowNull()][string]$Value)

    if ([string]::IsNullOrWhiteSpace($Value)) {
        return $script:BlobUiDefaultSettings.bannerPath
    }

    $normalized = $Value.Trim().Replace('/', '\')
    if ([System.IO.Path]::IsPathRooted($normalized) -or $normalized.Contains(':')) {
        throw "'Banner' must be a relative path inside the packaged assets folder."
    }
    if ($normalized.StartsWith('assets\', [System.StringComparison]::OrdinalIgnoreCase)) {
        $normalized = $normalized.Substring(7)
    }

    $parts = $normalized -split '\\'
    if ($parts.Count -eq 0) {
        throw "'Banner' must name an image in the packaged assets folder."
    }
    foreach ($part in $parts) {
        if ([string]::IsNullOrWhiteSpace($part) -or $part -eq '.' -or $part -eq '..' -or
            $part -notmatch '^[\p{L}\p{N} ._-]+$') {
            throw "'Banner' contains a path segment that is not allowed."
        }
    }
    $extension = [System.IO.Path]::GetExtension($parts[$parts.Count - 1])
    if ($extension -notmatch '^\.(?:png|jpe?g|webp|gif)$') {
        throw "'Banner' must point to a PNG, JPEG, WebP, or GIF image."
    }
    return [string]::Join('\', $parts)
}

function Select-BlobUiItem {
    param(
        [Parameter(Mandatory = $true)][object[]]$Items,
        [Parameter(Mandatory = $true)][string]$Prompt,
        [Parameter(Mandatory = $true)][string]$NameProperty,
        [scriptblock]$SelectionProvider
    )

    if ($Items.Count -eq 0) {
        throw "No choices are available for $Prompt."
    }
    for ($index = 0; $index -lt $Items.Count; $index++) {
        Write-Host ('{0}) {1}' -f ($index + 1), [string]$Items[$index].$NameProperty)
    }
    if ($null -ne $SelectionProvider) {
        $answer = [string](& $SelectionProvider 'single' $Prompt $Items)
    }
    else {
        $answer = (Read-Host $Prompt).Trim()
    }
    $number = 0
    if (-not [int]::TryParse($answer, [ref]$number) -or $number -lt 1 -or $number -gt $Items.Count) {
        throw 'The selected number is not in the list.'
    }
    return $Items[$number - 1]
}

function Select-BlobUiObjectIndices {
    param(
        [Parameter(Mandatory = $true)][object[]]$Items,
        [scriptblock]$SelectionProvider
    )

    if ($Items.Count -eq 0) {
        throw "The selected space has no objects of type 'setting'. Confirm the BlobUI Setting type exists in that space."
    }
    for ($index = 0; $index -lt $Items.Count; $index++) {
        $item = $Items[$index]
        Write-Host ('{0}) {1}' -f ($index + 1), [string]$item.name)
    }
    Write-Host 'Select one or more objects in merge order. Later objects override values from earlier objects.'
    $prompt = 'Settings object numbers, separated by commas'
    if ($null -ne $SelectionProvider) {
        $selectionTokens = @(& $SelectionProvider 'multiple' $prompt $Items)
    }
    else {
        $answer = (Read-Host $prompt).Trim()
        if ([string]::IsNullOrWhiteSpace($answer)) {
            throw 'At least one settings object must be selected.'
        }
        $selectionTokens = @($answer -split ',')
    }
    if ($selectionTokens.Count -eq 0) {
        throw 'At least one settings object must be selected.'
    }

    $indices = [System.Collections.Generic.List[int]]::new()
    foreach ($token in $selectionTokens) {
        $number = 0
        if (-not [int]::TryParse(([string]$token).Trim(), [ref]$number) -or $number -lt 1 -or $number -gt $Items.Count) {
            throw 'A selected settings object number is not in the list.'
        }
        if ($indices.Contains($number)) {
            throw 'A settings object was selected more than once.'
        }
        $indices.Add($number)
    }
    return $indices.ToArray()
}

function Get-BlobUiObjectFromApiResponse {
    param([Parameter(Mandatory = $true)]$Response)

    $dataProperty = $Response.PSObject.Properties['data']
    if ($null -ne $dataProperty) {
        $value = $dataProperty.Value
        if ($value -is [System.Array]) {
            if ($value.Count -eq 0) {
                throw 'Anytype returned no object data.'
            }
            return $value[0]
        }
        return $value
    }
    return $Response
}

function Get-BlobUiAnytypeObject {
    param(
        [Parameter(Mandatory = $true)][string]$ApiKey,
        [Parameter(Mandatory = $true)][string]$SpaceId,
        [Parameter(Mandatory = $true)][string]$ObjectId,
        [string]$ApiBaseUri = $script:BlobUiApiBaseUri,
        [scriptblock]$ApiTransport
    )

    $escapedSpace = [System.Uri]::EscapeDataString($SpaceId)
    $escapedObject = [System.Uri]::EscapeDataString($ObjectId)
    $response = Invoke-BlobUiApiRequest -ApiKey $ApiKey -Path ("/v1/spaces/$escapedSpace/objects/$escapedObject") -ApiBaseUri $ApiBaseUri -ApiTransport $ApiTransport
    return Get-BlobUiObjectFromApiResponse -Response $response
}

function Get-BlobUiNormalizedSettings {
    param([Parameter(Mandatory = $true)][object[]]$Objects)

    $rawSettings = @{
        font = $null
        code_font = $null
        accent_color = $null
        banner = $null
    }
    foreach ($object in $Objects) {
        if ((Get-BlobUiObjectTypeKey -Object $object) -cne 'setting') {
            throw "Selected object '$($object.name)' is not of the exported Setting type."
        }
        $propertyMap = @(
            @{ key = 'font'; name = 'Font' },
            @{ key = 'code_font'; name = 'Code Font' },
            @{ key = 'accent_color'; name = 'Accent Color' },
            @{ key = 'banner'; name = 'Banner' }
        )
        foreach ($definition in $propertyMap) {
            $value = Get-BlobUiObjectPropertyText -Object $object -Key $definition.key -DisplayName $definition.name
            if (-not [string]::IsNullOrWhiteSpace($value)) {
                $rawSettings[$definition.key] = $value
            }
        }
    }

    $fontValue = $rawSettings.font
    if ([string]::IsNullOrWhiteSpace($fontValue)) {
        $fontValue = $script:BlobUiDefaultSettings.fontFamily
    }
    $codeFontValue = $rawSettings.code_font
    if ([string]::IsNullOrWhiteSpace($codeFontValue)) {
        $codeFontValue = $script:BlobUiDefaultSettings.codeFontFamily
    }
    $accentValue = $rawSettings.accent_color
    if ([string]::IsNullOrWhiteSpace($accentValue)) {
        $accentValue = $script:BlobUiDefaultSettings.accentColor
    }

    return [ordered]@{
        fontFamily = ConvertTo-BlobUiCssPlaceholderValue -Type font -Value ([string]$fontValue) -Name 'Font'
        codeFontFamily = ConvertTo-BlobUiCssPlaceholderValue -Type font -Value ([string]$codeFontValue) -Name 'Code Font'
        accentColor = ConvertTo-BlobUiCssPlaceholderValue -Type color -Value ([string]$accentValue) -Name 'Accent Color'
        bannerPath = ConvertTo-BlobUiBannerRelativePath -Value ([string]$rawSettings.banner)
    }
}

function Read-BlobUiDataFile {
    param([Parameter(Mandatory = $true)][string]$Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "BlobUI settings file was not found: $Path"
    }
    try {
        $json = [System.IO.File]::ReadAllText([System.IO.Path]::GetFullPath($Path), [System.Text.Encoding]::UTF8)
        return ConvertFrom-Json -InputObject $json -ErrorAction Stop
    }
    catch {
        throw 'BlobUI data.json is not valid JSON.'
    }
}

function Write-BlobUiAtomicText {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Content
    )

    $fullPath = [System.IO.Path]::GetFullPath($Path)
    $directory = [System.IO.Path]::GetDirectoryName($fullPath)
    [System.IO.Directory]::CreateDirectory($directory) | Out-Null
    $temporaryPath = $fullPath + '.' + [System.Guid]::NewGuid().ToString('N') + '.tmp'
    $replacementBackupPath = $fullPath + '.' + [System.Guid]::NewGuid().ToString('N') + '.replace.bak'
    $utf8WithoutBom = [System.Text.UTF8Encoding]::new($false)
    try {
        [System.IO.File]::WriteAllText($temporaryPath, $Content, $utf8WithoutBom)
        if ([System.IO.File]::Exists($fullPath)) {
            [System.IO.File]::Replace($temporaryPath, $fullPath, $replacementBackupPath)
        }
        else {
            [System.IO.File]::Move($temporaryPath, $fullPath)
        }
    }
    finally {
        if ([System.IO.File]::Exists($temporaryPath)) {
            [System.IO.File]::Delete($temporaryPath)
        }
        if ([System.IO.File]::Exists($replacementBackupPath)) {
            [System.IO.File]::Delete($replacementBackupPath)
        }
    }
}

function Write-BlobUiAtomicJson {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$InputObject
    )

    $json = ConvertTo-Json -InputObject $InputObject -Depth 20
    Write-BlobUiAtomicText -Path $Path -Content ($json + [Environment]::NewLine)
}

function Get-BlobUiSettingsData {
    param(
        [Parameter(Mandatory = $true)][string]$ApiKey,
        [Parameter(Mandatory = $true)][string]$OutputPath,
        [string]$SpaceId,
        [string[]]$ObjectIds,
        [string]$ExistingDataPath,
        [string]$InstallDirectory,
        [string]$ApiBaseUri = $script:BlobUiApiBaseUri,
        [scriptblock]$SelectionProvider,
        [scriptblock]$ApiTransport
    )

    $spaces = @(Get-BlobUiPagedApiItems -ApiKey $ApiKey -Path '/v1/spaces' -ApiBaseUri $ApiBaseUri -ApiTransport $ApiTransport)
    if ($spaces.Count -eq 0) {
        throw 'The API key cannot access any Anytype spaces.'
    }

    if ([string]::IsNullOrWhiteSpace($SpaceId)) {
        $spaces = @($spaces | Sort-Object -Property name)
        $selectedSpace = Select-BlobUiItem -Items $spaces -Prompt 'Select the settings space number' -NameProperty 'name' -SelectionProvider $SelectionProvider
        $SpaceId = [string]$selectedSpace.id
    }
    else {
        $selectedSpaceResult = @($spaces | Where-Object { [string]$_.id -ceq $SpaceId } | Select-Object -First 1)
        if ($selectedSpaceResult.Count -eq 0) {
            throw 'The saved settings space is no longer available to this API key.'
        }
        $selectedSpace = $selectedSpaceResult[0]
    }

    if ($null -eq $selectedSpace.PSObject.Properties['id'] -or [string]::IsNullOrWhiteSpace([string]$selectedSpace.id)) {
        throw 'The Anytype spaces response did not include a space ID.'
    }

    $selectedObjects = [System.Collections.Generic.List[object]]::new()
    if ($null -ne $ObjectIds -and $ObjectIds.Count -gt 0) {
        foreach ($objectId in $ObjectIds) {
            $selectedObjects.Add((Get-BlobUiAnytypeObject -ApiKey $ApiKey -SpaceId $SpaceId -ObjectId $objectId -ApiBaseUri $ApiBaseUri -ApiTransport $ApiTransport))
        }
    }
    else {
        $escapedSpace = [System.Uri]::EscapeDataString($SpaceId)
        $settingsObjects = @(Get-BlobUiPagedApiItems -ApiKey $ApiKey -Path ("/v1/spaces/$escapedSpace/objects?type=setting") -ApiBaseUri $ApiBaseUri -ApiTransport $ApiTransport)
        $settingsObjects = @($settingsObjects | Where-Object { (Get-BlobUiObjectTypeKey -Object $_) -ceq 'setting' } | Sort-Object -Property name)
        $indices = @(Select-BlobUiObjectIndices -Items $settingsObjects -SelectionProvider $SelectionProvider)
        foreach ($index in $indices) {
            $selectedObjects.Add((Get-BlobUiAnytypeObject -ApiKey $ApiKey -SpaceId $SpaceId -ObjectId ([string]$settingsObjects[$index - 1].id) -ApiBaseUri $ApiBaseUri -ApiTransport $ApiTransport))
        }
    }

    if ($selectedObjects.Count -eq 0) {
        throw 'At least one settings object must be selected.'
    }
    $normalizedSettings = Get-BlobUiNormalizedSettings -Objects $selectedObjects.ToArray()
    $previousData = $null
    if (-not [string]::IsNullOrWhiteSpace($ExistingDataPath) -and (Test-Path -LiteralPath $ExistingDataPath -PathType Leaf)) {
        $previousData = Read-BlobUiDataFile -Path $ExistingDataPath
    }
    if ([string]::IsNullOrWhiteSpace($InstallDirectory) -and $null -ne $previousData) {
        $InstallDirectory = [string]$previousData.installDirectory
    }
    if ([string]::IsNullOrWhiteSpace($InstallDirectory)) {
        throw 'An Anytype install directory must be supplied for the settings record.'
    }

    $data = [ordered]@{
        schemaVersion = 1
        apiVersion = $script:BlobUiApiVersion
        fetchedAtUtc = [DateTime]::UtcNow.ToString('o')
        selectedSpace = [ordered]@{
            id = [string]$selectedSpace.id
            name = [string]$selectedSpace.name
        }
        settingsObjectIds = @($selectedObjects.ToArray() | ForEach-Object { [string]$_.id })
        settings = $normalizedSettings
        installDirectory = [System.IO.Path]::GetFullPath($InstallDirectory)
    }

    Write-BlobUiAtomicJson -Path $OutputPath -InputObject $data
    return $data
}

function Get-BlobUiDeclaredPlaceholders {
    param([Parameter(Mandatory = $true)][string]$CssText)

    $declarations = @{}
    $pattern = '/\*\s*@blobui-placeholder\s+(?<name>[A-Za-z][A-Za-z0-9]*)\s+(?<type>font|color|url|number|boolean)\s*\*/'
    foreach ($match in [System.Text.RegularExpressions.Regex]::Matches($CssText, $pattern)) {
        $name = $match.Groups['name'].Value
        $type = $match.Groups['type'].Value
        if ($declarations.ContainsKey($name) -and $declarations[$name] -cne $type) {
            throw "The '$name' CSS placeholder has conflicting type declarations."
        }
        $declarations[$name] = $type
    }
    if ($declarations.Count -eq 0 -and $CssText -match '\{\{') {
        throw 'CSS placeholders must have explicit @blobui-placeholder declarations.'
    }
    return $declarations
}

function Invoke-BlobUiCssCompile {
    param(
        [Parameter(Mandatory = $true)][string]$DevDirectory,
        [Parameter(Mandatory = $true)][string]$AssetSourceDirectory,
        [Parameter(Mandatory = $true)][string]$OutputPath,
        [Parameter(Mandatory = $true)]$Data
    )

    $devDirectoryPath = [System.IO.Path]::GetFullPath($DevDirectory).TrimEnd([System.IO.Path]::DirectorySeparatorChar)
    $devFullPath = $devDirectoryPath + [System.IO.Path]::DirectorySeparatorChar
    $outputFullPath = [System.IO.Path]::GetFullPath($OutputPath)
    if ($outputFullPath.Equals($devDirectoryPath, [System.StringComparison]::OrdinalIgnoreCase) -or
        $outputFullPath.StartsWith($devFullPath, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw 'Compiled CSS must be written outside the Dev directory.'
    }
    if (-not (Test-Path -LiteralPath $DevDirectory -PathType Container)) {
        throw "Development CSS directory was not found: $DevDirectory"
    }

    $sourceFilesByPath = @{}
    $relativePaths = [System.Collections.Generic.List[string]]::new()
    foreach ($file in (Get-ChildItem -LiteralPath $DevDirectory -Recurse -File -Filter '*.css')) {
        $relativePath = $file.FullName.Substring($devFullPath.Length).Replace('\', '/')
        $sourceFilesByPath[$relativePath] = $file.FullName
        $relativePaths.Add($relativePath)
    }
    if ($relativePaths.Count -eq 0) {
        throw 'No CSS source files were found under Dev.'
    }
    $orderedRelativePaths = $relativePaths.ToArray()
    [System.Array]::Sort($orderedRelativePaths, [System.StringComparer]::Ordinal)
    $segments = [System.Collections.Generic.List[string]]::new()
    foreach ($relativePath in $orderedRelativePaths) {
        $sourcePath = $sourceFilesByPath[$relativePath]
        $sourceText = [System.IO.File]::ReadAllText($sourcePath, [System.Text.Encoding]::UTF8)
        $segments.Add("/* BlobUI source: $relativePath */")
        $segments.Add($sourceText)
    }
    $cssText = [string]::Join([Environment]::NewLine, $segments.ToArray())
    $declarations = Get-BlobUiDeclaredPlaceholders -CssText $cssText

    $installDirectory = [string]$Data.installDirectory
    if ([string]::IsNullOrWhiteSpace($installDirectory)) {
        throw 'The settings data has no install directory.'
    }
    $bannerPath = ConvertTo-BlobUiBannerRelativePath -Value ([string]$Data.settings.bannerPath)
    $bannerSource = [System.IO.Path]::GetFullPath((Join-Path $AssetSourceDirectory $bannerPath))
    $assetRoot = [System.IO.Path]::GetFullPath($AssetSourceDirectory).TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
    if (-not $bannerSource.StartsWith($assetRoot, [System.StringComparison]::OrdinalIgnoreCase) -or
        -not (Test-Path -LiteralPath $bannerSource -PathType Leaf)) {
        throw "The selected banner image is not in the packaged assets: $bannerPath"
    }

    $bannerDestination = Join-Path ([System.IO.Path]::GetFullPath($installDirectory)) (Join-Path 'blob\assets' $bannerPath)
    $bannerUri = [System.Uri]::new($bannerDestination).AbsoluteUri
    $values = @{
        fontFamily = @{ type = 'font'; value = [string]$Data.settings.fontFamily }
        codeFontFamily = @{ type = 'font'; value = [string]$Data.settings.codeFontFamily }
        accentColor = @{ type = 'color'; value = [string]$Data.settings.accentColor }
        bannerUrl = @{ type = 'url'; value = $bannerUri }
    }

    $placeholderPattern = '\{\{\s*(?<name>[A-Za-z][A-Za-z0-9]*)\s*\}\}'
    foreach ($match in [System.Text.RegularExpressions.Regex]::Matches($cssText, $placeholderPattern)) {
        $name = $match.Groups['name'].Value
        if (-not $declarations.ContainsKey($name)) {
            throw "The '$name' CSS placeholder has no explicit declaration."
        }
        if (-not $values.ContainsKey($name)) {
            throw "The '$name' CSS placeholder has no settings value."
        }
        if ($declarations[$name] -cne $values[$name].type) {
            throw "The '$name' CSS placeholder type does not match its settings value."
        }
    }

    $compiled = [System.Text.RegularExpressions.Regex]::Replace(
        $cssText,
        $placeholderPattern,
        [System.Text.RegularExpressions.MatchEvaluator]{
            param($match)
            $name = $match.Groups['name'].Value
            $entry = $values[$name]
            return ConvertTo-BlobUiCssPlaceholderValue -Type $entry.type -Value ([string]$entry.value) -Name $name
        }
    )
    $declarationPattern = '/\*\s*@blobui-placeholder\s+(?<name>[A-Za-z][A-Za-z0-9]*)\s+(?<type>font|color|url|number|boolean)\s*\*/'
    $compiled = [System.Text.RegularExpressions.Regex]::Replace($compiled, $declarationPattern, '')
    if ($compiled -match '\{\{[^}]+\}\}') {
        throw 'Compiled CSS still contains an unresolved placeholder.'
    }
    Write-BlobUiAtomicText -Path $outputFullPath -Content $compiled
    return [ordered]@{
        path = $outputFullPath
        sourceFiles = @($orderedRelativePaths | ForEach-Object { Join-Path $DevDirectory $_ })
        bannerUri = $bannerUri
    }
}

function Get-BlobUiUniqueBackupPath {
    param([Parameter(Mandatory = $true)][string]$BackupDirectory)

    $stamp = Get-Date -Format 'yyyyMMdd-HHmmssfff'
    $candidate = Join-Path $BackupDirectory ("custom.css.$stamp.bak")
    $counter = 1
    while (Test-Path -LiteralPath $candidate) {
        $candidate = Join-Path $BackupDirectory ("custom.css.$stamp-$counter.bak")
        $counter++
    }
    return $candidate
}

function Copy-BlobUiDirectoryContents {
    param(
        [Parameter(Mandatory = $true)][string]$SourceDirectory,
        [Parameter(Mandatory = $true)][string]$DestinationDirectory
    )

    if (-not (Test-Path -LiteralPath $SourceDirectory -PathType Container)) {
        throw "Source directory was not found: $SourceDirectory"
    }
    [System.IO.Directory]::CreateDirectory($DestinationDirectory) | Out-Null
    $items = @(Get-ChildItem -LiteralPath $SourceDirectory -Force)
    if ($items.Count -eq 0) {
        throw "Source directory is empty: $SourceDirectory"
    }
    foreach ($item in $items) {
        Copy-Item -LiteralPath $item.FullName -Destination $DestinationDirectory -Recurse -Force -ErrorAction Stop
    }
}

function Copy-BlobUiRuntime {
    param(
        [Parameter(Mandatory = $true)][string]$SourceDirectory,
        [Parameter(Mandatory = $true)][string]$RuntimeDirectory
    )

    $sourceFullPath = [System.IO.Path]::GetFullPath($SourceDirectory).TrimEnd([System.IO.Path]::DirectorySeparatorChar)
    $runtimeFullPath = [System.IO.Path]::GetFullPath($RuntimeDirectory).TrimEnd([System.IO.Path]::DirectorySeparatorChar)
    if ($sourceFullPath.Equals($runtimeFullPath, [System.StringComparison]::OrdinalIgnoreCase)) {
        return
    }

    [System.IO.Directory]::CreateDirectory($runtimeFullPath) | Out-Null
    foreach ($scriptName in @('BlobUI.Core.psm1', 'Get-Data.ps1', 'Compile.ps1', 'Rebuild.ps1')) {
        $sourcePath = Join-Path $sourceFullPath $scriptName
        if (-not (Test-Path -LiteralPath $sourcePath -PathType Leaf)) {
            throw "BlobUI runtime script was not found: $scriptName"
        }
        Copy-Item -LiteralPath $sourcePath -Destination $runtimeFullPath -Force -ErrorAction Stop
    }
    Copy-BlobUiDirectoryContents -SourceDirectory (Join-Path $sourceFullPath 'Dev') -DestinationDirectory (Join-Path $runtimeFullPath 'Dev')
    Copy-BlobUiDirectoryContents -SourceDirectory (Join-Path $sourceFullPath 'assets') -DestinationDirectory (Join-Path $runtimeFullPath 'assets')
}

function Set-BlobUiCredentialAcl {
    param([Parameter(Mandatory = $true)][string]$Path)

    if ($env:OS -ne 'Windows_NT') {
        throw 'Encrypted API key storage requires Windows DPAPI.'
    }
    try {
        $identity = [System.Security.Principal.WindowsIdentity]::GetCurrent()
        $acl = Get-Acl -LiteralPath $Path -ErrorAction Stop
        $acl.SetAccessRuleProtection($true, $false)
        $rule = [System.Security.AccessControl.FileSystemAccessRule]::new(
            $identity.User,
            [System.Security.AccessControl.FileSystemRights]::FullControl,
            [System.Security.AccessControl.AccessControlType]::Allow
        )
        $acl.SetAccessRule($rule)
        Set-Acl -LiteralPath $Path -AclObject $acl -ErrorAction Stop
    }
    catch {
        throw 'Could not restrict the encrypted API key file to the current Windows user.'
    }
}

function New-BlobUiProtectedCredential {
    param([Parameter(Mandatory = $true)][string]$ApiKey)

    if ($env:OS -ne 'Windows_NT') {
        throw 'Encrypted API key storage requires Windows DPAPI.'
    }
    if ([string]::IsNullOrWhiteSpace($ApiKey)) {
        throw 'An Anytype API key is required.'
    }
    $secureString = ConvertTo-SecureString -String $ApiKey -AsPlainText -Force
    try {
        return ConvertFrom-SecureString -SecureString $secureString
    }
    finally {
        $secureString.Dispose()
    }
}

function Read-BlobUiProtectedCredential {
    param([Parameter(Mandatory = $true)][string]$Path)

    if ($env:OS -ne 'Windows_NT') {
        throw 'Encrypted API key storage requires Windows DPAPI.'
    }
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw 'The encrypted Anytype API key was not found. Run Install.ps1 to enter it again.'
    }
    $encryptedText = [System.IO.File]::ReadAllText([System.IO.Path]::GetFullPath($Path), [System.Text.Encoding]::UTF8).Trim()
    $secureString = $null
    $pointer = [IntPtr]::Zero
    try {
        $secureString = ConvertTo-SecureString -String $encryptedText -ErrorAction Stop
        $pointer = [System.Runtime.InteropServices.Marshal]::SecureStringToBSTR($secureString)
        return [System.Runtime.InteropServices.Marshal]::PtrToStringBSTR($pointer)
    }
    catch {
        throw 'The encrypted Anytype API key cannot be opened under this Windows account. Run Install.ps1 to enter it again.'
    }
    finally {
        if ($pointer -ne [IntPtr]::Zero) {
            [System.Runtime.InteropServices.Marshal]::ZeroFreeBSTR($pointer)
        }
        if ($null -ne $secureString) {
            $secureString.Dispose()
        }
    }
}

function Invoke-BlobUiInstall {
    param(
        [Parameter(Mandatory = $true)][string]$InstallDirectory,
        [Parameter(Mandatory = $true)][string]$RuntimeSourceDirectory,
        [Parameter(Mandatory = $true)][string]$AssetsSourceDirectory,
        [Parameter(Mandatory = $true)][string]$CompiledCssPath,
        [Parameter(Mandatory = $true)][string]$DataCandidatePath,
        [string]$CredentialProtectedText
    )

    $installFullPath = [System.IO.Path]::GetFullPath($InstallDirectory)
    if (-not (Test-Path -LiteralPath $installFullPath -PathType Container)) {
        throw "The selected Anytype directory does not exist: $installFullPath"
    }
    if (-not (Test-Path -LiteralPath $CompiledCssPath -PathType Leaf)) {
        throw "Compiled CSS was not found: $CompiledCssPath"
    }
    if (-not (Test-Path -LiteralPath $DataCandidatePath -PathType Leaf)) {
        throw "Fetched settings data was not found: $DataCandidatePath"
    }
    $candidateData = Read-BlobUiDataFile -Path $DataCandidatePath
    if ([System.IO.Path]::GetFullPath([string]$candidateData.installDirectory) -cne $installFullPath) {
        throw 'The fetched settings directory does not match the selected Anytype directory.'
    }
    $bannerPath = ConvertTo-BlobUiBannerRelativePath -Value ([string]$candidateData.settings.bannerPath)
    $bannerSource = [System.IO.Path]::GetFullPath((Join-Path $AssetsSourceDirectory $bannerPath))
    $assetRoot = [System.IO.Path]::GetFullPath($AssetsSourceDirectory).TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
    if (-not $bannerSource.StartsWith($assetRoot, [System.StringComparison]::OrdinalIgnoreCase) -or
        -not (Test-Path -LiteralPath $bannerSource -PathType Leaf)) {
        throw "The selected banner image is not in the packaged assets: $bannerPath"
    }

    $blobDirectory = Join-Path $installFullPath 'blob'
    $assetDestination = Join-Path $blobDirectory 'assets'
    $runtimeDestination = Join-Path $blobDirectory 'runtime'
    $backupDirectory = Join-Path $blobDirectory 'backups'
    $credentialDirectory = Join-Path $blobDirectory 'credentials'
    $credentialPath = Join-Path $credentialDirectory 'anytype-api-key.dpapi'
    $dataPath = Join-Path $blobDirectory 'data.json'
    $cssDestination = Join-Path $installFullPath 'custom.css'
    $oldCssExists = Test-Path -LiteralPath $cssDestination -PathType Leaf
    $oldDataExists = Test-Path -LiteralPath $dataPath -PathType Leaf
    $oldCredentialExists = Test-Path -LiteralPath $credentialPath -PathType Leaf
    $oldCss = $null
    $oldData = $null
    $oldCredential = $null
    if ($oldCssExists) { $oldCss = [System.IO.File]::ReadAllText($cssDestination, [System.Text.Encoding]::UTF8) }
    if ($oldDataExists) { $oldData = [System.IO.File]::ReadAllText($dataPath, [System.Text.Encoding]::UTF8) }
    if ($oldCredentialExists) { $oldCredential = [System.IO.File]::ReadAllText($credentialPath, [System.Text.Encoding]::UTF8) }

    [System.IO.Directory]::CreateDirectory($blobDirectory) | Out-Null
    try {
        Copy-BlobUiDirectoryContents -SourceDirectory $AssetsSourceDirectory -DestinationDirectory $assetDestination
        Copy-BlobUiRuntime -SourceDirectory $RuntimeSourceDirectory -RuntimeDirectory $runtimeDestination

        if ($oldCssExists) {
            [System.IO.Directory]::CreateDirectory($backupDirectory) | Out-Null
            $backupPath = Get-BlobUiUniqueBackupPath -BackupDirectory $backupDirectory
            Copy-Item -LiteralPath $cssDestination -Destination $backupPath -ErrorAction Stop
            Write-Host "Existing CSS backed up to: $backupPath"
        }

        $compiledCss = [System.IO.File]::ReadAllText([System.IO.Path]::GetFullPath($CompiledCssPath), [System.Text.Encoding]::UTF8)
        Write-BlobUiAtomicText -Path $cssDestination -Content $compiledCss
        Write-BlobUiAtomicText -Path $dataPath -Content ([System.IO.File]::ReadAllText([System.IO.Path]::GetFullPath($DataCandidatePath), [System.Text.Encoding]::UTF8))
        if (-not [string]::IsNullOrWhiteSpace($CredentialProtectedText)) {
            [System.IO.Directory]::CreateDirectory($credentialDirectory) | Out-Null
            Write-BlobUiAtomicText -Path $credentialPath -Content ($CredentialProtectedText + [Environment]::NewLine)
            Set-BlobUiCredentialAcl -Path $credentialPath
        }
    }
    catch {
        $installFailure = $_.Exception.Message
        try {
            if ($oldCssExists) {
                Write-BlobUiAtomicText -Path $cssDestination -Content $oldCss
            }
            elseif (Test-Path -LiteralPath $cssDestination -PathType Leaf) {
                Remove-Item -LiteralPath $cssDestination -Force -ErrorAction Stop
            }
            if ($oldDataExists) {
                Write-BlobUiAtomicText -Path $dataPath -Content $oldData
            }
            elseif (Test-Path -LiteralPath $dataPath -PathType Leaf) {
                Remove-Item -LiteralPath $dataPath -Force -ErrorAction Stop
            }
            if ($oldCredentialExists) {
                Write-BlobUiAtomicText -Path $credentialPath -Content $oldCredential
                Set-BlobUiCredentialAcl -Path $credentialPath
            }
            elseif (Test-Path -LiteralPath $credentialPath -PathType Leaf) {
                Remove-Item -LiteralPath $credentialPath -Force -ErrorAction Stop
            }
        }
        catch {
            throw "Installation failed and rollback could not restore all saved files. $installFailure"
        }
        throw $installFailure
    }

    return [ordered]@{
        cssPath = $cssDestination
        assetsPath = $assetDestination
        dataPath = $dataPath
        runtimePath = $runtimeDestination
        credentialPath = if (-not [string]::IsNullOrWhiteSpace($CredentialProtectedText)) { $credentialPath } else { $null }
    }
}

Export-ModuleMember -Function @(
    'ConvertTo-BlobUiCssPlaceholderValue',
    'Invoke-BlobUiCssCompile',
    'Copy-BlobUiDirectoryContents',
    'Get-BlobUiSettingsData',
    'Invoke-BlobUiInstall',
    'New-BlobUiProtectedCredential',
    'Read-BlobUiDataFile',
    'Read-BlobUiProtectedCredential',
    'Write-BlobUiAtomicJson',
    'Write-BlobUiAtomicText'
)
