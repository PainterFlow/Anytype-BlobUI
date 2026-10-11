$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'BlobUI.Core.psm1') -Force

function Get-InstallDirectory {
    $defaultDirectory = $null
    if (-not [string]::IsNullOrWhiteSpace($env:APPDATA)) {
        $defaultDirectory = Join-Path $env:APPDATA 'anytype'
    }

    while ($true) {
        if ($defaultDirectory) {
            Write-Host "A) Automatic (default): $defaultDirectory"
        }
        else {
            Write-Host 'A) Automatic (default): unavailable because APPDATA is not set'
        }
        Write-Host 'M) Enter a directory manually'

        $selection = (Read-Host 'Install location [A/M] (default A)').Trim().ToUpperInvariant()
        if ([string]::IsNullOrWhiteSpace($selection)) {
            $selection = 'A'
        }

        switch ($selection) {
            'A' {
                if (-not $defaultDirectory) {
                    Write-Host 'Automatic detection needs APPDATA. Select M to enter the Anytype directory.'
                    continue
                }
                if (Test-Path -LiteralPath $defaultDirectory -PathType Container) {
                    return $defaultDirectory
                }
                Write-Host "The detected directory does not exist: $defaultDirectory"
                Write-Host 'Select M to enter the Anytype directory manually.'
            }
            'M' {
                $manualDirectory = (Read-Host 'Enter the full Anytype data directory path').Trim().Trim('"')
                if ([string]::IsNullOrWhiteSpace($manualDirectory)) {
                    throw 'No directory was entered.'
                }
                return $manualDirectory
            }
            default {
                Write-Host 'Choose A or M.'
            }
        }
    }
}

Write-Host 'Anytype Instillation'
$secureApiKey = $null
$apiKeyPointer = [IntPtr]::Zero
$apiKey = $null
$runDirectory = $null
$stagingDirectory = $null
$blobDirectory = $null
$blobDirectoryExisted = $false
try {
    $installDirectory = [System.IO.Path]::GetFullPath((Get-InstallDirectory))
    if (-not (Test-Path -LiteralPath $installDirectory -PathType Container)) {
        throw "The selected Anytype directory does not exist: $installDirectory"
    }

    $secureApiKey = Read-Host 'Enter your existing Anytype local API key' -AsSecureString
    if ($null -eq $secureApiKey -or $secureApiKey.Length -eq 0) {
        throw 'An Anytype API key is required.'
    }
    $apiKeyPointer = [System.Runtime.InteropServices.Marshal]::SecureStringToBSTR($secureApiKey)
    $apiKey = [System.Runtime.InteropServices.Marshal]::PtrToStringBSTR($apiKeyPointer)
    $protectedCredential = New-BlobUiProtectedCredential -ApiKey $apiKey

    $blobDirectory = Join-Path $installDirectory 'blob'
    $blobDirectoryExisted = Test-Path -LiteralPath $blobDirectory -PathType Container
    $stagingDirectory = Join-Path $blobDirectory 'staging'
    $runDirectory = Join-Path $stagingDirectory ([System.Guid]::NewGuid().ToString('N'))
    [System.IO.Directory]::CreateDirectory($runDirectory) | Out-Null

    $candidateDataPath = Join-Path $runDirectory 'data.json'
    $compiledCssPath = Join-Path $runDirectory 'custom.css'
    $getDataArguments = @{
        ApiKey = $apiKey
        OutputPath = $candidateDataPath
        InstallDirectory = $installDirectory
    }
    & (Join-Path $PSScriptRoot 'Get-Data.ps1') @getDataArguments

    $compileArguments = @{
        DataPath = $candidateDataPath
        OutputPath = $compiledCssPath
        DevDirectory = Join-Path $PSScriptRoot 'Dev'
        AssetSourceDirectory = Join-Path $PSScriptRoot 'assets'
    }
    & (Join-Path $PSScriptRoot 'Compile.ps1') @compileArguments

    $installArguments = @{
        InstallDirectory = $installDirectory
        RuntimeSourceDirectory = $PSScriptRoot
        AssetsSourceDirectory = Join-Path $PSScriptRoot 'assets'
        CompiledCssPath = $compiledCssPath
        DataCandidatePath = $candidateDataPath
        CredentialProtectedText = $protectedCredential
    }
    $result = Invoke-BlobUiInstall @installArguments

    $installedBlobDirectory = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
    $sourceInstallPointerPath = Get-BlobUiInstallPointerPath -SourceDirectory $PSScriptRoot
    if (-not (Test-Path -LiteralPath (Join-Path $installedBlobDirectory 'data.json') -PathType Leaf)) {
        Write-BlobUiInstallPointer -SourceDirectory $PSScriptRoot -InstallDirectory $installDirectory
        Write-Host "Source rebuild location saved: $sourceInstallPointerPath"
    }

    Write-Host "Installed CSS: $($result.cssPath)"
    Write-Host "Installed assets: $($result.assetsPath)"
    Write-Host 'Installation complete.'
    Write-Host 'Switch to Anytype and press Ctrl+R to reload the stylesheet. If custom CSS is not active, select Menu > File > Apply Custom CSS first.'
}
catch {
    Write-Host "Installation failed: $($_.Exception.Message)" -ForegroundColor Red
    throw
}
finally {
    $apiKey = $null
    $protectedCredential = $null
    if ($apiKeyPointer -ne [IntPtr]::Zero) {
        [System.Runtime.InteropServices.Marshal]::ZeroFreeBSTR($apiKeyPointer)
    }
    if ($null -ne $secureApiKey) {
        $secureApiKey.Dispose()
    }
    if ($null -ne $runDirectory -and (Test-Path -LiteralPath $runDirectory -PathType Container)) {
        Remove-Item -LiteralPath $runDirectory -Recurse -Force -ErrorAction SilentlyContinue
    }
    if ($null -ne $stagingDirectory -and
        (Test-Path -LiteralPath $stagingDirectory -PathType Container) -and
        @(Get-ChildItem -LiteralPath $stagingDirectory -Force).Count -eq 0) {
        Remove-Item -LiteralPath $stagingDirectory -Force -ErrorAction SilentlyContinue
    }
    if ($null -ne $blobDirectory -and -not $blobDirectoryExisted -and
        (Test-Path -LiteralPath $blobDirectory -PathType Container) -and
        @(Get-ChildItem -LiteralPath $blobDirectory -Force).Count -eq 0) {
        Remove-Item -LiteralPath $blobDirectory -Force -ErrorAction SilentlyContinue
    }
}
