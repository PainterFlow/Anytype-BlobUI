$ErrorActionPreference = 'Stop'
$settingsExecutable = Join-Path (Split-Path -Parent $PSScriptRoot) 'settings-app/dist/blobui-settings.exe'
if (Test-Path -LiteralPath $settingsExecutable -PathType Leaf) {
    & (Join-Path $PSScriptRoot 'Settings.ps1')
    return
}
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
[Windows.Forms.Application]::EnableVisualStyles()
$source = Split-Path -Parent $PSScriptRoot
$form = New-Object Windows.Forms.Form
$form.Text = 'BlobUI'
$form.ClientSize = New-Object Drawing.Size(520, 210)
$form.StartPosition = 'CenterScreen'
$form.FormBorderStyle = 'FixedDialog'
$form.MaximizeBox = $false
$label = New-Object Windows.Forms.Label
$label.SetBounds(24, 24, 470, 95)
$label.Text = "BlobUI is installed.`r`n`r`nEnable File > Apply Custom CSS in Anytype and press Ctrl+R.`r`nOnboarding will be added in a future update."
$form.Controls.Add($label)
$button = New-Object Windows.Forms.Button
$button.Text = 'Rebuild CSS'
$button.SetBounds(24, 140, 140, 36)
$button.Add_Click({
    $button.Enabled = $false
    try {
        & (Join-Path $PSScriptRoot 'Rebuild.ps1')
        [Windows.Forms.MessageBox]::Show('CSS rebuilt. Press Ctrl+R in Anytype.', 'BlobUI') | Out-Null
    }
    catch { [Windows.Forms.MessageBox]::Show($_.Exception.Message, 'BlobUI') | Out-Null }
    finally { $button.Enabled = $true }
})
$form.Controls.Add($button)
[void]$form.ShowDialog()
$form.Dispose()
