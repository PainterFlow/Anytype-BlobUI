# Dot-source from a build script with $workspaceRoot set.
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        $toolchainRoot = Join-Path $workspaceRoot '.test-run\toolchain'
        $rustRoot = Join-Path $toolchainRoot 'rustup\toolchains\stable-x86_64-pc-windows-msvc'
        $rustBin = Join-Path $rustRoot 'bin'
        if (-not (Test-Path -LiteralPath (Join-Path $rustBin 'cargo.exe') -PathType Leaf)) {
            throw 'Cargo was not found on PATH or in the workspace toolchain. Install Rust or add its cargo bin folder to PATH, then reopen PowerShell.'
        }
        $env:CARGO_HOME = Join-Path $toolchainRoot 'cargo'
        $env:RUSTUP_HOME = Join-Path $toolchainRoot 'rustup'
        $env:TEMP = Join-Path $toolchainRoot 'temp'
        $env:TMP = $env:TEMP
        [System.IO.Directory]::CreateDirectory($env:TEMP) | Out-Null
        $env:PATH = "$rustBin;$env:PATH"

        # The workspace toolchain uses the installed Visual C++ compiler and its local SDK.
        $visualStudioRoot = Join-Path ${env:ProgramFiles} 'Microsoft Visual Studio'
        $vcRoot = Get-ChildItem -Path (Join-Path $visualStudioRoot '*\*\VC\Tools\MSVC\*') -Directory -ErrorAction SilentlyContinue |
            Where-Object { Test-Path -LiteralPath (Join-Path $_.FullName 'bin\Hostx64\x64\cl.exe') } |
            Sort-Object Name -Descending | Select-Object -First 1
        $sdkRoot = Join-Path $toolchainRoot 'sdk'
        $sdkHeaders = Get-ChildItem -LiteralPath (Join-Path $sdkRoot 'microsoft.windows.sdk.cpp\c\Include') -Directory -ErrorAction SilentlyContinue |
            Sort-Object Name -Descending | Select-Object -First 1
        if (-not $vcRoot -or -not $sdkHeaders) {
            throw 'The workspace Rust toolchain requires Visual C++ build tools and the workspace Windows SDK, but one is missing.'
        }
        $sdkBin = Join-Path $sdkRoot "microsoft.windows.sdk.cpp\c\bin\$($sdkHeaders.Name)\x64"
        $env:PATH = "$($vcRoot.FullName)\bin\Hostx64\x64;$sdkBin;$env:PATH"
        $env:LIB = "$($vcRoot.FullName)\lib\x64;$sdkRoot\microsoft.windows.sdk.cpp.x64\c\um\x64;$sdkRoot\microsoft.windows.sdk.cpp.x64\c\ucrt\x64;$env:LIB"
        $env:INCLUDE = "$($vcRoot.FullName)\include;$($sdkHeaders.FullName)\ucrt;$($sdkHeaders.FullName)\shared;$($sdkHeaders.FullName)\um;$($sdkHeaders.FullName)\winrt;$env:INCLUDE"
        $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = Join-Path $rustRoot 'lib\rustlib\x86_64-pc-windows-msvc\bin\rust-lld.exe'
        Write-Host 'Using the workspace Rust toolchain.'
    }
