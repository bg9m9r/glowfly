# Packages a Windows release build into dist/: the zip, the MSI (WiX 5) and a .sha256 file for
# each. Run by the Release workflow after `cargo build --release --target <Target>`.
#
#   pwsh scripts/package-windows.ps1 -Target x86_64-pc-windows-msvc -Tag v0.2.0
#
# Signing (both optional): when WINDOWS_CERTIFICATE (a base64 PFX) and
# WINDOWS_CERTIFICATE_PASSWORD are set, signtool signs trellis.exe before it is packed and then
# the MSI, with an RFC 3161 timestamp. Without them the files are left unsigned.
param(
    [Parameter(Mandatory = $true)][string]$Target,
    [Parameter(Mandatory = $true)][string]$Tag
)
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

$version = $Tag -replace '^[vV]', ''
$name = "trellis-$Tag-$Target"
$bin = "target/$Target/release"
$dist = "dist"
New-Item -ItemType Directory -Force $dist | Out-Null

# --- Signing ---------------------------------------------------------------------------------
$pfx = $null
if ($env:WINDOWS_CERTIFICATE -and $env:WINDOWS_CERTIFICATE_PASSWORD) {
    $pfx = Join-Path $env:RUNNER_TEMP "trellis-signing.pfx"
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

function Sign-File([string]$path) {
    if (-not $pfx) { return }
    $signtool = Find-SignTool
    & $signtool sign /f $pfx /p $env:WINDOWS_CERTIFICATE_PASSWORD /fd sha256 /tr http://timestamp.digicert.com /td sha256 /d Trellis $path
    if ($LASTEXITCODE -ne 0) { throw "signtool failed for $path" }
}

function Write-Sum([string]$path) {
    $hash = (Get-FileHash -Algorithm SHA256 $path).Hash.ToLowerInvariant()
    $file = Split-Path $path -Leaf
    # The format sha256sum writes, with LF line endings.
    [IO.File]::WriteAllText("$path.sha256", "$hash  $file`n")
}

Sign-File "$bin/trellis.exe"

# --- The zip -----------------------------------------------------------------------------------
$stage = Join-Path $dist $name
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item "$bin/trellis.exe", LICENSE-MIT, LICENSE-APACHE, README.md, THIRD-PARTY-NOTICES.md $stage
if (Test-Path CHANGELOG.md) { Copy-Item CHANGELOG.md $stage }
$zip = Join-Path $dist "$name.zip"
if (Test-Path $zip) { Remove-Item $zip }
Compress-Archive -Path $stage -DestinationPath $zip
Remove-Item -Recurse -Force $stage
Write-Sum $zip

# --- The MSI -----------------------------------------------------------------------------------
$msi = Join-Path $dist "$name.msi"
$root = (Get-Location).Path
$defines = @(
    "-d", "Version=$version",
    "-d", "BinDir=$root\$($bin -replace '/', '\')",
    "-d", "SourceDir=$root",
    "-d", "AssetsDir=$root\assets\icon"
)
if (Test-Path CHANGELOG.md) { $defines += @("-d", "HasChangelog=1") }
wix build -arch x64 @defines -o $msi packaging/windows/trellis.wxs
if ($LASTEXITCODE -ne 0) { throw "wix build failed" }
Remove-Item -ErrorAction SilentlyContinue (Join-Path $dist "$name.wixpdb")
Sign-File $msi
Write-Sum $msi

if ($pfx) { Remove-Item -Force $pfx }
Get-ChildItem $dist
