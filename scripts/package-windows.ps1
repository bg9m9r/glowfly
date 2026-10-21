# Packages a Windows release build into dist/: the zip, the MSI (WiX 5) and a .sha256 file for
# each. Run by the Release workflow after `cargo build --release --target <Target>`.
#
#   pwsh scripts/package-windows.ps1 -Target x86_64-pc-windows-msvc -Tag v0.2.0
#
# Signing (both optional): when WINDOWS_CERTIFICATE (a base64 PFX) and
# WINDOWS_CERTIFICATE_PASSWORD are set, signtool signs glowfly.exe before it is packed and then
# the MSI, with an RFC 3161 timestamp. Without them the files are left unsigned.
param(
    [Parameter(Mandatory = $true)][string]$Target,
    [Parameter(Mandatory = $true)][string]$Tag
)
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

$version = $Tag -replace '^[vV]', ''
$name = "glowfly-$Tag-$Target"
$bin = "target/$Target/release"
$dist = "dist"
New-Item -ItemType Directory -Force $dist | Out-Null

# --- Signing ---------------------------------------------------------------------------------
$pfx = $null
if ($env:WINDOWS_CERTIFICATE -and $env:WINDOWS_CERTIFICATE_PASSWORD) {
    $pfx = Join-Path $env:RUNNER_TEMP "glowfly-signing.pfx"
    [IO.File]::WriteAllBytes($pfx, [Convert]::FromBase64String($env:WINDOWS_CERTIFICATE))
} else {
    Write-Host "::notice::Windows code signing skipped (no WINDOWS_CERTIFICATE)"
}

function Find-SignTool {
    $found = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($found) { return $found.Source }
    $kits = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe" -ErrorAction SilentlyContinue |
        Sort-Object FullName -Descending | Select-Object -First 1
    if (-not $kits) { throw "signtool.exe wasn't found" }
    return $kits.FullName
}

function Add-Signature([string]$path) {
    if (-not $pfx) { return }
    $signtool = Find-SignTool
    & $signtool sign /f $pfx /p $env:WINDOWS_CERTIFICATE_PASSWORD /fd sha256 /tr http://timestamp.digicert.com /td sha256 /d Glowfly $path
    if ($LASTEXITCODE -ne 0) { throw "signtool failed for $path" }
}

function Write-Sum([string]$path) {
    $hash = (Get-FileHash -Algorithm SHA256 $path).Hash.ToLowerInvariant()
    $file = Split-Path $path -Leaf
    # The format sha256sum writes, with LF line endings. .NET resolves a relative path against
    # the process's directory, not the script's location, so pass a full one.
    [IO.File]::WriteAllText((Join-Path (Get-Location).Path "$path.sha256"), "$hash  $file`n")
}

Add-Signature "$bin/glowfly.exe"

# --- The zip -----------------------------------------------------------------------------------
$stage = Join-Path $dist $name
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item "$bin/glowfly.exe", LICENSE-MIT, LICENSE-APACHE, README.md, THIRD-PARTY-NOTICES.md $stage
if (Test-Path CHANGELOG.md) { Copy-Item CHANGELOG.md $stage }
$zip = Join-Path $dist "$name.zip"
if (Test-Path $zip) { Remove-Item $zip }
Compress-Archive -Path $stage -DestinationPath $zip
Remove-Item -Recurse -Force $stage
Write-Sum $zip

# --- The MSI -----------------------------------------------------------------------------------
$msi = Join-Path $dist "$name.msi"
$root = (Get-Location).Path

# WiX's standard pages and its launch-after-install action, kept in .wix/ under the repository.
$wixVersion = "5.0.2"
# Added once: a second run in the same checkout (an older and a newer package) finds them listed.
# An unrecognised listing falls back to adding.
$listed = (wix extension list | Out-String)
foreach ($ext in "WixToolset.UI.wixext", "WixToolset.Util.wixext") {
    if ($listed -match [regex]::Escape("$ext $wixVersion")) { continue }
    wix extension add "$ext/$wixVersion"
    if ($LASTEXITCODE -ne 0) { throw "wix extension add $ext failed" }
}

# The License page: the licence choice, then the MIT text, as RTF (the page shows nothing else).
function ConvertTo-Rtf([string]$text) {
    $escaped = $text -replace '\\', '\\' -replace '\{', '\{' -replace '\}', '\}'
    # Hard-wrapped lines join into paragraphs; blank lines separate them.
    $paras = ($escaped -replace "`r", '') -split "`n\s*`n" | ForEach-Object { ($_ -split "`n" | ForEach-Object Trim) -join ' ' }
    $body = ($paras | Where-Object { $_ }) -join "\par\par`n"
    "{\rtf1\ansi\deff0{\fonttbl{\f0 Segoe UI;}}\fs18`n$body\par`n}"
}
$intro = "Glowfly is available under the MIT License or the Apache License, Version 2.0, at your option. " +
    "Both are installed with it (LICENSE-MIT and LICENSE-APACHE), and THIRD-PARTY-NOTICES.md lists the licenses " +
    "of the code it includes."
$rtf = Join-Path $root "$dist\license.rtf"
[IO.File]::WriteAllText($rtf, (ConvertTo-Rtf "$intro`n`n$(Get-Content -Raw LICENSE-MIT)"))

$defines = @(
    "-d", "Version=$version",
    "-d", "BinDir=$root\$($bin -replace '/', '\')",
    "-d", "SourceDir=$root",
    "-d", "AssetsDir=$root\assets",
    "-d", "LicenseRtf=$rtf"
)
if (Test-Path CHANGELOG.md) { $defines += @("-d", "HasChangelog=1") }
$exts = @("-ext", "WixToolset.UI.wixext/$wixVersion", "-ext", "WixToolset.Util.wixext/$wixVersion")
wix build -arch x64 @exts @defines -o $msi packaging/windows/glowfly.wxs packaging/windows/pages.wxs
if ($LASTEXITCODE -ne 0) { throw "wix build failed" }
Remove-Item -ErrorAction SilentlyContinue (Join-Path $dist "$name.wixpdb"), $rtf
Add-Signature $msi
Write-Sum $msi

if ($pfx) { Remove-Item -Force $pfx }
Get-ChildItem $dist
