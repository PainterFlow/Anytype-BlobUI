function Restore-ProcessEnvironment {
    param([Parameter(Mandatory)][System.Collections.IDictionary]$SavedEnvironment)

    foreach ($name in $SavedEnvironment.Keys) {
        $variablePath = "Env:$name"
        if ($null -eq $SavedEnvironment[$name]) {
            Remove-Item -LiteralPath $variablePath -ErrorAction SilentlyContinue
        } else {
            Set-Item -LiteralPath $variablePath -Value $SavedEnvironment[$name]
        }
    }
}
