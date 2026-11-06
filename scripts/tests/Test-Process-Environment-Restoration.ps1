$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '..\Restore-ProcessEnvironment.ps1')

$suffix = [Guid]::NewGuid().ToString('N')
$unsetName = "BLOBUI_RESTORE_UNSET_$suffix"
$emptyName = "BLOBUI_RESTORE_EMPTY_$suffix"
$valueName = "BLOBUI_RESTORE_VALUE_$suffix"
$names = @($unsetName, $emptyName, $valueName)
$originalEnvironment = @{}
foreach ($name in $names) {
    $originalEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}

try {
    Remove-Item -LiteralPath "Env:$unsetName" -ErrorAction SilentlyContinue
    Set-Item -LiteralPath "Env:$emptyName" -Value ''
    Set-Item -LiteralPath "Env:$valueName" -Value 'before-build'

    $savedEnvironment = @{}
    foreach ($name in $names) {
        $savedEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
        Set-Item -LiteralPath "Env:$name" -Value 'during-build'
    }

    Restore-ProcessEnvironment -SavedEnvironment $savedEnvironment

    if (Test-Path -LiteralPath "Env:$unsetName") {
        throw 'An environment variable that was absent before the build was restored as present.'
    }
    if (-not (Test-Path -LiteralPath "Env:$emptyName") -or
        [Environment]::GetEnvironmentVariable($emptyName, 'Process') -cne '') {
        throw 'An environment variable that was empty before the build was not restored as empty.'
    }
    if ([Environment]::GetEnvironmentVariable($valueName, 'Process') -cne 'before-build') {
        throw 'A non-empty environment variable was not restored to its previous value.'
    }

    Write-Host 'Process environment restoration checks passed.'
}
finally {
    Restore-ProcessEnvironment -SavedEnvironment $originalEnvironment
}
