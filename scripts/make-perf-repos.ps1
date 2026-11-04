# Builds a reusable set of large repositories for performance testing (docs/PERF_REPOS.md): the
# real histories of git/git and the Linux kernel, and synthetic extremes made with git
# fast-import (a million commits in a line, 120k refs, a 500k-file working tree with many
# changes, wide branching with long octopus merges, huge and binary files, LFS pointers and
# nested submodules).
#
#   pwsh scripts/make-perf-repos.ps1 -DryRun              # what would be built, where; writes nothing
#   pwsh scripts/make-perf-repos.ps1                      # build the synthetic repositories
#   pwsh scripts/make-perf-repos.ps1 -Download            # also clone git/git and the Linux kernel
#   pwsh scripts/make-perf-repos.ps1 -Only linear,octopus # just these
#
# The clones download about 4 GB (the kernel nearly all of it), so they are made only with
# -Download. The set goes into -Out, by default `glowfly-perf-repos` in the user's local cache
# folder (%LOCALAPPDATA%, ~/Library/Caches or $XDG_CACHE_HOME), never inside this checkout.
#
# Running it again builds only what is missing: a finished repository is never touched, an
# unfinished synthetic one is built again from the start, and an unfinished clone carries on
# fetching. The script deletes nothing it didn't make: -Rebuild <names> builds finished
# synthetic repositories again, and a clone or any other folder is yours to remove.
#
# -Scale multiplies every count and size of the synthetic repositories (0.01 builds a small
# copy of the set in a minute); -GitUrl and -LinuxUrl clone from elsewhere (a mirror, or a
# local fixture in scripts/make-perf-repos-test.ps1).
# -Download is read inside Get-RepoState, which the analyzer doesn't follow.
[Diagnostics.CodeAnalysis.SuppressMessageAttribute('PSReviewUnusedParameter', 'Download')]
[CmdletBinding()]
param(
    [string]$Out,
    [string[]]$Only = @(),
    [switch]$Download,
    [string[]]$Rebuild = @(),
    [ValidateRange(0.0001, 10)][double]$Scale = 1,
    [switch]$DryRun,
    [string]$GitUrl = 'https://github.com/git/git.git',
    [string]$LinuxUrl = 'https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git'
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'perf-repos/build.ps1')
. (Join-Path $PSScriptRoot 'perf-repos/clone.ps1')

# Every repository: how it's made, what to point GLOWFLY_PERF_REPO at (Open, below the folder)
# and its rough size at scale 1.
$repos = [ordered]@{
    'git'          = @{ Url = $GitUrl; About = 'git/git, the real history (~85k commits)'; Size = '~0.3 GB download, ~0.4 GB on disk' }
    'linux'        = @{ Url = $LinuxUrl; About = 'the Linux kernel, full history (~1.5M commits, ~97k files)'; Size = '~4 GB download, ~6 GB on disk' }
    'linear'       = @{ Build = 'Build-LinearRepo'; About = '1M commits in a line over 2,000 files'; Size = '~0.6 GB' }
    'many-refs'    = @{ Build = 'Build-RefRepo'; About = '100k tags and 20k branches on 100k commits'; Size = '~0.1 GB' }
    'wide-tree'    = @{ Build = 'Build-WideTreeRepo'; About = '500k files; ~10k changed, ~2.5k deleted, ~5k untracked'; Size = '~0.7 GB' }
    'octopus'      = @{ Build = 'Build-OctopusRepo'; About = '60 lanes merged by 61-parent octopus merges, 2,000 topic merges'; Size = '~10 MB' }
    'big-files'    = @{ Build = 'Build-BigFileRepo'; About = 'a 100 MB log, a 20 MB line, a 50 MB binary x3, 200 x 1 MB binaries'; Size = '~0.75 GB' }
    'lfs-pointers' = @{ Build = 'Build-LfsRepo'; About = '5,000 Git LFS pointers (no LFS objects)'; Size = '~10 MB' }
    'submodules'   = @{ Build = 'Build-SubmoduleRepo'; Open = 'super'; About = 'submodules three deep, one moved ahead and dirty'; Size = '~1 MB' }
}

# `pwsh -File` passes a comma list as one string.
$Only = @($Only | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
$Rebuild = @($Rebuild | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
foreach ($name in $Only + $Rebuild) {
    if (-not $repos.Contains($name)) { throw "No repository named '$name'; the names are: $($repos.Keys -join ', ')" }
}
foreach ($name in $Rebuild) {
    if ($repos[$name].Url) { throw "$name is a clone; remove its folder yourself to clone it again" }
}

if (-not $Out) {
    $cache = if ($IsWindows) { $env:LOCALAPPDATA }
    elseif ($IsMacOS) { Join-Path $HOME 'Library/Caches' }
    elseif ($env:XDG_CACHE_HOME -and [IO.Path]::IsPathRooted($env:XDG_CACHE_HOME)) { $env:XDG_CACHE_HOME }
    else { Join-Path $HOME '.cache' }
    $Out = Join-Path $cache 'glowfly-perf-repos'
}
# Relative to PowerShell's current folder, not the process's.
$Out = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Out)
$checkout = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..')).TrimEnd([IO.Path]::DirectorySeparatorChar)
$sep = [IO.Path]::DirectorySeparatorChar
if (($Out + $sep).StartsWith($checkout + $sep, [StringComparison]::OrdinalIgnoreCase)) {
    throw "-Out $Out is inside this checkout; choose a folder outside it"
}
$stateDir = Join-Path $Out '.state'

# What a run would do with repository $name: done, build, rebuild, resume, download (needs
# -Download) or foreign (a folder this script didn't make).
function Get-RepoState([string]$name) {
    $dir = Join-Path $Out $name
    $exists = Test-Path -LiteralPath $dir
    # A finished repository whose folder was removed is built again.
    $done = $exists -and (Test-Path -LiteralPath (Join-Path $stateDir "$name.done"))
    $building = Test-Path -LiteralPath (Join-Path $stateDir "$name.building")
    $clone = [bool]$repos[$name].Url
    if ($done) { if ($Rebuild -contains $name) { 'rebuild' } else { 'done' } }
    elseif ($clone -and -not $Download) { 'download' }
    elseif ($building) { if ($clone) { 'resume' } else { 'rebuild' } }
    elseif ($exists) { 'foreign' }
    else { 'build' }
}

$names = if ($Only.Count -gt 0) { @($repos.Keys | Where-Object { $Only -contains $_ -or $Rebuild -contains $_ }) } else { @($repos.Keys) }
$what = @{
    done = 'built'; build = 'to build'; rebuild = 'to build again'; resume = 'to carry on fetching'
    download = 'skipped: needs -Download'; foreign = 'skipped: folder not made here'
}
Write-Host "Performance repositories in $Out (scale $Scale)"
foreach ($name in $names) {
    $state = Get-RepoState $name
    $r = $repos[$name]
    $note = if ($r.Url) { "from $($r.Url)" } else { $r.Size + ' at scale 1' }
    Write-Host ('  {0,-13} {1,-26} {2} ({3})' -f $name, $what[$state], $r.About, $note)
    if ($state -eq 'done') {
        $built = Get-Content -Raw -LiteralPath (Join-Path $stateDir "$name.done") | ConvertFrom-Json
        if (-not $r.Url -and [double]$built.scale -ne $Scale) {
            Write-Host "                built at scale $($built.scale); -Rebuild $name builds it at $Scale"
        }
    }
}
if ($DryRun) { exit 0 }

New-Item -ItemType Directory -Force $stateDir | Out-Null
Initialize-Git (Join-Path $stateDir 'no-hooks')
$failed = @()
foreach ($name in $names) {
    $state = Get-RepoState $name
    if ($state -notin 'build', 'rebuild', 'resume') { continue }
    $r = $repos[$name]
    $dir = Join-Path $Out $name
    $marker = Join-Path $stateDir "$name.building"
    $doneFile = Join-Path $stateDir "$name.done"
    Write-Host "`n== ${name}: $($r.About)"
    $timer = [Diagnostics.Stopwatch]::StartNew()
    try {
        [IO.File]::WriteAllText($marker, "$Scale`n")
        # A stale marker of a removed folder goes too, so a failed build isn't taken as done.
        if (Test-Path -LiteralPath $doneFile) { Remove-Item -LiteralPath $doneFile }
        if ($state -eq 'rebuild' -and (Test-Path -LiteralPath $dir)) { Remove-Item -LiteralPath $dir -Recurse -Force }
        if ($r.Url) {
            Build-Clone $dir $r.Url (Join-Path $stateDir "$name.skipped.txt")
        } else {
            & $r.Build $dir
        }
        $open = if ($r.Open) { Join-Path $dir $r.Open } else { $dir }
        $record = [ordered]@{ built = (Get-Date).ToString('o'); scale = $Scale; open = $open; seconds = [int]$timer.Elapsed.TotalSeconds }
        if ($r.Url) { $record.Remove('scale'); $record.url = $r.Url }
        [IO.File]::WriteAllText($doneFile, ($record | ConvertTo-Json))
        Remove-Item -LiteralPath $marker
        Write-Host "== $name built in $([int]$timer.Elapsed.TotalSeconds) s"
    } catch {
        Write-Host "== $name failed: $_" -ForegroundColor Red
        $failed += $name
    }
}

Write-Host "`nPoint GLOWFLY_PERF_REPO (or Glowfly) at:"
foreach ($name in $names) {
    $doneFile = Join-Path $stateDir "$name.done"
    if ((Test-Path -LiteralPath $doneFile) -and (Test-Path -LiteralPath (Join-Path $Out $name))) {
        Write-Host ('  {0,-13} {1}' -f $name, (Get-Content -Raw -LiteralPath $doneFile | ConvertFrom-Json).open)
    }
}
if ($failed.Count -gt 0) {
    Write-Host "`nNot finished: $($failed -join ', '). Run the script again to carry on." -ForegroundColor Red
    exit 1
}
