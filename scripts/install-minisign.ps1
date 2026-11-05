# Installs minisign from its author's release into -Dir, on x86_64 Windows: the Installers
# workflow signs its local update feeds with it and the committed test key. The zip is checked
# against the SHA-256 pinned here before anything in it runs. That digest is of the file Frank
# Denis signed: the zip's .minisig was verified with his key
# (RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3, from minisign's README) when it was
# pinned, so a changed download fails. scripts/install-minisign.sh says how to update it.
#
#   pwsh -NoProfile -File scripts/install-minisign.ps1 -Dir $env:RUNNER_TEMP/minisign   # then <Dir>/minisign.exe
param(
    [Parameter(Mandatory = $true)][string]$Dir
)
$ErrorActionPreference = 'Stop'
$version = '0.12'
$sha256 = '37b600344e20c19314b2e82813db2bfdcc408b77b876f7727889dbd46d539479'

$Dir = [IO.Path]::GetFullPath($Dir)
New-Item -ItemType Directory -Force $Dir | Out-Null
$zip = Join-Path $Dir "minisign-$version-win64.zip"
Invoke-WebRequest -UseBasicParsing -MaximumRetryCount 3 -OutFile $zip `
    -Uri "https://github.com/jedisct1/minisign/releases/download/$version/minisign-$version-win64.zip"
$got = (Get-FileHash -Algorithm SHA256 -LiteralPath $zip).Hash.ToLowerInvariant()
if ($got -ne $sha256) { throw "minisign-$version-win64.zip has SHA-256 $got, expected $sha256" }
$unpacked = Join-Path $Dir 'unpacked'
Expand-Archive -LiteralPath $zip -DestinationPath $unpacked -Force
Move-Item -Force -LiteralPath (Join-Path $unpacked 'minisign-win64/x86_64/minisign.exe') -Destination (Join-Path $Dir 'minisign.exe')
Remove-Item -Recurse -Force -LiteralPath $unpacked, $zip
& (Join-Path $Dir 'minisign.exe') -v
if ($LASTEXITCODE -ne 0) { throw "minisign.exe -v exited with $LASTEXITCODE" }
