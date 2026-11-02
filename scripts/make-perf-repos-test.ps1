# Tests make-perf-repos.ps1 on small local fixtures: the dry run, every synthetic repository at a
# thousandth of its size, the clones from a local fixture repository (default branch, a tag, and
# paths a Windows or case-insensitive file system can't hold), and running again: nothing
# rebuilt, an unfinished clone resumed, an unfinished synthetic repository rebuilt, a folder the
# script didn't make left alone. Downloads nothing; runs in a minute or two.
#
#   pwsh scripts/make-perf-repos-test.ps1 [-Work <new or empty folder>] [-Keep]
#
# Afterwards it deletes only what it made: -Work itself when it created it, else what it put in
# it. An existing -Work that isn't empty is refused.
param([string]$Work, [switch]$Keep)
$ErrorActionPreference = 'Stop'

if (-not $Work) {
    $Work = Join-Path ([IO.Path]::GetTempPath()) ('glowfly-perf-test-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
}
# Relative to PowerShell's current folder, not the process's.
$Work = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Work)
$created = -not (Test-Path -LiteralPath $Work)
if (-not $created -and (Get-ChildItem -Force -LiteralPath $Work | Select-Object -First 1)) {
    throw "-Work $Work isn't empty; give a new or empty folder"
}
$make = Join-Path $PSScriptRoot 'make-perf-repos.ps1'
$out = Join-Path $Work 'out'
$state = Join-Path $out '.state'
$failures = 0
. (Join-Path $PSScriptRoot 'perf-repos/build.ps1')

function Check([bool]$Ok, [string]$What) {
    if ($Ok) { Write-Host "ok   $What" } else { Write-Host "FAIL $What" -ForegroundColor Red; $script:failures++ }
}

# Runs the script with $Arguments; its exit code and output.
function Invoke-Make([string[]]$Arguments) {
    $text = & pwsh -NoProfile -File $make @Arguments 2>&1 | Out-String
    @{ Code = $LASTEXITCODE; Text = $text }
}

function Get-GitLine([string]$Dir, [string[]]$Arguments) { (& git -C $Dir @Arguments) -join "`n" }

try {
    New-Item -ItemType Directory -Force $Work | Out-Null
    Initialize-Git (Join-Path $Work 'no-hooks')
    # The fixture standing in for git/git and the kernel (bare: its paths are never written here).
    $fixture = Join-Path $Work 'fixture.git'
    & git init -q --bare -b trunk $fixture
    # Let it hold a name Windows reserves and two names differing only in case.
    & git -C $fixture config core.protectNTFS false
    & git -C $fixture config core.ignorecase false
    $files = @('README', 'drivers/aux.c', 'include/Foo.h', 'include/foo.h')
    $stream = foreach ($n in 1, 2) {
        "commit refs/heads/trunk`nauthor A <a@example.com> 100000000$n +0000`ncommitter A <a@example.com> 100000000$n +0000`ndata 7`ncommit$n"
        foreach ($f in $files) { "M 100644 inline $f`ndata $($f.Length + 1)`n$f$n" }
        ''
    }
    $stream += "reset refs/tags/v1`nfrom refs/heads/trunk`n"
    $bytes = [Text.Encoding]::UTF8.GetBytes(($stream -join "`n") + "`n")
    Invoke-FastImport $fixture { param($s) $s.Write($bytes, 0, $bytes.Length) }
    $common = @('-Out', $out, '-Scale', '0.001', '-GitUrl', $fixture, '-LinuxUrl', $fixture)

    $r = Invoke-Make (@('-DryRun', '-Download') + $common)
    Check ($r.Code -eq 0 -and -not (Test-Path -LiteralPath $out)) 'a dry run writes nothing'
    Check ($r.Text -match 'linux\s+to build' -and $r.Text -match 'octopus\s+to build') 'a dry run lists what it would build'
    $r = Invoke-Make (@('-DryRun') + $common)
    Check ($r.Text -match 'linux\s+skipped: needs -Download') 'the clones need -Download'
    $r = Invoke-Make @('-DryRun', '-Out', (Join-Path $PSScriptRoot 'perf'))
    Check ($r.Code -ne 0 -and $r.Text -match 'inside this checkout') 'a folder inside the checkout is refused'
    $r = Invoke-Make @('-DryRun', '-Only', 'linear,nope', '-Out', $out)
    Check ($r.Code -ne 0 -and $r.Text -match "No repository named 'nope'") 'an unknown name is refused'
    $r = Invoke-Make @('-DryRun', '-Rebuild', 'linux', '-Out', $out)
    Check ($r.Code -ne 0 -and $r.Text -match 'is a clone') 'a clone is never rebuilt'

    $r = Invoke-Make (@('-Download') + $common)
    Check ($r.Code -eq 0) 'the whole set builds'
    if ($r.Code -ne 0) { Write-Host $r.Text }
    $names = 'git', 'linux', 'linear', 'many-refs', 'wide-tree', 'octopus', 'big-files', 'lfs-pointers', 'submodules'
    Check (@($names | Where-Object { -not (Test-Path -LiteralPath (Join-Path $state "$_.done")) }).Count -eq 0) 'every repository is marked done'
    Check (@(Get-ChildItem -LiteralPath $state -Filter *.building).Count -eq 0) 'no repository is left unfinished'

    $d = Join-Path $out 'linear'
    Check ((Get-GitLine $d rev-list, --count, main) -eq '1000') 'linear: 1,000 commits'
    $d = Join-Path $out 'many-refs'
    Check (@(& git -C $d tag).Count -eq 100) 'many-refs: 100 tags'
    Check (@(& git -C $d branch).Count -eq 21) 'many-refs: 20 branches and main'
    $d = Join-Path $out 'wide-tree'
    Check (@(& git -C $d ls-files).Count -eq 500) 'wide-tree: 500 files'
    $status = @(& git -C $d status --porcelain)
    Check (@($status | Where-Object { $_ -match '^M ' }).Count -gt 0 -and @($status | Where-Object { $_ -match '^ [MD] ' }).Count -gt 0 -and @($status | Where-Object { $_ -match '^\?\? ' }).Count -gt 0) 'wide-tree: staged, unstaged and untracked changes'
    $d = Join-Path $out 'octopus'
    Check ((Get-GitLine $d log, -1, --min-parents=61, '--format=%H') -ne '') 'octopus: a 61-parent merge'
    Check (@(& git -C $d branch --list 'lane-*').Count -eq 60) 'octopus: 60 lanes'
    $d = Join-Path $out 'big-files'
    Check ((Get-Item -LiteralPath (Join-Path $d 'logs/huge.log')).Length -ge 100KB) 'big-files: the log file'
    Check ((Get-Item -LiteralPath (Join-Path $d 'assets/disk.img')).Length -eq 52429) 'big-files: the binary'
    $d = Join-Path $out 'lfs-pointers'
    $pointer = Get-Content -Raw -LiteralPath (Join-Path $d 'art/a000/asset1.bin')
    Check ($pointer.StartsWith("version https://git-lfs.github.com/spec/v1`noid sha256:")) 'lfs-pointers: pointers are checked out'
    $d = Join-Path $out 'submodules/super'
    $subs = @(& git -C $d submodule status --recursive)
    Check ($subs.Count -eq 4 -and ($subs -match 'libs/a/vendor/b/deep/c').Count -eq 1) 'submodules: three deep and one beside'
    Check ((Get-GitLine $d status, --porcelain) -match '^ M libs/a') 'submodules: libs/a is ahead of its commit'

    foreach ($clone in 'git', 'linux') {
        $d = Join-Path $out $clone
        Check ((Get-GitLine $d branch, --show-current) -eq 'trunk') "${clone}: the default branch is checked out"
        Check ((Get-GitLine $d rev-list, --count, --all) -eq '2' -and (Get-GitLine $d tag) -eq 'v1') "${clone}: the whole history and tags"
        Check ((Get-GitLine $d status, --porcelain) -eq '') "${clone}: the checkout is clean"
        $skipped = Join-Path $state "$clone.skipped.txt"
        $skippedPaths = if (Test-Path -LiteralPath $skipped) { @(Get-Content -LiteralPath $skipped) } else { @() }
        $probe = Join-Path $Work 'CaseProbe'
        Set-Content -LiteralPath $probe ''
        $insensitive = Test-Path -LiteralPath (Join-Path $Work 'caseprobe')
        Remove-Item -LiteralPath $probe
        $expect = @()
        if ($IsWindows) { $expect += 'drivers/aux.c' }
        if ($insensitive) { $expect += 'include/Foo.h', 'include/foo.h' }
        Check ((@($skippedPaths) -join ',') -eq ($expect -join ',')) "${clone}: left out exactly: $($expect -join ', ')"
        if ($IsWindows) { Check (-not (Test-Path -LiteralPath (Join-Path $d 'drivers/aux.c'))) "${clone}: drivers/aux.c isn't written" }
    }

    $before = @{}
    foreach ($n in $names) { $before[$n] = (Get-Item -LiteralPath (Join-Path $state "$n.done")).LastWriteTimeUtc }
    $r = Invoke-Make (@('-Download') + $common)
    Check ($r.Code -eq 0 -and $r.Text -notmatch '(?m)^== ') 'a second run builds nothing'
    Check (@($names | Where-Object { (Get-Item -LiteralPath (Join-Path $state "$_.done")).LastWriteTimeUtc -ne $before[$_] }).Count -eq 0) 'a second run leaves every repository as it was'

    Remove-Item -LiteralPath (Join-Path $state 'git.done')
    Set-Content -LiteralPath (Join-Path $state 'git.building') '1'
    $r = Invoke-Make (@('-Download', '-Only', 'git') + $common)
    Check ($r.Code -eq 0 -and $r.Text -match 'git\s+to carry on fetching' -and (Test-Path -LiteralPath (Join-Path $state 'git.done'))) 'an unfinished clone carries on'

    # Removing a finished clone's folder clones it again.
    Remove-Item -LiteralPath (Join-Path $out 'linux') -Recurse -Force
    $r = Invoke-Make (@('-DryRun', '-Only', 'linux') + $common)
    Check ($r.Text -match 'linux\s+skipped: needs -Download') 'a removed clone counts as not built'
    $r = Invoke-Make (@('-Download', '-Only', 'linux') + $common)
    Check ($r.Code -eq 0 -and $r.Text -match 'linux\s+to build' -and (Get-GitLine (Join-Path $out 'linux') rev-list, --count, --all) -eq '2') 'a removed clone is cloned again'
    Remove-Item -LiteralPath (Join-Path $out 'many-refs') -Recurse -Force
    $r = Invoke-Make (@('-Only', 'many-refs') + $common)
    Check ($r.Code -eq 0 -and $r.Text -match 'many-refs\s+to build' -and (Test-Path -LiteralPath (Join-Path $state 'many-refs.done'))) 'a removed synthetic repository is built again'
    Check (-not (Get-ChildItem -Force -LiteralPath (Join-Path $state 'no-hooks'))) 'hooks point at an empty folder in .state'

    Remove-Item -LiteralPath (Join-Path $state 'octopus.done')
    Set-Content -LiteralPath (Join-Path $state 'octopus.building') '1'
    Set-Content -LiteralPath (Join-Path $out 'octopus/stray.txt') 'left by a stopped build'
    $r = Invoke-Make (@('-Only', 'octopus') + $common)
    Check ($r.Code -eq 0 -and $r.Text -match 'octopus\s+to build again' -and -not (Test-Path -LiteralPath (Join-Path $out 'octopus/stray.txt'))) 'an unfinished synthetic repository is built again'

    Remove-Item -LiteralPath (Join-Path $state 'lfs-pointers.done')
    $r = Invoke-Make (@('-Only', 'lfs-pointers') + $common)
    Check ($r.Code -eq 0 -and $r.Text -match 'lfs-pointers\s+skipped: folder not made here' -and (Test-Path -LiteralPath (Join-Path $out 'lfs-pointers/.git'))) 'a folder the script did not make is left alone'

    $r = Invoke-Make @('-DryRun', '-Out', $out, '-Only', 'linear', '-Scale', '0.002')
    Check ($r.Text -match 'built at scale 0.001') 'a dry run notes a repository built at another scale'
} finally {
    if (-not $Keep -and (Test-Path -LiteralPath $Work)) {
        if ($created) { Remove-Item -LiteralPath $Work -Recurse -Force }
        else { foreach ($made in 'fixture.git', 'out', 'no-hooks') { $p = Join-Path $Work $made; if (Test-Path -LiteralPath $p) { Remove-Item -LiteralPath $p -Recurse -Force } } }
    }
}
if ($failures -gt 0) {
    Write-Host "$failures check(s) failed." -ForegroundColor Red
    exit 1
}
Write-Host 'All make-perf-repos checks passed.' -ForegroundColor Green
