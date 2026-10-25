# Re-renders only the UI snapshots git reports as conflicted (a merge, rebase or cherry-pick that
# stopped), then stages them. Snapshots are rendered from the code, so the merged code's own
# rendering is the right resolution for both the text snapshots (.txt) and the pixel goldens (.png).
#
#   pwsh scripts/rerender-conflicted.ps1           # re-render and stage the conflicted snapshots
#   pwsh scripts/rerender-conflicted.ps1 -DryRun   # list them and the tests that would run
#
# Resolve the code conflicts first: the tests are built from the working tree. Each snapshot is
# found by its name's string literal in crates/glowfly-app/src; the test around it runs (or, when
# the name sits in a helper, every test in that file). A name not found as a literal runs every
# snapshot test.
param([switch]$DryRun)
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

$dir = "crates/glowfly-app/tests/snapshots"
$conflicted = @(git diff --name-only --diff-filter=U -- $dir)
if ($LASTEXITCODE -ne 0) { Write-Error "git diff failed" }
if ($conflicted.Count -eq 0) {
    Write-Host "No conflicted snapshots."
    exit 0
}

# The test functions in a file's lines (`#[test]` within the attributes above `fn`).
function Test-Fns([string[]]$lines) {
    $fns = @()
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match '^\s*(pub(\([a-z]+\))?\s+)?fn\s+(\w+)') {
            $name = $Matches[3]
            for ($j = $i - 1; $j -ge 0 -and $lines[$j].Trim().StartsWith('#['); $j--) {
                if ($lines[$j].Trim() -eq '#[test]') { $fns += $name; break }
            }
        }
    }
    $fns
}

# The test(s) to run for snapshot `name`, or $null when its literal isn't found.
function Tests-For([string]$name) {
    $hits = @(git grep -n -F "`"$name`"" -- crates/glowfly-app/src)
    if ($hits.Count -eq 0) { return $null }
    $out = @()
    foreach ($hit in $hits) {
        $file, $line, $null = $hit -split ':', 3
        $lines = Get-Content $file
        $at = [int]$line - 1
        $fn = $null
        for ($i = $at; $i -ge 0; $i--) {
            if ($lines[$i] -match '^\s*(pub(\([a-z]+\))?\s+)?fn\s+(\w+)') { $fn = $Matches[3]; $start = $i; break }
        }
        $isTest = $false
        if ($fn) {
            for ($j = $start - 1; $j -ge 0 -and $lines[$j].Trim().StartsWith('#['); $j--) {
                if ($lines[$j].Trim() -eq '#[test]') { $isTest = $true; break }
            }
        }
        if ($isTest) { $out += $fn } else { $out += Test-Fns $lines }
    }
    $out
}

$names = $conflicted | ForEach-Object { [IO.Path]::GetFileNameWithoutExtension($_) } | Sort-Object -Unique
$filters = @()
$everything = $false
foreach ($name in $names) {
    $tests = Tests-For $name
    if ($null -eq $tests -or $tests.Count -eq 0) {
        Write-Host "note: no test names snapshot $name as a literal; running every snapshot test" -ForegroundColor Yellow
        $everything = $true
    } else {
        Write-Host "$name <- $($tests -join ', ')"
        $filters += $tests
    }
}
$filters = if ($everything) { @("snapshot") } else { @($filters | Sort-Object -Unique) }

Write-Host "Conflicted snapshots: $($conflicted.Count); tests to run: $($filters -join ' ')"
if ($DryRun) { exit 0 }

# A clean copy to overwrite (a text snapshot holds conflict markers until then).
foreach ($f in $conflicted) {
    git checkout --ours -- $f 2>$null
}

$env:UPDATE_SNAPSHOTS = "1"
try {
    cargo test -p glowfly --bin glowfly -- @filters
    if ($LASTEXITCODE -ne 0) { Write-Error "the snapshot tests failed; fix them, then run this again" }
} finally {
    Remove-Item Env:UPDATE_SNAPSHOTS
}

foreach ($f in $conflicted) {
    if (Test-Path $f) { git add -- $f } else { git rm -q --cached -- $f }
}
Write-Host "Re-rendered and staged $($conflicted.Count) snapshot(s). Look at the changed PNGs before continuing." -ForegroundColor Green
