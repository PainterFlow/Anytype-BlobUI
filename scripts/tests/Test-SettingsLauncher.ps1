$ErrorActionPreference = 'Stop'
$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$fixtureRoot = Join-Path $repoRoot '.test-run\launcher-validation'
$fixtureScripts = Join-Path $fixtureRoot 'scripts'
$fixtureDist = Join-Path $fixtureRoot 'settings-app\dist'
[IO.Directory]::CreateDirectory($fixtureScripts) | Out-Null
[IO.Directory]::CreateDirectory($fixtureDist) | Out-Null
Copy-Item -LiteralPath (Join-Path $repoRoot 'scripts\Settings.ps1') -Destination $fixtureScripts -Force
[IO.File]::WriteAllText((Join-Path $fixtureDist 'blobui-settings.exe'), 'Not an executable. Start-Process is mocked by this test.')
$launcher = Join-Path $fixtureScripts 'Settings.ps1'
$oldPath = [Environment]::GetEnvironmentVariable('BLOBUI_INSTALL_DIRECTORY', 'Process')
$launchCalls = [Collections.Generic.List[object]]::new()

# Capture the inherited environment without opening any app or launching a process.
$startProcessMock = {
    param([string]$FilePath, [string]$WindowStyle, [string[]]$ArgumentList)
    $launchCalls.Add([pscustomobject]@{
        file = $FilePath
        installDirectory = [Environment]::GetEnvironmentVariable('BLOBUI_INSTALL_DIRECTORY', 'Process')
        arguments = $ArgumentList
        windowStyle = $WindowStyle
    })
}.GetNewClosure()
Set-Item -Path Function:\Start-Process -Value $startProcessMock

try {
    [Environment]::SetEnvironmentVariable('BLOBUI_INSTALL_DIRECTORY', [NullString]::Value, 'Process')
    & $launcher
    & $launcher
    if ($launchCalls.Count -ne 2 -or
        $null -ne $launchCalls[0].installDirectory -or
        $null -ne $launchCalls[1].installDirectory -or
        [Environment]::GetEnvironmentVariables('Process').Contains('BLOBUI_INSTALL_DIRECTORY')) {
        throw 'Repeated default launches introduced an empty install override.'
    }
    $customInstall = Join-Path $fixtureRoot 'Custom Anytype'
    & $launcher -InstallDirectory $customInstall
    if ($launchCalls[2].installDirectory -cne $customInstall -or
        [Environment]::GetEnvironmentVariables('Process').Contains('BLOBUI_INSTALL_DIRECTORY')) {
        throw 'The custom directory was not forwarded or the absent environment variable was not restored.'
    }
    [Environment]::SetEnvironmentVariable('BLOBUI_INSTALL_DIRECTORY', $customInstall, 'Process')
    & $launcher -InstallDirectory (Join-Path $fixtureRoot 'Other Anytype')
    if ([Environment]::GetEnvironmentVariable('BLOBUI_INSTALL_DIRECTORY', 'Process') -cne $customInstall) {
        throw 'The launcher did not preserve a pre-existing install override.'
    }
    & $launcher -Background
    if ($launchCalls[4].arguments -cne '--background' -or $launchCalls[4].windowStyle -cne 'Hidden') {
        throw 'The background launcher did not forward its background flag.'
    }
    Write-Output 'Launcher environment checks passed. No app was launched.'
}
finally {
    $restoreValue = if ($null -eq $oldPath) { [NullString]::Value } else { $oldPath }
    [Environment]::SetEnvironmentVariable('BLOBUI_INSTALL_DIRECTORY', $restoreValue, 'Process')
    Remove-Item -LiteralPath Function:\Start-Process
}
