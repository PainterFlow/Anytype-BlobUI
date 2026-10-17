Set-StrictMode -Version Latest

$script:BlobUiApiVersion = 'v2'
$script:BlobUiApiBaseUri = 'http://127.0.0.1:31009'
$script:BlobUiReleaseVersion = '1.1.0'
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

function Get-BlobUiHttpErrorBody {
    param([Parameter(Mandatory = $true)]$ErrorRecord)

    $errorDetailsProperty = $ErrorRecord.PSObject.Properties['ErrorDetails']
    $errorDetails = if ($null -ne $errorDetailsProperty) { $errorDetailsProperty.Value } else { $null }
    if ($null -ne $errorDetails -and -not [string]::IsNullOrWhiteSpace([string]$errorDetails.Message)) {
        return [string]$errorDetails.Message
    }

    $exception = $ErrorRecord.Exception
    $responseProperty = $exception.PSObject.Properties['Response']
    $response = if ($null -ne $responseProperty) { $responseProperty.Value } else { $null }
    if ($null -ne $response) {
        try {
            $contentProperty = $response.PSObject.Properties['Content']
            if ($null -ne $contentProperty -and $null -ne $contentProperty.Value) {
                $content = $contentProperty.Value
                if ($content -is [string]) { return $content }
                $readMethod = $content.GetType().GetMethod('ReadAsStringAsync', [type[]]@())
                if ($null -ne $readMethod) { return [string]$readMethod.Invoke($content, @()).GetAwaiter().GetResult() }
            }
        }
        catch {}
        try {
            $stream = $response.GetResponseStream()
            if ($null -ne $stream) {
                $reader = [System.IO.StreamReader]::new($stream)
                try { return $reader.ReadToEnd() } finally { $reader.Dispose() }
            }
        }
        catch {}
    }

    try {
        if (-not [string]::IsNullOrWhiteSpace([string]$exception.Data['ResponseBody'])) {
            return [string]$exception.Data['ResponseBody']
        }
    }
    catch {}
    return ''
}

function Get-BlobUiSafeApiErrorMessage {
    param([string]$Body, [string]$ApiKey)

    if ([string]::IsNullOrWhiteSpace($Body)) { return '' }

    $message = $Body.Trim()
    try {
        $payload = ConvertFrom-Json -InputObject $message -ErrorAction Stop
        foreach ($fieldName in @('message', 'error', 'detail', 'title')) {
            $field = $payload.PSObject.Properties[$fieldName]
            if ($null -ne $field -and -not [string]::IsNullOrWhiteSpace([string]$field.Value)) {
                $message = [string]$field.Value
                break
            }
        }
    }
    catch {}

    if (-not [string]::IsNullOrEmpty($ApiKey)) {
        $message = $message.Replace($ApiKey, '[redacted]')
    }
    $message = [System.Text.RegularExpressions.Regex]::Replace($message, '(?i)authorization\s*[:=]\s*(?:bearer\s+)?[^\s"'',<>]+', 'Authorization: [redacted]')
    $message = [System.Text.RegularExpressions.Regex]::Replace($message, '(?i)bearer\s+[^\s"'',<>]+', 'Bearer [redacted]')
    $message = [System.Text.RegularExpressions.Regex]::Replace($message, '[\r\n\t]+', ' ')
    $message = $message.Trim()
    if ($message.Length -gt 400) { $message = $message.Substring(0, 400) + '…' }
    return $message
}

function Invoke-BlobUiApiRequest {
    param(
        [Parameter(Mandatory = $true)][string]$ApiKey,
        [Parameter(Mandatory = $true)][string]$Path,
        [ValidateSet('Get', 'Post')][string]$Method = 'Get',
        [AllowNull()]$Body,
        [string]$ApiBaseUri = $script:BlobUiApiBaseUri,
        [scriptblock]$ApiTransport
    )

    if ([string]::IsNullOrWhiteSpace($ApiKey)) {
        throw 'An Anytype API key is required.'
    }
    if ($Method -eq 'Post' -and $null -eq $Body) {
        throw 'A JSON body is required for the Anytype API v2 search request.'
    }

    $uri = $ApiBaseUri.TrimEnd('/') + $Path
    $headers = @{
        Authorization = 'Bearer ' + $ApiKey
        Accept = 'application/json'
    }

    try {
        if ($null -ne $ApiTransport) {
            return & $ApiTransport $ApiKey $Method $Path $Body $headers
        }
        if ($Method -eq 'Post') {
            $jsonBody = ConvertTo-Json -InputObject $Body -Depth 20 -Compress
            return Invoke-RestMethod -Method Post -Uri $uri -Headers $headers -ContentType 'application/json' -Body $jsonBody -TimeoutSec 15 -ErrorAction Stop
        }
        return Invoke-RestMethod -Method Get -Uri $uri -Headers $headers -TimeoutSec 15 -ErrorAction Stop
    }
    catch {
        $statusCode = Get-BlobUiHttpStatusCode -ErrorRecord $_
        $serverMessage = Get-BlobUiSafeApiErrorMessage -Body (Get-BlobUiHttpErrorBody -ErrorRecord $_) -ApiKey $ApiKey
        if ($statusCode -eq 401) {
            $message = 'Anytype rejected the API key (HTTP 401). Confirm the key is valid.'
            if ($serverMessage) { $message += " Anytype said: $serverMessage" }
            throw $message
        }
        if ($statusCode -eq 403) {
            $message = 'Anytype accepted the API key but denied this API v2 read (HTTP 403). Confirm the key grants read access to the selected space.'
            if ($serverMessage) { $message += " Anytype said: $serverMessage" }
            throw $message
        }
        if ($statusCode -eq 404) {
            $message = 'Anytype API v2 returned HTTP 404. Confirm this Anytype build provides API v2 and the selected space or settings object still exists.'
            if ($serverMessage) { $message += " Anytype said: $serverMessage" }
            throw $message
        }
        if ($statusCode -eq 400) {
            $message = 'Anytype rejected the API v2 request (HTTP 400). Check the settings type key and request parameters.'
            if ($serverMessage) { $message += " Anytype said: $serverMessage" }
            throw $message
        }
        if ($statusCode -gt 0) {
            $message = "Anytype returned HTTP $statusCode while reading local API data."
            if ($serverMessage) { $message += " Anytype said: $serverMessage" }
            throw $message
        }
        $exceptionMessage = [string]$_.Exception.Message
        if ($exceptionMessage -match '(?i)timeout|timed out') {
            throw 'The request to Anytype local API at 127.0.0.1:31009 timed out. Confirm Anytype is open and its local API is enabled.'
        }
        throw 'Could not reach Anytype local API at 127.0.0.1:31009 because the connection failed. Confirm Anytype is open and its local API is enabled.'
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
        [ValidateSet('Get', 'Post')][string]$Method = 'Get',
        [AllowNull()]$Body,
        [string]$ApiBaseUri = $script:BlobUiApiBaseUri,
        [scriptblock]$ApiTransport
    )

    $limit = 100
    $offset = 0
    $allItems = [System.Collections.Generic.List[object]]::new()

    while ($true) {
        $separator = '?'
        if ($Path.Contains('?')) {
            $separator = '&'
        }
        $pagePath = $Path + $separator + 'limit=' + $limit + '&offset=' + $offset
        $response = Invoke-BlobUiApiRequest -ApiKey $ApiKey -Path $pagePath -Method $Method -Body $Body -ApiBaseUri $ApiBaseUri -ApiTransport $ApiTransport
        $items = @(Get-BlobUiResponseItems -Response $response)
        foreach ($item in $items) {
            if ($null -ne $item) {
                $allItems.Add($item)
            }
        }

        $hasMoreProperty = $response.PSObject.Properties['has_more']
        if ($null -eq $hasMoreProperty) {
            throw 'Anytype API v2 returned a paged response without the required has_more field.'
        }
        $hasMore = [bool]$hasMoreProperty.Value

        if ($hasMore) {
            if ($items.Count -eq 0) {
                throw 'Anytype returned an empty page while more results were reported.'
            }
            $offset += $items.Count
            continue
        }
        break
    }

    return $allItems.ToArray()
}

function Get-BlobUiObjectTypeKey {
    param([Parameter(Mandatory = $true)]$Object)

    $typeProperty = $Object.PSObject.Properties['type']
    if ($null -ne $typeProperty -and $null -ne $typeProperty.Value) {
        if ($typeProperty.Value -is [string]) {
            return ([string]$typeProperty.Value).ToLowerInvariant()
        }
        $keyProperty = $typeProperty.Value.PSObject.Properties['key']
        if ($null -ne $keyProperty) {
            return ([string]$keyProperty.Value).ToLowerInvariant()
        }
        $nameProperty = $typeProperty.Value.PSObject.Properties['name']
        if ($null -ne $nameProperty) {
            return ([string]$nameProperty.Value).ToLowerInvariant()
        }
    }

    $typeKeyProperty = $Object.PSObject.Properties['type_key']
    if ($null -ne $typeKeyProperty) {
        return ([string]$typeKeyProperty.Value).ToLowerInvariant()
    }
    return ''
}

function Get-BlobUiPropertyField {
    param(
        [Parameter(Mandatory = $true)]$InputObject,
        [Parameter(Mandatory = $true)][string]$FieldName
    )

    if ($InputObject -is [System.Collections.IDictionary]) {
        foreach ($key in $InputObject.Keys) {
            if ([string]$key -ceq $FieldName) { return $InputObject[$key] }
        }
        return $null
    }
    $field = $InputObject.PSObject.Properties[$FieldName]
    if ($null -ne $field) { return $field.Value }
    return $null
}

function Get-BlobUiPropertyValue {
    param(
        [Parameter(Mandatory = $true)]$PropertyValue,
        [Parameter(Mandatory = $true)][string]$DisplayName
    )

    if ($null -eq $PropertyValue) { return $null }
    if ($PropertyValue -is [string] -or $PropertyValue -is [ValueType]) { return $PropertyValue }

    $format = [string](Get-BlobUiPropertyField -InputObject $PropertyValue -FieldName 'format')
    $valuePropertyName = if (-not [string]::IsNullOrWhiteSpace($format)) { $format } else { '' }
    if (-not [string]::IsNullOrWhiteSpace($valuePropertyName)) {
        $formattedValue = Get-BlobUiPropertyField -InputObject $PropertyValue -FieldName $valuePropertyName
        if ($null -ne $formattedValue) {
            if ($valuePropertyName -ceq 'select') {
                if ($formattedValue -is [string]) { return [string]$formattedValue }
                $optionName = Get-BlobUiPropertyField -InputObject $formattedValue -FieldName 'name'
                if ($null -ne $optionName) { return [string]$optionName }
                throw "The '$DisplayName' select value has no option name."
            }
            return $formattedValue
        }
    }

    # Some API v2 responses omit the format but still return a typed value.
    foreach ($candidate in @('text', 'select', 'number', 'checkbox', 'date', 'url', 'email', 'phone', 'files')) {
        $formattedValue = Get-BlobUiPropertyField -InputObject $PropertyValue -FieldName $candidate
        if ($null -eq $formattedValue) { continue }
        if ($candidate -ceq 'select') {
            if ($formattedValue -is [string]) { return [string]$formattedValue }
            $optionName = Get-BlobUiPropertyField -InputObject $formattedValue -FieldName 'name'
            if ($null -ne $optionName) { return [string]$optionName }
            throw "The '$DisplayName' select value has no option name."
        }
        return $formattedValue
    }

    throw "The '$DisplayName' property value has no supported typed value."
}

function Get-BlobUiObjectPropertyValue {
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
    $raw = $null
    $found = $false
    if ($properties -is [System.Collections.IDictionary]) {
        foreach ($candidateKey in @($Key, $DisplayName)) {
            foreach ($propertyKey in $properties.Keys) {
                if ([string]$propertyKey -ceq $candidateKey) {
                    $raw = $properties[$propertyKey]
                    $found = $true
                    break
                }
            }
            if ($found) { break }
        }
    }
    elseif ($properties -isnot [System.Array]) {
        foreach ($candidateKey in @($Key, $DisplayName)) {
            foreach ($property in $properties.PSObject.Properties) {
                if ($property.Name -ceq $candidateKey) {
                    $raw = $property.Value
                    $found = $true
                    break
                }
            }
            if ($found) { break }
        }
    }

    if ($found) {
        return Get-BlobUiPropertyValue -PropertyValue $raw -DisplayName $DisplayName
    }

    # Retain compatibility with the earlier typed-property representation in
    # already-exported responses while API v2 uses a display-name/value map.
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

    return Get-BlobUiPropertyValue -PropertyValue $matches[0] -DisplayName $DisplayName
}

function Test-BlobUiCssColorComponent {
    param([Parameter(Mandatory = $true)][string]$Value, [Parameter(Mandatory = $true)][decimal]$Maximum)

    $numberText = $Value.Trim()
    $isPercentage = $numberText.EndsWith('%')
    if ($isPercentage) { $numberText = $numberText.Substring(0, $numberText.Length - 1) }
    if ($numberText -notmatch '^(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)$') { return $false }
    $number = [decimal]0
    if (-not [decimal]::TryParse($numberText, [System.Globalization.NumberStyles]::AllowDecimalPoint, [System.Globalization.CultureInfo]::InvariantCulture, [ref]$number)) {
        return $false
    }
    $limit = if ($isPercentage) { [decimal]100 } else { $Maximum }
    return $number -ge 0 -and $number -le $limit
}

function Test-BlobUiCssColorAlpha {
    param([Parameter(Mandatory = $true)][string]$Value)

    $alphaText = $Value.Trim()
    $isPercentage = $alphaText.EndsWith('%')
    if ($isPercentage) { $alphaText = $alphaText.Substring(0, $alphaText.Length - 1) }
    if ($alphaText -notmatch '^(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)$') { return $false }
    $alpha = [decimal]0
    if (-not [decimal]::TryParse($alphaText, [System.Globalization.NumberStyles]::AllowDecimalPoint, [System.Globalization.CultureInfo]::InvariantCulture, [ref]$alpha)) {
        return $false
    }
    $limit = if ($isPercentage) { [decimal]100 } else { [decimal]1 }
    return $alpha -ge 0 -and $alpha -le $limit
}

function Test-BlobUiCssColorComponentIsMaximum {
    param([Parameter(Mandatory = $true)][string]$Value, [Parameter(Mandatory = $true)][decimal]$Maximum)

    $numberText = $Value.Trim()
    $isPercentage = $numberText.EndsWith('%')
    if ($isPercentage) { $numberText = $numberText.Substring(0, $numberText.Length - 1) }
    $number = [decimal]0
    if (-not [decimal]::TryParse($numberText, [System.Globalization.NumberStyles]::AllowDecimalPoint, [System.Globalization.CultureInfo]::InvariantCulture, [ref]$number)) {
        return $false
    }
    $limit = if ($isPercentage) { [decimal]100 } else { $Maximum }
    return $number -eq $limit
}

function ConvertTo-BlobUiColorValue {
    param(
        [AllowNull()][string]$Value,
        [string]$Name = 'CSS color',
        [switch]$AllowDefault,
        [switch]$AllowWhite,
        [switch]$RejectWhite
    )

    if ([string]::IsNullOrWhiteSpace($Value)) {
        if ($AllowDefault) { return $null }
        throw "'$Name' must be a supported color value."
    }
    $trimmed = $Value.Trim()
    if ($trimmed.Length -gt 128) { throw "'$Name' must be a supported color value." }
    $paletteNames = @{
        grey = 'Grey'
        yellow = 'Yellow'
        amber = 'Amber'
        red = 'Red'
        pink = 'Pink'
        purple = 'Purple'
        blue = 'Blue'
        sky = 'Sky'
        teal = 'Teal'
        green = 'Green'
        'blob pink' = 'Blob Pink'
    }
    $lowerValue = $trimmed.ToLowerInvariant()
    if ($lowerValue -ceq 'default') {
        if ($AllowDefault) { return $null }
        throw "'$Name' does not accept Default."
    }
    if ($paletteNames.ContainsKey($lowerValue)) { return $paletteNames[$lowerValue] }
    if ($AllowWhite -and $lowerValue -ceq 'white') {
        if ($RejectWhite) { throw "White is not supported for $Name. Choose another color or enter 'default'." }
        return 'White'
    }
    if ($trimmed -match '^#(?:[0-9A-Fa-f]{3}|[0-9A-Fa-f]{6})$') {
        if ($RejectWhite -and $trimmed -match '^#(?:[Ff]{3}|[Ff]{6})$') {
            throw "White is not supported for $Name. Choose another color or enter 'default'."
        }
        return $trimmed.ToUpperInvariant()
    }
    if ($trimmed -in @(
        'var(--color-grey)',
        'var(--color-yellow)',
        'var(--color-amber, var(--color-orange))',
        'var(--color-red)',
        'var(--color-pink)',
        'var(--color-purple)',
        'var(--color-blue)',
        'var(--color-sky, var(--color-ice))',
        'var(--color-teal)',
        'var(--color-green, var(--color-lime))'
    )) {
        return $trimmed
    }

    $functionMatch = [System.Text.RegularExpressions.Regex]::Match(
        $trimmed,
        '^(?i:rgba?)\(\s*(?<body>[^()]+)\s*\)$'
    )
    if ($functionMatch.Success) {
        $body = $functionMatch.Groups['body'].Value.Trim()
        $channels = @()
        $alpha = $null
        if ($body.Contains(',')) {
            if ($body -match '/') { throw "'$Name' contains an invalid rgb()/rgba() color." }
            $parts = @($body.Split(',') | ForEach-Object { $_.Trim() })
            if ($parts.Count -notin @(3, 4)) { throw "'$Name' contains an invalid rgb()/rgba() color." }
            $channels = @($parts[0..2])
            $channelUnits = @($channels | ForEach-Object { $_.EndsWith('%') } | Select-Object -Unique)
            if ($channelUnits.Count -gt 1) { throw "'$Name' contains mixed rgb()/rgba() channel units." }
            if ($parts.Count -eq 4) { $alpha = $parts[3] }
        }
        else {
            $spaceMatch = [System.Text.RegularExpressions.Regex]::Match(
                $body,
                '^(?<r>\S+)\s+(?<g>\S+)\s+(?<b>\S+)(?:\s*/\s*(?<a>\S+))?$'
            )
            if (-not $spaceMatch.Success) { throw "'$Name' contains an invalid rgb()/rgba() color." }
            $channels = @($spaceMatch.Groups['r'].Value, $spaceMatch.Groups['g'].Value, $spaceMatch.Groups['b'].Value)
            if ($spaceMatch.Groups['a'].Success) { $alpha = $spaceMatch.Groups['a'].Value }
        }
        foreach ($channel in $channels) {
            if (-not (Test-BlobUiCssColorComponent -Value $channel -Maximum ([decimal]255))) {
                throw "'$Name' contains an rgb()/rgba() channel outside its valid range."
            }
        }
        if ($null -ne $alpha -and -not (Test-BlobUiCssColorAlpha -Value $alpha)) {
            throw "'$Name' contains an rgb()/rgba() alpha outside its valid range."
        }
        if ($RejectWhite -and
            (Test-BlobUiCssColorComponentIsMaximum -Value $channels[0] -Maximum ([decimal]255)) -and
            (Test-BlobUiCssColorComponentIsMaximum -Value $channels[1] -Maximum ([decimal]255)) -and
            (Test-BlobUiCssColorComponentIsMaximum -Value $channels[2] -Maximum ([decimal]255))) {
            throw "White is not supported for $Name. Choose another color or enter 'default'."
        }
        return $trimmed
    }

    throw "'$Name' must be a three or six digit hexadecimal color, a valid rgb()/rgba() color, an Anytype color name (Grey, Yellow, Amber, Red, Pink, Purple, Blue, Sky, Teal, or Green), or Blob Pink."
}

function Get-BlobUiCalendarObjectColors {
    param(
        [Parameter(Mandatory = $true)][string]$ApiKey,
        [Parameter(Mandatory = $true)][string[]]$SpaceIds,
        [string]$ApiBaseUri = $script:BlobUiApiBaseUri,
        [scriptblock]$ApiTransport
    )

    $colors = [ordered]@{}
    foreach ($spaceId in $SpaceIds) {
        if ([string]::IsNullOrWhiteSpace($spaceId)) {
            throw 'The Anytype spaces response included a space without an ID.'
        }

        $escapedSpace = [System.Uri]::EscapeDataString($spaceId)
        $propertyPath = "/v2/spaces/$escapedSpace/properties"
        $propertyRows = @(Get-BlobUiPagedApiItems -ApiKey $ApiKey -Path $propertyPath -ApiBaseUri $ApiBaseUri -ApiTransport $ApiTransport)
        $colorProperties = @($propertyRows | Where-Object {
            $name = Get-BlobUiPropertyField -InputObject $_ -FieldName 'name'
            $null -ne $name -and [string]::Equals([string]$name, 'Calendar Color', [System.StringComparison]::OrdinalIgnoreCase)
        })
        if ($colorProperties.Count -eq 0) {
            continue
        }
        if ($colorProperties.Count -gt 1) {
            throw "The Anytype space '$spaceId' has more than one property named 'Calendar Color'; give the color property a unique name before rebuilding BlobUI."
        }

        $propertyKey = [string](Get-BlobUiPropertyField -InputObject $colorProperties[0] -FieldName 'key')
        if ([string]::IsNullOrWhiteSpace($propertyKey)) {
            throw "The 'Calendar Color' property in Anytype space '$spaceId' has no API key. Rebuild BlobUI after restoring the property."
        }

        # Search rows carry only requested property values. Resolve the display name
        # through this space's property list because property keys can differ by space.
        $searchBody = [ordered]@{ fields = @($propertyKey) }
        $objects = @(Get-BlobUiPagedApiItems -ApiKey $ApiKey -Path ("/v2/spaces/$escapedSpace/search") -Method Post -Body $searchBody -ApiBaseUri $ApiBaseUri -ApiTransport $ApiTransport)
        foreach ($object in $objects) {
            $value = Get-BlobUiObjectPropertyValue -Object $object -Key $propertyKey -DisplayName 'Calendar Color'
            if ($null -eq $value -or [string]::IsNullOrWhiteSpace([string]$value)) { continue }
            $colorName = ConvertTo-BlobUiColorValue -Value ([string]$value) -Name 'Calendar Color' -AllowDefault
            if ($null -eq $colorName) { continue }

            $objectId = [string](Get-BlobUiPropertyField -InputObject $object -FieldName 'id')
            if ($objectId -notmatch '^[A-Za-z0-9][A-Za-z0-9._:-]{0,255}$') {
                throw "An object with Calendar Color '$colorName' in Anytype space '$spaceId' has an invalid object ID."
            }
            if ($colors.Contains($objectId)) {
                if ([string]$colors[$objectId] -cne $colorName) {
                    throw "Anytype object '$objectId' has conflicting Calendar Color values across accessible spaces."
                }
                continue
            }
            $colors[$objectId] = $colorName
        }
    }
    return $colors
}

function ConvertTo-BlobUiCssPlaceholderValue {
    param(
        [Parameter(Mandatory = $true)][ValidateSet('font', 'color', 'url', 'number', 'boolean')][string]$Type,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Value,
        [string]$Name = 'CSS setting',
        [switch]$RejectWhite
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
            $paletteVariables = @{
                white = '#FFFFFF'
                grey = 'var(--color-grey)'
                yellow = 'var(--color-yellow)'
                amber = 'var(--color-amber, var(--color-orange))'
                red = 'var(--color-red)'
                pink = 'var(--color-pink)'
                purple = 'var(--color-purple)'
                blue = 'var(--color-blue)'
                sky = 'var(--color-sky, var(--color-ice))'
                teal = 'var(--color-teal)'
                green = 'var(--color-green, var(--color-lime))'
            }
            $colorValue = ConvertTo-BlobUiColorValue -Value $trimmed -Name $Name -AllowWhite -RejectWhite:$RejectWhite
            $paletteName = $colorValue.ToLowerInvariant()
            if ($paletteVariables.ContainsKey($paletteName)) {
                return $paletteVariables[$paletteName]
            }
            if ($paletteName -ceq 'white') { return '#FFFFFF' }
            if ($paletteName -ceq 'blob pink') {
                return '#DE50DE'
            }
            return $colorValue
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

function ConvertTo-BlobUiAccentColorValue {
    param([AllowNull()][string]$Value)

    if ($null -ne $Value -and $Value.Trim() -ieq 'default') {
        return 'var(--color-system-accent-100)'
    }
    if ($null -ne $Value -and $Value.Trim() -ceq 'var(--color-system-accent-100)') {
        return $Value.Trim()
    }
    return ConvertTo-BlobUiCssPlaceholderValue -Type color -Value ([string]$Value) -Name 'Accent Color' -RejectWhite
}

function Set-BlobUiSettingsV2Overlay {
    param(
        [Parameter(Mandatory = $true)]$Data,
        [Parameter(Mandatory = $true)]$Overlay
    )

    $version = Get-BlobUiPropertyField -InputObject $Overlay -FieldName 'version'
    if ([int]$version -ne 2) {
        throw 'The local BlobUI settings overlay has an unsupported version.'
    }
    $values = Get-BlobUiPropertyField -InputObject $Overlay -FieldName 'settings'
    if ($null -eq $values) {
        throw 'The local BlobUI settings overlay has no settings object.'
    }

    $fontValue = [string](Get-BlobUiPropertyField -InputObject $values -FieldName 'fontFamily')
    $codeFontValue = [string](Get-BlobUiPropertyField -InputObject $values -FieldName 'codeFontFamily')
    $accentValue = [string](Get-BlobUiPropertyField -InputObject $values -FieldName 'accentColor')
    $accentMode = [string](Get-BlobUiPropertyField -InputObject $values -FieldName 'accentMode')
    $accentPreset = [string](Get-BlobUiPropertyField -InputObject $values -FieldName 'accentPreset')

    $fontValue = ConvertTo-BlobUiCssPlaceholderValue -Type font -Value $fontValue -Name 'Font'
    $codeFontValue = ConvertTo-BlobUiCssPlaceholderValue -Type font -Value $codeFontValue -Name 'Code Font'
    $accentValue = $accentValue.Trim()
    $normalizedAccent = ConvertTo-BlobUiAccentColorValue -Value $accentValue

    $presetValues = @{
        'grey' = 'Grey'
        'yellow' = 'Yellow'
        'amber' = 'Amber'
        'red' = 'Red'
        'pink' = 'Pink'
        'purple' = 'Purple'
        'blue' = 'Blue'
        'sky' = 'Sky'
        'teal' = 'Teal'
        'green' = 'Green'
        'blob-pink' = 'Blob Pink'
    }
    switch ($accentMode) {
        'native' {
            if ($accentValue -notin @('Default', 'var(--color-system-accent-100)') -or $accentPreset) {
                throw 'The native accent selection is invalid.'
            }
        }
        'preset' {
            if (-not $presetValues.ContainsKey($accentPreset) -or
                $accentValue -cne $presetValues[$accentPreset]) {
                throw 'The preset accent selection is invalid.'
            }
        }
        'custom' {
            if ($accentPreset) {
                throw 'A custom accent cannot include a preset key.'
            }
        }
        default {
            throw "The accent mode '$accentMode' is not supported."
        }
    }

    # Validate every value before touching the candidate so a malformed overlay
    # cannot leave a partially changed data object behind.
    if ($null -eq $Data.settings) {
        throw 'The candidate settings record has no normalized settings object.'
    }
    $Data.settings.fontFamily = $fontValue
    $Data.settings.codeFontFamily = $codeFontValue
    $Data.settings.accentColor = $normalizedAccent
    $settingsV2 = [ordered]@{
        version = 2
        settings = [ordered]@{
            fontFamily = $fontValue
            codeFontFamily = $codeFontValue
            accentColor = $accentValue
            accentMode = $accentMode
            accentPreset = $accentPreset
        }
    }
    if ($Data -is [System.Collections.IDictionary]) {
        $Data['settingsV2'] = $settingsV2
    }
    else {
        $null = $Data | Add-Member -MemberType NoteProperty -Name 'settingsV2' -Value $settingsV2 -Force
    }
    return $Data
}

function Update-BlobUiCalendarColorCache {
    param(
        [Parameter(Mandatory = $true)][string]$ApiKey,
        [Parameter(Mandatory = $true)]$Data,
        [string]$ApiBaseUri = $script:BlobUiApiBaseUri,
        [scriptblock]$ApiTransport
    )

    $spaces = @(Get-BlobUiPagedApiItems -ApiKey $ApiKey -Path '/v2/spaces?ids=full' -ApiBaseUri $ApiBaseUri -ApiTransport $ApiTransport)
    if ($spaces.Count -eq 0) {
        throw 'The API key cannot access any Anytype spaces.'
    }
    $spaceIds = @($spaces | ForEach-Object { [string](Get-BlobUiPropertyField -InputObject $_ -FieldName 'id') })
    $calendarObjectColors = Get-BlobUiCalendarObjectColors -ApiKey $ApiKey -SpaceIds $spaceIds -ApiBaseUri $ApiBaseUri -ApiTransport $ApiTransport
    if ($Data -is [System.Collections.IDictionary]) {
        $Data['calendarObjectColors'] = $calendarObjectColors
        $Data['fetchedAtUtc'] = [DateTime]::UtcNow.ToString('o')
    }
    else {
        $null = $Data | Add-Member -MemberType NoteProperty -Name 'calendarObjectColors' -Value $calendarObjectColors -Force
        $null = $Data | Add-Member -MemberType NoteProperty -Name 'fetchedAtUtc' -Value ([DateTime]::UtcNow.ToString('o')) -Force
    }
    return $Data
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

    $objectProperty = $Response.PSObject.Properties['object']
    if ($null -ne $objectProperty -and $null -ne $objectProperty.Value -and $objectProperty.Value -isnot [string]) {
        return $objectProperty.Value
    }

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
    $response = Invoke-BlobUiApiRequest -ApiKey $ApiKey -Path ("/v2/spaces/$escapedSpace/objects/${escapedObject}?include=properties&ids=full") -ApiBaseUri $ApiBaseUri -ApiTransport $ApiTransport
    return Get-BlobUiObjectFromApiResponse -Response $response
}

function Get-BlobUiNormalizedSettings {
    param([Parameter(Mandatory = $true)][object[]]$Objects)

    $rawSettings = @{
        font = $null
        code_font = $null
        accent_color = $null
    }
    foreach ($object in $Objects) {
        if ((Get-BlobUiObjectTypeKey -Object $object) -cne 'setting') {
            throw "Selected object '$($object.name)' is not of the exported Setting type."
        }
        $propertyMap = @(
            @{ key = 'font'; name = 'Font' },
            @{ key = 'code_font'; name = 'Code Font' },
            @{ key = 'accent_color'; name = 'Accent Color' }
        )
        foreach ($definition in $propertyMap) {
            $value = Get-BlobUiObjectPropertyValue -Object $object -Key $definition.key -DisplayName $definition.name
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
        accentColor = ConvertTo-BlobUiAccentColorValue -Value ([string]$accentValue)
        # Kept in the saved schema for compatibility. The banner is no longer an Anytype setting.
        bannerPath = $script:BlobUiDefaultSettings.bannerPath
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

function Get-BlobUiInstallPointerPath {
    param([Parameter(Mandatory = $true)][string]$SourceDirectory)

    return Join-Path ([System.IO.Path]::GetFullPath($SourceDirectory)) '.blobui-install.json'
}

function Write-BlobUiInstallPointer {
    param(
        [Parameter(Mandatory = $true)][string]$SourceDirectory,
        [Parameter(Mandatory = $true)][string]$InstallDirectory
    )

    $installFullPath = [System.IO.Path]::GetFullPath($InstallDirectory)
    Write-BlobUiAtomicJson -Path (Get-BlobUiInstallPointerPath -SourceDirectory $SourceDirectory) -InputObject ([ordered]@{
        installDirectory = $installFullPath
    })
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

    $spaces = @(Get-BlobUiPagedApiItems -ApiKey $ApiKey -Path '/v2/spaces?ids=full' -ApiBaseUri $ApiBaseUri -ApiTransport $ApiTransport)
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
        $settingsSearch = [ordered]@{ type = 'setting' }
        $settingsObjects = @(Get-BlobUiPagedApiItems -ApiKey $ApiKey -Path ("/v2/spaces/$escapedSpace/search") -Method Post -Body $settingsSearch -ApiBaseUri $ApiBaseUri -ApiTransport $ApiTransport)
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
    $calendarSpaceIds = @($spaces | ForEach-Object { [string](Get-BlobUiPropertyField -InputObject $_ -FieldName 'id') })
    $calendarObjectColors = Get-BlobUiCalendarObjectColors -ApiKey $ApiKey -SpaceIds $calendarSpaceIds -ApiBaseUri $ApiBaseUri -ApiTransport $ApiTransport
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
        calendarObjectColors = $calendarObjectColors
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

function New-BlobUiCalendarColorCss {
    param([AllowNull()]$CalendarObjectColors)

    if ($null -eq $CalendarObjectColors) { return '' }
    $entries = [System.Collections.Generic.List[object]]::new()
    if ($CalendarObjectColors -is [System.Collections.IDictionary]) {
        foreach ($objectId in $CalendarObjectColors.Keys) {
            $entries.Add([pscustomobject]@{ objectId = [string]$objectId; colorName = [string]$CalendarObjectColors[$objectId] })
        }
    }
    else {
        foreach ($property in $CalendarObjectColors.PSObject.Properties) {
            $entries.Add([pscustomobject]@{ objectId = [string]$property.Name; colorName = [string]$property.Value })
        }
    }

    $orderedEntries = @($entries.ToArray() | Sort-Object -Property objectId)
    $rules = [System.Collections.Generic.List[string]]::new()
    foreach ($entry in $orderedEntries) {
        if ($entry.objectId -notmatch '^[A-Za-z0-9][A-Za-z0-9._:-]{0,255}$') {
            throw "Calendar Color data contains an invalid Anytype object ID: $($entry.objectId)"
        }
        $colorName = ConvertTo-BlobUiColorValue -Value ([string]$entry.colorName) -Name 'Calendar Color' -AllowDefault
        if ($null -eq $colorName) { continue }
        $cssColor = ConvertTo-BlobUiCssPlaceholderValue -Type color -Value $colorName -Name 'Calendar Color'
        $rules.Add((
            '[data-view-type="4"][data-drop-type="record"][data-id="{0}"]:not(.record *),' -f $entry.objectId
        ))
        $rules.Add(('.viewContent.viewCalendar .record[id="record-{0}"] {{' -f $entry.objectId))
        $rules.Add(('    background-color: {0} !important;' -f $cssColor))
        $rules.Add(('    background-color: color-mix(in srgb, {0} 18%, #1b1b1b) !important;' -f $cssColor))
        $rules.Add(('    border-left: 3px solid {0} !important;' -f $cssColor))
        $rules.Add('    border-radius: 3px !important;')
        $rules.Add('    color: #FFFFFF !important;')
        $rules.Add('}')
        $rules.Add(('[data-view-type="4"][data-drop-type="record"][data-id="{0}"] .name,' -f $entry.objectId))
        $rules.Add(('[data-view-type="4"][data-drop-type="record"][data-id="{0}"] .name span,' -f $entry.objectId))
        $rules.Add(('.viewContent.viewCalendar .record[id="record-{0}"] .name,' -f $entry.objectId))
        $rules.Add(('.viewContent.viewCalendar .record[id="record-{0}"] .name span {{' -f $entry.objectId))
        $rules.Add('    color: #FFFFFF !important;')
        $rules.Add('}')

        $menuRowSelector = '.content > .wrap > .items > .item[id="item-{0}"]:has(> .iconObject[id="menuCalendarDay-{0}-icon"])' -f $entry.objectId
        $rules.Add($menuRowSelector + ' {')
        $rules.Add(('    background-color: {0} !important;' -f $cssColor))
        $rules.Add(('    background-color: color-mix(in srgb, {0} 18%, #1b1b1b) !important;' -f $cssColor))
        $rules.Add(('    border-left: 3px solid {0} !important;' -f $cssColor))
        $rules.Add('    border-radius: 3px !important;')
        $rules.Add('    color: #FFFFFF !important;')
        $rules.Add('}')
        $rules.Add(($menuRowSelector + ' .name,'))
        $rules.Add(($menuRowSelector + ' .name span {'))
        $rules.Add('    color: #FFFFFF !important;')
        $rules.Add('}')
    }

    if ($rules.Count -eq 0) { return '' }
    return "/* BlobUI Calendar Color overrides */`r`n" + [string]::Join("`r`n", $rules.ToArray())
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
    $bannerPath = $script:BlobUiDefaultSettings.bannerPath
    $bannerSource = [System.IO.Path]::GetFullPath((Join-Path $AssetSourceDirectory $bannerPath))
    $assetRoot = [System.IO.Path]::GetFullPath($AssetSourceDirectory).TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
    if (-not $bannerSource.StartsWith($assetRoot, [System.StringComparison]::OrdinalIgnoreCase) -or
        -not (Test-Path -LiteralPath $bannerSource -PathType Leaf)) {
        throw "The fixed banner image is missing from the packaged assets: $bannerPath"
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
            if ($name -ceq 'accentColor') {
                return ConvertTo-BlobUiAccentColorValue -Value ([string]$entry.value)
            }
            return ConvertTo-BlobUiCssPlaceholderValue -Type $entry.type -Value ([string]$entry.value) -Name $name
        }
    )
    $customAccentConditionPattern = '(?s)/\*\s*@blobui-if-custom-accent\s*\*/(?<content>.*?)/\*\s*@blobui-endif-custom-accent\s*\*/'
    $normalizedAccentColor = ConvertTo-BlobUiAccentColorValue -Value ([string]$Data.settings.accentColor)
    $hasCustomAccent = $normalizedAccentColor -cne 'var(--color-system-accent-100)'
    $compiled = [System.Text.RegularExpressions.Regex]::Replace(
        $compiled,
        $customAccentConditionPattern,
        [System.Text.RegularExpressions.MatchEvaluator]{
            param($match)
            if ($hasCustomAccent) { return $match.Groups['content'].Value }
            return ''
        }
    )
    if ($compiled -match '/\*\s*@blobui-(?:if-custom-accent|endif-custom-accent)\s*\*/') {
        throw 'CSS contains an unmatched custom accent condition.'
    }
    $declarationPattern = '/\*\s*@blobui-placeholder\s+(?<name>[A-Za-z][A-Za-z0-9]*)\s+(?<type>font|color|url|number|boolean)\s*\*/'
    $compiled = [System.Text.RegularExpressions.Regex]::Replace($compiled, $declarationPattern, '')
    $apiConnection = Get-BlobUiPropertyField -InputObject $Data -FieldName 'apiConnection'
    $calendarColors = if ($null -ne $apiConnection -and (Get-BlobUiPropertyField -InputObject $apiConnection -FieldName 'enabled') -eq $false) { @{} } else { $Data.calendarObjectColors }
    $calendarColorCss = New-BlobUiCalendarColorCss -CalendarObjectColors $calendarColors
    if (-not [string]::IsNullOrWhiteSpace($calendarColorCss)) {
        $compiled = $compiled.TrimEnd() + [Environment]::NewLine + [Environment]::NewLine + $calendarColorCss + [Environment]::NewLine
    }
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
    $runtimeScriptsDirectory = Join-Path $runtimeFullPath 'scripts'
    [System.IO.Directory]::CreateDirectory($runtimeScriptsDirectory) | Out-Null
    foreach ($scriptName in @('BlobUI.Core.psm1', 'Get-Data.ps1', 'Compile.ps1', 'Install.ps1', 'Rebuild.ps1', 'Update.ps1', 'Uninstall.ps1', 'Settings.ps1')) {
        $sourcePath = Join-Path (Join-Path $sourceFullPath 'scripts') $scriptName
        if (-not (Test-Path -LiteralPath $sourcePath -PathType Leaf)) {
            throw "BlobUI runtime script was not found: $scriptName"
        }
        Copy-Item -LiteralPath $sourcePath -Destination $runtimeScriptsDirectory -Force -ErrorAction Stop
    }
    Copy-BlobUiDirectoryContents -SourceDirectory (Join-Path $sourceFullPath 'Dev') -DestinationDirectory (Join-Path $runtimeFullPath 'Dev')
    Copy-BlobUiDirectoryContents -SourceDirectory (Join-Path $sourceFullPath 'assets') -DestinationDirectory (Join-Path $runtimeFullPath 'assets')
}

function ConvertTo-BlobUiManagedFileKey {
    param([Parameter(Mandatory = $true)][string]$Path)

    $key = $Path.Replace('\', '/').Trim('/')
    if ([string]::IsNullOrWhiteSpace($key) -or
        $key.Contains(':') -or
        $key.StartsWith('/') -or
        @($key.Split('/') | Where-Object { $_ -eq '' -or $_ -eq '.' -or $_ -eq '..' }).Count -gt 0) {
        throw "The BlobUI file manifest contains an unsafe path: $Path"
    }
    if ($key.Equals('assets/spacebanner/banner.png', [System.StringComparison]::OrdinalIgnoreCase)) {
        return $null
    }
    if ($key -notmatch '^(assets|runtime/(scripts|Dev|assets))/.+' -and
        $key -cne 'runtime/settings-app.exe') {
        throw "The BlobUI file manifest contains an unsupported path: $Path"
    }
    return $key
}

function Read-BlobUiInstallManifest {
    param([Parameter(Mandatory = $true)][string]$Path)

    if (-not (Test-Path -LiteralPath $Path)) {
        return [pscustomobject]@{
            exists = $false
            releaseVersion = $null
            managedFiles = @()
        }
    }
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "The BlobUI ownership manifest is not a file: $Path"
    }
    try {
        $manifest = ConvertFrom-Json -InputObject ([System.IO.File]::ReadAllText($Path, [System.Text.Encoding]::UTF8)) -ErrorAction Stop
    }
    catch {
        throw "The BlobUI ownership manifest is invalid: $Path"
    }
    if ([int]$manifest.manifestVersion -ne 1 -or $null -eq $manifest.PSObject.Properties['managedFiles']) {
        throw "The BlobUI ownership manifest has an unsupported format: $Path"
    }
    $managedFiles = [System.Collections.Generic.List[string]]::new()
    $seen = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
    foreach ($entry in @($manifest.managedFiles)) {
        $key = ConvertTo-BlobUiManagedFileKey -Path ([string]$entry)
        if ($null -ne $key -and $seen.Add($key)) {
            $managedFiles.Add($key)
        }
    }
    return [pscustomobject]@{
        exists = $true
        releaseVersion = [string]$manifest.releaseVersion
        managedFiles = $managedFiles.ToArray()
    }
}

function Get-BlobUiPackagedFiles {
    param(
        [Parameter(Mandatory = $true)][string]$RuntimeSourceDirectory,
        [Parameter(Mandatory = $true)][string]$AssetsSourceDirectory
    )

    $runtimeSource = [System.IO.Path]::GetFullPath($RuntimeSourceDirectory)
    $assetSource = [System.IO.Path]::GetFullPath($AssetsSourceDirectory)
    if (-not (Test-Path -LiteralPath $runtimeSource -PathType Container)) {
        throw "BlobUI runtime source was not found: $runtimeSource"
    }
    if (-not (Test-Path -LiteralPath $assetSource -PathType Container)) {
        throw "BlobUI asset source was not found: $assetSource"
    }

    $files = [ordered]@{}
    foreach ($scriptName in @('BlobUI.Core.psm1', 'Get-Data.ps1', 'Compile.ps1', 'Install.ps1', 'Rebuild.ps1', 'Update.ps1', 'Uninstall.ps1', 'Settings.ps1')) {
        $sourcePath = Join-Path (Join-Path $runtimeSource 'scripts') $scriptName
        if (-not (Test-Path -LiteralPath $sourcePath -PathType Leaf)) {
            throw "BlobUI runtime script was not found: $scriptName"
        }
        $files['runtime/scripts/' + $scriptName] = [System.IO.Path]::GetFullPath($sourcePath)
    }

    foreach ($tree in @(
        [pscustomobject]@{ name = 'Dev'; path = (Join-Path $runtimeSource 'Dev') },
        [pscustomobject]@{ name = 'assets'; path = (Join-Path $runtimeSource 'assets') }
    )) {
        if (-not (Test-Path -LiteralPath $tree.path -PathType Container)) {
            throw "BlobUI runtime content was not found: $($tree.path)"
        }
        $treeRoot = [System.IO.Path]::GetFullPath($tree.path).TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
        $treeFiles = @(Get-ChildItem -LiteralPath $tree.path -File -Force -Recurse | Where-Object {
            ($_.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -eq 0
        })
        if ($treeFiles.Count -eq 0) {
            throw "BlobUI runtime content is empty: $($tree.path)"
        }
        foreach ($file in $treeFiles) {
            $relative = $file.FullName.Substring($treeRoot.Length).Replace('\', '/')
            $files['runtime/' + $tree.name + '/' + $relative] = [System.IO.Path]::GetFullPath($file.FullName)
        }
    }

    $settingsAppPath = Join-Path $runtimeSource 'settings-app\dist\blobui-settings.exe'
    if (Test-Path -LiteralPath $settingsAppPath -PathType Leaf) {
        $settingsAppItem = Get-Item -LiteralPath $settingsAppPath -Force -ErrorAction Stop
        if (($settingsAppItem.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "BlobUI settings app is a reparse point: $settingsAppPath"
        }
        $files['runtime/settings-app.exe'] = [System.IO.Path]::GetFullPath($settingsAppPath)
    }

    $assetRoot = $assetSource.TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
    $assetFiles = @(Get-ChildItem -LiteralPath $assetSource -File -Force -Recurse | Where-Object {
        ($_.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -eq 0
    })
    if ($assetFiles.Count -eq 0) {
        throw "BlobUI asset source is empty: $assetSource"
    }
    foreach ($file in $assetFiles) {
        $relative = $file.FullName.Substring($assetRoot.Length).Replace('\', '/')
        $key = 'assets/' + $relative
        if (-not $key.Equals('assets/spacebanner/banner.png', [System.StringComparison]::OrdinalIgnoreCase)) {
            $files[$key] = [System.IO.Path]::GetFullPath($file.FullName)
        }
    }
    return $files
}

function Get-BlobUiManagedDestinationPath {
    param(
        [Parameter(Mandatory = $true)][string]$BlobDirectory,
        [Parameter(Mandatory = $true)][string]$ManagedPath,
        [string[]]$AllowedManagedFileParents = @()
    )

    $key = ConvertTo-BlobUiManagedFileKey -Path $ManagedPath
    if ($null -eq $key) {
        throw 'The installed custom banner is user data and cannot be treated as an owned package file.'
    }
    $blobRoot = [System.IO.Path]::GetFullPath($BlobDirectory)
    $rootWithSeparator = $blobRoot.TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
    $destination = [System.IO.Path]::GetFullPath((Join-Path $blobRoot ($key.Replace('/', [System.IO.Path]::DirectorySeparatorChar))))
    if (-not $destination.StartsWith($rootWithSeparator, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "The BlobUI managed file path leaves the Anytype blob directory: $ManagedPath"
    }

    $blobItem = Get-Item -LiteralPath $blobRoot -Force -ErrorAction SilentlyContinue
    if ($null -ne $blobItem -and (($blobItem.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0)) {
        throw "The Anytype blob directory is a reparse point: $blobRoot"
    }
    $relativeParent = [System.IO.Path]::GetDirectoryName($key.Replace('/', [System.IO.Path]::DirectorySeparatorChar))
    $current = $blobRoot
    $allowedParentSet = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
    foreach ($allowedParent in $AllowedManagedFileParents) {
        $null = $allowedParentSet.Add([System.IO.Path]::GetFullPath($allowedParent))
    }
    if (-not [string]::IsNullOrWhiteSpace($relativeParent)) {
        foreach ($part in $relativeParent.Split([System.IO.Path]::DirectorySeparatorChar)) {
            $current = Join-Path $current $part
            if (Test-Path -LiteralPath $current) {
                $item = Get-Item -LiteralPath $current -Force -ErrorAction Stop
                if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
                    throw "A BlobUI managed file parent is a reparse point: $current"
                }
                if (-not $item.PSIsContainer) {
                    if (-not $allowedParentSet.Contains([System.IO.Path]::GetFullPath($current))) {
                        throw "A BlobUI managed file parent is not a directory: $current"
                    }
                }
            }
        }
    }
    return $destination
}

function Get-BlobUiFileSnapshot {
    param([Parameter(Mandatory = $true)][string]$Path)

    $item = Get-Item -LiteralPath $Path -Force -ErrorAction SilentlyContinue
    if ($null -ne $item) {
        if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "BlobUI will not replace a reparse point: $Path"
        }
        if ($item.PSIsContainer) {
            return [pscustomobject]@{
                path = $Path
                kind = 'Directory'
                bytes = $null
                acl = $null
                attributes = $item.Attributes
            }
        }
        return [pscustomobject]@{
            path = $Path
            kind = 'File'
            bytes = [System.IO.File]::ReadAllBytes($Path)
            acl = (Get-Acl -LiteralPath $Path -ErrorAction Stop)
            attributes = $item.Attributes
        }
    }
    return [pscustomobject]@{
        path = $Path
        kind = 'Missing'
        bytes = $null
        acl = $null
        attributes = $null
    }
}

function Restore-BlobUiFileSnapshot {
    param([Parameter(Mandatory = $true)]$Snapshot)

    switch ($Snapshot.kind) {
        'File' {
            $parent = Split-Path -Parent $Snapshot.path
            [System.IO.Directory]::CreateDirectory($parent) | Out-Null
            if (Test-Path -LiteralPath $Snapshot.path -PathType Container) {
                throw "Rollback found a directory where a file was saved: $($Snapshot.path)"
            }
            [System.IO.File]::WriteAllBytes($Snapshot.path, [byte[]]$Snapshot.bytes)
            Set-Acl -LiteralPath $Snapshot.path -AclObject $Snapshot.acl -ErrorAction Stop
            [System.IO.File]::SetAttributes($Snapshot.path, [System.IO.FileAttributes]$Snapshot.attributes)
        }
        'Missing' {
            if (Test-Path -LiteralPath $Snapshot.path -PathType Leaf) {
                Remove-Item -LiteralPath $Snapshot.path -Force -ErrorAction Stop
            }
            elseif (Test-Path -LiteralPath $Snapshot.path -PathType Container) {
                if ([System.IO.Directory]::GetFileSystemEntries($Snapshot.path).Length -eq 0) {
                    Remove-Item -LiteralPath $Snapshot.path -Force -ErrorAction Stop
                }
            }
        }
        'Directory' {
            if (-not (Test-Path -LiteralPath $Snapshot.path -PathType Container)) {
                throw "Rollback could not restore the saved directory: $($Snapshot.path)"
            }
        }
    }
}

function Restore-BlobUiFileSnapshots {
    param([Parameter(Mandatory = $true)][System.Collections.Generic.List[object]]$Snapshots)

    $failures = [System.Collections.Generic.List[string]]::new()
    # Remove files created at paths that used to be directories or missing, so
    # prior file trees and file-to-directory transitions can be restored.
    foreach ($snapshot in $Snapshots) {
        try {
            if ($snapshot.kind -ne 'File' -and (Test-Path -LiteralPath $snapshot.path -PathType Leaf)) {
                Remove-Item -LiteralPath $snapshot.path -Force -ErrorAction Stop
            }
        }
        catch {
            $failures.Add($_.Exception.Message)
        }
    }
    # Restore directories before files so their saved children have parents.
    foreach ($snapshot in @($Snapshots | Where-Object { $_.kind -eq 'Directory' } | Sort-Object { $_.path.Length })) {
        try {
            if (Test-Path -LiteralPath $snapshot.path -PathType Leaf) {
                Remove-Item -LiteralPath $snapshot.path -Force -ErrorAction Stop
            }
            [System.IO.Directory]::CreateDirectory($snapshot.path) | Out-Null
        }
        catch {
            $failures.Add($_.Exception.Message)
        }
    }
    # Missing snapshots are ordered deepest-first so only directories created
    # during this install are removed, and only when still empty.
    foreach ($snapshot in @($Snapshots | Where-Object { $_.kind -eq 'Missing' } | Sort-Object { $_.path.Length } -Descending)) {
        try {
            Restore-BlobUiFileSnapshot -Snapshot $snapshot
        }
        catch {
            $failures.Add($_.Exception.Message)
        }
    }
    foreach ($snapshot in @($Snapshots | Where-Object { $_.kind -eq 'File' } | Sort-Object { $_.path.Length })) {
        try {
            if (Test-Path -LiteralPath $snapshot.path -PathType Container) {
                Remove-Item -LiteralPath $snapshot.path -Force -ErrorAction Stop
            }
            Restore-BlobUiFileSnapshot -Snapshot $snapshot
        }
        catch {
            $failures.Add($_.Exception.Message)
        }
    }
    if ($failures.Count -gt 0) {
        throw [string]::Join('; ', $failures)
    }
}

function Remove-BlobUiEmptyManagedDirectories {
    param(
        [Parameter(Mandatory = $true)][string]$BlobDirectory,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$ManagedPaths
    )

    $blobRoot = [System.IO.Path]::GetFullPath($BlobDirectory).TrimEnd([System.IO.Path]::DirectorySeparatorChar)
    $candidates = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
    foreach ($managedPath in $ManagedPaths) {
        $current = [System.IO.Path]::GetFullPath($managedPath)
        while ($current.StartsWith($blobRoot + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
            $item = Get-Item -LiteralPath $current -Force -ErrorAction SilentlyContinue
            if ($null -ne $item -and $item.PSIsContainer -and
                (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -eq 0)) {
                $null = $candidates.Add($current)
            }
            $current = Split-Path -Parent $current
        }
    }
    foreach ($directory in @($candidates | Sort-Object { $_.Length } -Descending)) {
        if ([System.IO.Directory]::GetFileSystemEntries($directory).Length -eq 0) {
            [System.IO.Directory]::Delete($directory)
        }
    }
}

function Invoke-BlobUiLocalDataMigrations {
    param(
        [Parameter(Mandatory = $true)]$Data,
        [string]$FromReleaseVersion,
        [string]$ToReleaseVersion = $script:BlobUiReleaseVersion
    )

    return $Data
}

function Get-BlobUiReleaseVersion {
    return $script:BlobUiReleaseVersion
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
        [string]$CredentialProtectedText,
        [string]$ReleaseVersion = $script:BlobUiReleaseVersion
    )

    $installFullPath = [System.IO.Path]::GetFullPath($InstallDirectory)
    if (-not (Test-Path -LiteralPath $installFullPath -PathType Container)) {
        throw "The selected Anytype directory does not exist: $installFullPath"
    }
    if ($ReleaseVersion -notmatch '^\d+\.\d+\.\d+(?:[-+][A-Za-z0-9.-]+)?$') {
        throw "The BlobUI release version is invalid: $ReleaseVersion"
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
    $bannerPath = $script:BlobUiDefaultSettings.bannerPath
    $bannerSource = [System.IO.Path]::GetFullPath((Join-Path $AssetsSourceDirectory $bannerPath))
    $assetRoot = [System.IO.Path]::GetFullPath($AssetsSourceDirectory).TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
    if (-not $bannerSource.StartsWith($assetRoot, [System.StringComparison]::OrdinalIgnoreCase) -or
        -not (Test-Path -LiteralPath $bannerSource -PathType Leaf)) {
        throw "The fixed banner image is missing from the packaged assets: $bannerPath"
    }

    $blobDirectory = Join-Path $installFullPath 'blob'
    $assetDestination = Join-Path $blobDirectory 'assets'
    $runtimeDestination = Join-Path $blobDirectory 'runtime'
    $credentialDirectory = Join-Path $blobDirectory 'credentials'
    $credentialPath = Join-Path $credentialDirectory 'anytype-api-key.dpapi'
    $dataPath = Join-Path $blobDirectory 'data.json'
    $manifestPath = Join-Path $blobDirectory '.blobui-managed-files.json'
    $cssDestination = Join-Path $installFullPath 'custom.css'
    $blobItem = Get-Item -LiteralPath $blobDirectory -Force -ErrorAction SilentlyContinue
    if ($null -ne $blobItem -and (($blobItem.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0)) {
        throw "The Anytype blob directory is a reparse point: $blobDirectory"
    }
    $null = Get-BlobUiFileSnapshot -Path $manifestPath
    $previousManifest = Read-BlobUiInstallManifest -Path $manifestPath
    $packagedFiles = Get-BlobUiPackagedFiles -RuntimeSourceDirectory $RuntimeSourceDirectory -AssetsSourceDirectory $AssetsSourceDirectory
    $runtimeSourcePath = [System.IO.Path]::GetFullPath($RuntimeSourceDirectory).TrimEnd([System.IO.Path]::DirectorySeparatorChar)
    $runtimeDestinationPath = [System.IO.Path]::GetFullPath($runtimeDestination).TrimEnd([System.IO.Path]::DirectorySeparatorChar)
    $managedFiles = @($packagedFiles.Keys | Where-Object {
        -not ([string]$_).Equals('assets/spacebanner/banner.png', [System.StringComparison]::OrdinalIgnoreCase)
    } | Sort-Object -Unique)
    # Keep a previously managed app binary when a source package is rebuilt
    # without the optional prebuilt executable. A release that includes a newer
    # binary replaces it explicitly through the normal copy operation.
    $settingsAppManagedKey = 'runtime/settings-app.exe'
    $previousOwnsSettingsApp = @($previousManifest.managedFiles | Where-Object {
        [string]$_ -ceq $settingsAppManagedKey
    }).Count -gt 0
    $settingsAppDestination = Join-Path $runtimeDestination 'settings-app.exe'
    if (-not $packagedFiles.Contains($settingsAppManagedKey) -and $previousOwnsSettingsApp -and
        (Test-Path -LiteralPath $settingsAppDestination -PathType Leaf)) {
        $managedFiles = @($managedFiles + $settingsAppManagedKey | Sort-Object -Unique)
    }
    if ($runtimeSourcePath.Equals($runtimeDestinationPath, [System.StringComparison]::OrdinalIgnoreCase)) {
        $managedFiles = @($previousManifest.managedFiles | Sort-Object -Unique)
        $ownedFiles = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
        foreach ($managedPath in $managedFiles) {
            $null = $ownedFiles.Add([string]$managedPath)
        }
        foreach ($packagedPath in @($packagedFiles.Keys)) {
            if (-not $ownedFiles.Contains([string]$packagedPath)) {
                $packagedFiles.Remove($packagedPath)
            }
        }
    }
    $manifestBannerPath = Join-Path (Join-Path $assetDestination 'spacebanner') 'banner.png'
    foreach ($bannerDirectory in @($assetDestination, (Split-Path -Parent $manifestBannerPath))) {
        if (Test-Path -LiteralPath $bannerDirectory) {
            $bannerDirectoryItem = Get-Item -LiteralPath $bannerDirectory -Force -ErrorAction Stop
            if (-not $bannerDirectoryItem.PSIsContainer -or
                (($bannerDirectoryItem.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0)) {
                throw "The installed BlobUI banner directory cannot be written safely: $bannerDirectory"
            }
        }
    }
    $installDefaultBanner = -not (Test-Path -LiteralPath $manifestBannerPath -PathType Leaf)
    if (Test-Path -LiteralPath $manifestBannerPath -PathType Container) {
        throw "The installed BlobUI banner path is a directory: $manifestBannerPath"
    }
    if ($installDefaultBanner) {
        if (Test-Path -LiteralPath $manifestBannerPath) {
            throw "The installed BlobUI banner path is not a file: $manifestBannerPath"
        }
    }

    $managedFileSet = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
    foreach ($managedPath in $managedFiles) {
        $null = $managedFileSet.Add([string]$managedPath)
    }
    $stalePaths = [System.Collections.Generic.List[string]]::new()
    foreach ($previousPath in @($previousManifest.managedFiles)) {
        if (-not $managedFileSet.Contains([string]$previousPath)) {
            $stalePaths.Add([string]$previousPath)
        }
    }

    $copyOperations = [System.Collections.Generic.List[object]]::new()
    $staleDestinations = [System.Collections.Generic.List[string]]::new()
    foreach ($stalePath in $stalePaths) {
        $staleDestinations.Add((Get-BlobUiManagedDestinationPath -BlobDirectory $blobDirectory -ManagedPath $stalePath))
    }
    foreach ($managedPath in $packagedFiles.Keys) {
        $sourcePath = [System.IO.Path]::GetFullPath([string]$packagedFiles[$managedPath])
        $destinationPath = Get-BlobUiManagedDestinationPath -BlobDirectory $blobDirectory -ManagedPath ([string]$managedPath) -AllowedManagedFileParents $staleDestinations.ToArray()
        $samePath = $sourcePath.Equals($destinationPath, [System.StringComparison]::OrdinalIgnoreCase)
        $sameContents = $false
        if (-not $samePath -and (Test-Path -LiteralPath $destinationPath -PathType Leaf)) {
            $sourceItem = Get-Item -LiteralPath $sourcePath -Force -ErrorAction Stop
            $destinationItem = Get-Item -LiteralPath $destinationPath -Force -ErrorAction Stop
            if ($sourceItem.Length -eq $destinationItem.Length) {
                $sameContents = (Get-FileHash -LiteralPath $sourcePath -Algorithm SHA256).Hash -ceq (Get-FileHash -LiteralPath $destinationPath -Algorithm SHA256).Hash
            }
        }
        if (-not $samePath -and -not $sameContents) {
            $copyOperations.Add([pscustomobject]@{
                source = $sourcePath
                destination = $destinationPath
            })
        }
    }

    $compiledCss = [System.IO.File]::ReadAllText([System.IO.Path]::GetFullPath($CompiledCssPath), [System.Text.Encoding]::UTF8)
    $candidateDataText = [System.IO.File]::ReadAllText([System.IO.Path]::GetFullPath($DataCandidatePath), [System.Text.Encoding]::UTF8)
    $manifestText = ConvertTo-Json -InputObject ([ordered]@{
        manifestVersion = 1
        releaseVersion = $ReleaseVersion
        managedFiles = @($managedFiles)
    }) -Depth 10
    $manifestText += [Environment]::NewLine

    $snapshotPaths = [System.Collections.Generic.List[string]]::new()
    $snapshotPathSet = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
    foreach ($operation in $copyOperations) {
        if ($snapshotPathSet.Add([string]$operation.destination)) {
            $snapshotPaths.Add([string]$operation.destination)
        }
    }
    if ($installDefaultBanner) {
        if ($snapshotPathSet.Add([string]$manifestBannerPath)) {
            $snapshotPaths.Add([string]$manifestBannerPath)
        }
    }
    foreach ($staleDestination in $staleDestinations) {
        if ($snapshotPathSet.Add([string]$staleDestination)) {
            $snapshotPaths.Add([string]$staleDestination)
        }
    }
    foreach ($fixedPath in @($cssDestination, $dataPath, $manifestPath)) {
        if ($snapshotPathSet.Add([string]$fixedPath)) {
            $snapshotPaths.Add([string]$fixedPath)
        }
    }
    if (-not [string]::IsNullOrWhiteSpace($CredentialProtectedText) -and $snapshotPathSet.Add([string]$credentialPath)) {
        $snapshotPaths.Add([string]$credentialPath)
    }
    $managedMutationPaths = [System.Collections.Generic.List[string]]::new()
    foreach ($operation in $copyOperations) { $managedMutationPaths.Add([string]$operation.destination) }
    foreach ($staleDestination in $staleDestinations) { $managedMutationPaths.Add([string]$staleDestination) }
    foreach ($managedPath in $managedMutationPaths) {
        $current = Split-Path -Parent $managedPath
        while ($current.StartsWith([System.IO.Path]::GetFullPath($blobDirectory).TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
            if ($snapshotPathSet.Add($current)) { $snapshotPaths.Add($current) }
            $current = Split-Path -Parent $current
        }
    }
    $snapshots = [System.Collections.Generic.List[object]]::new()
    foreach ($snapshotPath in $snapshotPaths) {
        $snapshots.Add((Get-BlobUiFileSnapshot -Path $snapshotPath))
    }

    try {
        [System.IO.Directory]::CreateDirectory($blobDirectory) | Out-Null
        foreach ($staleDestination in $staleDestinations) {
            if (Test-Path -LiteralPath $staleDestination -PathType Leaf) {
                Remove-Item -LiteralPath $staleDestination -Force -ErrorAction Stop
            }
        }
        Remove-BlobUiEmptyManagedDirectories -BlobDirectory $blobDirectory -ManagedPaths $managedMutationPaths.ToArray()
        foreach ($operation in $copyOperations) {
            if (Test-Path -LiteralPath $operation.destination -PathType Container) {
                throw "A BlobUI managed file destination is an unowned or non-empty directory: $($operation.destination)"
            }
            $parent = Split-Path -Parent $operation.destination
            [System.IO.Directory]::CreateDirectory($parent) | Out-Null
            [System.IO.File]::WriteAllBytes($operation.destination, [System.IO.File]::ReadAllBytes($operation.source))
        }
        if ($installDefaultBanner) {
            $bannerParent = Split-Path -Parent $manifestBannerPath
            [System.IO.Directory]::CreateDirectory($bannerParent) | Out-Null
            [System.IO.File]::WriteAllBytes($manifestBannerPath, [System.IO.File]::ReadAllBytes($bannerSource))
        }
        Write-BlobUiAtomicText -Path $cssDestination -Content $compiledCss
        Write-BlobUiAtomicText -Path $dataPath -Content $candidateDataText
        if (-not [string]::IsNullOrWhiteSpace($CredentialProtectedText)) {
            [System.IO.Directory]::CreateDirectory($credentialDirectory) | Out-Null
            Write-BlobUiAtomicText -Path $credentialPath -Content ($CredentialProtectedText + [Environment]::NewLine)
            Set-BlobUiCredentialAcl -Path $credentialPath
        }
        Write-BlobUiAtomicText -Path $manifestPath -Content $manifestText
    }
    catch {
        $installFailure = $_.Exception.Message
        $rollbackFailures = [System.Collections.Generic.List[string]]::new()
        try {
            Restore-BlobUiFileSnapshots -Snapshots $snapshots
        }
        catch {
            $rollbackFailures.Add($_.Exception.Message)
        }
        if ($rollbackFailures.Count -gt 0) {
            throw "Installation failed and rollback could not restore all saved files. $installFailure Rollback errors: $([string]::Join('; ', $rollbackFailures))"
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
    'Set-BlobUiSettingsV2Overlay',
    'Update-BlobUiCalendarColorCache',
    'Get-BlobUiInstallPointerPath',
    'Get-BlobUiReleaseVersion',
    'Invoke-BlobUiInstall',
    'Invoke-BlobUiLocalDataMigrations',
    'New-BlobUiProtectedCredential',
    'Read-BlobUiDataFile',
    'Read-BlobUiProtectedCredential',
    'Write-BlobUiAtomicJson',
    'Write-BlobUiInstallPointer',
    'Write-BlobUiAtomicText'
)
