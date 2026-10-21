# Checks `glowfly --check-update` against local update feeds (GLOWFLY_UPDATE_FEED): a folder
# holding release.json, the packages and SHA256SUMS, built here from the given package files.
# Installs nothing and opens no window, so it runs on any build; the installer checks
# (installer-check-windows.ps1, installer-check-linux.sh) run it with the installed program.
#
#   pwsh -NoProfile -File scripts/update-feed-check.ps1 -Glowfly <program> -Version 0.1.0 `
#     -Installer <dir>/glowfly-v0.1.0-<triple>.msi -Archive <dir>/glowfly-v0.1.0-<triple>.zip `
#     -Expect installer -Out <work folder>
#
# -Expect names the file the program should choose: `installer` for an installed copy,
# `archive` for any other (a cargo-built debug program counts as portable). The packages may be
# stand-in files: only their names and bytes matter. Each case's output is kept in -Out.
param(
    [Parameter(Mandatory = $true)][string]$Glowfly,
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][string]$Installer,
    [Parameter(Mandatory = $true)][string]$Archive,
    [Parameter(Mandatory = $true)][ValidateSet('installer', 'archive')][string]$Expect,
    [Parameter(Mandatory = $true)][string]$Out
)
$ErrorActionPreference = 'Stop'

$Glowfly = (Resolve-Path -LiteralPath $Glowfly).Path
$Installer = (Resolve-Path -LiteralPath $Installer).Path
$Archive = (Resolve-Path -LiteralPath $Archive).Path
$Out = [IO.Path]::GetFullPath($Out)
New-Item -ItemType Directory -Force $Out | Out-Null
$utf8 = [Text.UTF8Encoding]::new($false)

$namePattern = '^glowfly-v(?<ver>[^-]+)-(?<triple>[^.]+)\.(?<ext>msi|deb|dmg|zip|tar\.gz)$'

# The triple and extension of a package file named glowfly-v<Version>-<triple>.<ext>.
function Get-PackageName([string]$Path) {
    $leaf = Split-Path -Leaf $Path
    $m = [regex]::Match($leaf, $namePattern)
    if (-not $m.Success) { throw "$leaf isn't named glowfly-v<version>-<triple>.<ext>" }
    if ($m.Groups['ver'].Value -ne $Version) { throw "$leaf isn't version $Version" }
    @{ Path = $Path; Triple = $m.Groups['triple'].Value; Ext = $m.Groups['ext'].Value }
}

$packages = @((Get-PackageName $Installer), (Get-PackageName $Archive))
if ($packages[0].Ext -notin 'msi', 'deb', 'dmg') { throw "-Installer must be an .msi, .deb or .dmg" }
if ($packages[1].Ext -notin 'zip', 'tar.gz') { throw "-Archive must be a .zip or .tar.gz" }
if ($packages[0].Triple -ne $packages[1].Triple) { throw "-Installer and -Archive are for different systems" }
$underTest = if ($Expect -eq 'installer') { $packages[0] } else { $packages[1] }
$wantKind = if ($Expect -eq 'installer') { 'installer' } else { 'portable' }

# The next patch release after this build's version (pre-release and build parts dropped).
$core = ($Version -split '[-+]')[0]
$parts = $core.Split('.')
if ($parts.Count -ne 3) { throw "$Version isn't a version" }
$newTag = "v$($parts[0]).$($parts[1]).$([int]$parts[2] + 1)"

function Get-Sha256([string]$Path) {
    (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash.ToLowerInvariant()
}

function Get-FeedName([hashtable]$Package, [string]$Tag) {
    "glowfly-$Tag-$($Package.Triple).$($Package.Ext)"
}

# Stops when a feed file the script wrote starts with a BOM or holds a carriage return.
function Assert-FeedText([string]$Path) {
    $bytes = [IO.File]::ReadAllBytes($Path)
    $bom = $bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF
    if ($bom) { throw "$Path starts with a byte order mark" }
    if ($bytes -contains 13) { throw "$Path has a carriage return (lines must end in LF)" }
}

# A feed folder for $Tag: both packages under the tag's names, SHA256SUMS (the format
# sha256sum writes, LF, no BOM) and release.json (UTF-8 without a BOM, which the JSON reader
# refuses). -Digest replaces the tested package's checksum, -OmitSum leaves its line out and
# -TamperAfter changes the package once SHA256SUMS is written.
function New-Feed {
    param(
        [Parameter(Mandatory = $true)][string]$Dir,
        [Parameter(Mandatory = $true)][string]$Tag,
        [string]$Digest,
        [switch]$OmitSum,
        [switch]$TamperAfter
    )
    New-Item -ItemType Directory -Force $Dir | Out-Null
    $tested = Get-FeedName $underTest $Tag
    $names = [Collections.Generic.List[string]]::new()
    foreach ($p in $packages) {
        $name = Get-FeedName $p $Tag
        Copy-Item -LiteralPath $p.Path -Destination (Join-Path $Dir $name)
        $names.Add($name)
    }
    $sorted = [string[]]$names.ToArray()
    [Array]::Sort($sorted, [StringComparer]::Ordinal)
    $lines = foreach ($name in $sorted) {
        if ($name -eq $tested -and $OmitSum) { continue }
        $hash = if ($name -eq $tested -and $Digest) { $Digest } else { Get-Sha256 (Join-Path $Dir $name) }
        "$hash  $name`n"
    }
    [IO.File]::WriteAllText((Join-Path $Dir 'SHA256SUMS'), (-join $lines), $utf8)
    $assets = @($sorted + 'SHA256SUMS' | ForEach-Object { @{ name = $_ } })
    $release = [ordered]@{ tag_name = $Tag; name = "Glowfly $Tag"; body = 'Installer check feed'; assets = $assets }
    # ConvertTo-Json separates lines with the system's newline (CRLF on Windows).
    $json = ($release | ConvertTo-Json -Depth 4) -replace "`r`n", "`n"
    [IO.File]::WriteAllText((Join-Path $Dir 'release.json'), $json, $utf8)
    foreach ($file in 'SHA256SUMS', 'release.json') { Assert-FeedText (Join-Path $Dir $file) }
    if ($TamperAfter) {
        $stream = [IO.File]::Open((Join-Path $Dir $tested), [IO.FileMode]::Append)
        try { $stream.WriteByte(0x21) } finally { $stream.Dispose() }
    }
}

# Runs `glowfly --check-update <Dl>` with GLOWFLY_UPDATE_FEED set to $Feed (unset for $null),
# keeping its output as <Out>/<Name>.out and .err.
function Invoke-Check([AllowNull()][string]$Feed, [string]$Dl, [string]$Name) {
    $outFile = Join-Path $Out "$Name.out"
    $errFile = Join-Path $Out "$Name.err"
    $saved = $env:GLOWFLY_UPDATE_FEED
    try {
        if ($Feed) { $env:GLOWFLY_UPDATE_FEED = $Feed } else { Remove-Item Env:GLOWFLY_UPDATE_FEED -ErrorAction SilentlyContinue }
        $p = Start-Process -FilePath $Glowfly -ArgumentList '--check-update', "`"$Dl`"" -Wait -PassThru -NoNewWindow `
            -RedirectStandardOutput $outFile -RedirectStandardError $errFile
    } finally {
        if ($null -eq $saved) { Remove-Item Env:GLOWFLY_UPDATE_FEED -ErrorAction SilentlyContinue } else { $env:GLOWFLY_UPDATE_FEED = $saved }
    }
    @{ Code = $p.ExitCode; Out = "$(Get-Content -Raw -LiteralPath $outFile)"; Err = "$(Get-Content -Raw -LiteralPath $errFile)" }
}

$script:case = ''
$script:run = @{ Code = $null; Out = ''; Err = '' }

function Assert-That([bool]$Condition, [string]$Message) {
    if ($Condition) { return }
    Write-Host "::error::$($script:case): $Message"
    Write-Host "exit code: $($script:run.Code)"
    Write-Host "stdout:`n$($script:run.Out)"
    Write-Host "stderr:`n$($script:run.Err)"
    exit 1
}

# Whether $Dir is missing or holds nothing.
function Test-Empty([string]$Dir) {
    -not (Test-Path -LiteralPath $Dir) -or -not (Get-ChildItem -Force -LiteralPath $Dir | Select-Object -First 1)
}

function Assert-Empty([string]$Dir) {
    Assert-That (Test-Empty $Dir) "something was saved in $Dir"
}

function Start-Case([string]$Name) {
    $script:case = $Name
    $dir = Join-Path $Out $Name
    if (Test-Path -LiteralPath $dir) { Remove-Item -Recurse -Force -LiteralPath $dir }
    @{ Feed = (Join-Path $dir 'feed'); Dl = (Join-Path $dir 'dl') }
}

function Complete-Case {
    Write-Host "ok $($script:case)"
}

# Up to date: the feed's release is this version.
$c = Start-Case 'same-version'
New-Feed -Dir $c.Feed -Tag "v$Version"
$script:run = Invoke-Check $c.Feed $c.Dl $script:case
Assert-That ($script:run.Code -eq 3) 'expected exit 3 (up to date)'
Assert-That ($script:run.Out -match '(?m)^up-to-date\r?$') 'no up-to-date line'
Assert-That ($script:run.Out -match "(?m)^current $([regex]::Escape($Version))\r?$") "no current $Version line"
Assert-Empty $c.Dl
Complete-Case

# No release yet: a folder without release.json.
$c = Start-Case 'empty-feed'
New-Item -ItemType Directory -Force $c.Feed | Out-Null
$script:run = Invoke-Check $c.Feed $c.Dl $script:case
Assert-That ($script:run.Code -eq 3) 'expected exit 3 (no release)'
Assert-Empty $c.Dl
Complete-Case

# A newer release: this system's file for this kind of install, saved byte for byte.
$c = Start-Case 'newer'
New-Feed -Dir $c.Feed -Tag $newTag
$script:run = Invoke-Check $c.Feed $c.Dl $script:case
$wantName = Get-FeedName $underTest $newTag
Assert-That ($script:run.Code -eq 0) 'expected exit 0 (downloaded)'
Assert-That ($script:run.Out -match "(?m)^install $wantKind\r?$") "expected install $wantKind"
Assert-That ($script:run.Out -match "(?m)^offer $([regex]::Escape($newTag.TrimStart('v'))) $([regex]::Escape($wantName))\r?$") "expected an offer of $wantName"
$m = [regex]::Match($script:run.Out, '(?m)^downloaded (?<path>.+?)\r?$')
Assert-That $m.Success 'no downloaded line'
$saved = $m.Groups['path'].Value
Assert-That ([IO.Path]::GetFullPath($saved) -eq [IO.Path]::GetFullPath((Join-Path $c.Dl $wantName))) "saved at $saved, not in the download folder as $wantName"
Assert-That (Test-Path -LiteralPath $saved -PathType Leaf) "$saved doesn't exist"
Assert-That ((Get-Sha256 $saved) -eq (Get-Sha256 $underTest.Path)) 'the saved file differs from the package'
Complete-Case

# A checksum that doesn't match the file.
$c = Start-Case 'wrong-digest'
New-Feed -Dir $c.Feed -Tag $newTag -Digest ('0' * 64)
$script:run = Invoke-Check $c.Feed $c.Dl $script:case
Assert-That ($script:run.Code -eq 1) 'expected exit 1 (refused)'
Assert-That ($script:run.Err -match "didn't match its published checksum") 'expected the checksum error'
Assert-Empty $c.Dl
Complete-Case

# The file changed after SHA256SUMS was written.
$c = Start-Case 'changed-file'
New-Feed -Dir $c.Feed -Tag $newTag -TamperAfter
$script:run = Invoke-Check $c.Feed $c.Dl $script:case
Assert-That ($script:run.Code -eq 1) 'expected exit 1 (refused)'
Assert-That ($script:run.Err -match "didn't match its published checksum") 'expected the checksum error'
Assert-Empty $c.Dl
Complete-Case

# SHA256SUMS without the file's line.
$c = Start-Case 'no-sum'
New-Feed -Dir $c.Feed -Tag $newTag -OmitSum
$script:run = Invoke-Check $c.Feed $c.Dl $script:case
Assert-That ($script:run.Code -eq 1) 'expected exit 1 (refused)'
Assert-That ($script:run.Err -match "doesn't publish a checksum") 'expected the missing-checksum error'
Assert-Empty $c.Dl
Complete-Case

# A relative feed folder is a configuration error, never resolved against the working folder.
$c = Start-Case 'relative-feed'
$script:run = Invoke-Check 'feed' $c.Dl $script:case
Assert-That ($script:run.Code -eq 2) 'expected exit 2 (configuration error)'
Assert-That ($script:run.Err -match 'must name an absolute folder') 'expected the absolute-folder error'
Assert-Empty $c.Dl
Complete-Case

Write-Host 'all update-feed checks passed'
exit 0
