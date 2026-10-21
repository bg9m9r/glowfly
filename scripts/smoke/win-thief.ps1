# A small window for scripts/smoke-windows.ps1 to give the foreground to, so Glowfly loses the
# focus. The caller places it at (X, Y), in the work area's bottom-right corner, away from the
# rows the checks click, and stops it when the pass ends.
param(
    [int] $X,
    [int] $Y,
    [string] $Title = 'Glowfly smoke focus thief'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms

$form = [System.Windows.Forms.Form]::new()
$form.StartPosition = [System.Windows.Forms.FormStartPosition]::Manual
$form.Location = [System.Drawing.Point]::new($X, $Y)
$form.Size = [System.Drawing.Size]::new(220, 90)
$form.Text = $Title
$label = [System.Windows.Forms.Label]::new()
$label.Text = 'focus thief'
$label.Dock = [System.Windows.Forms.DockStyle]::Fill
$label.TextAlign = [System.Drawing.ContentAlignment]::MiddleCenter
$form.Controls.Add($label)
[System.Windows.Forms.Application]::Run($form)
