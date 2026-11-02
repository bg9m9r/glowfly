# Format check, lint (warnings are errors), and tests. Run before committing.
#
#   pwsh scripts/check.ps1                 # everything: the one full check
#   pwsh scripts/check.ps1 -Only <part>    # one part, as CI's parallel Windows jobs run it
#
# The parts add up to the full check: lint (fmt and clippy), core (glowfly-core's tests), flows
# (glowfly-flows' tests) and rest (every other crate's tests).
param(
    [ValidateSet('all', 'lint', 'core', 'flows', 'rest')]
    [string]$Only = 'all'
)
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

if ($Only -eq 'all' -or $Only -eq 'lint') {
    cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) { Write-Error "cargo fmt found unformatted code (run: cargo fmt --all)" }

    cargo clippy --workspace --all-targets -- -D warnings
    if ($LASTEXITCODE -ne 0) { Write-Error "clippy failed" }
}

$testArgs = switch ($Only) {
    'all' { , @('--workspace') }
    'core' { , @('-p', 'glowfly-core') }
    'flows' { , @('-p', 'glowfly-flows') }
    'rest' { , @('--workspace', '--exclude', 'glowfly-core', '--exclude', 'glowfly-flows') }
    default { $null }
}

if ($testArgs) {
    # The tests run with Glowfly's config, cache, state and log folders pointed at a fresh temporary
    # folder, so no test can read or write the developer's own settings, recent repositories, tabs,
    # themes, caches or logs, even one that gets past runner::testing's stand-ins.
    $names = "APPDATA", "LOCALAPPDATA", "XDG_CONFIG_HOME", "XDG_STATE_HOME", "XDG_CACHE_HOME"
    $saved = @{}
    foreach ($n in $names) { $saved[$n] = [Environment]::GetEnvironmentVariable($n) }
    $folders = Join-Path ([IO.Path]::GetTempPath()) "glowfly-check-$PID-$([Guid]::NewGuid().ToString('N').Substring(0, 8))"
    try {
        foreach ($n in $names) {
            $dir = New-Item -ItemType Directory -Force (Join-Path $folders $n.ToLowerInvariant())
            [Environment]::SetEnvironmentVariable($n, $dir.FullName)
        }
        cargo test @testArgs
        $tested = $LASTEXITCODE
    } finally {
        foreach ($n in $names) { [Environment]::SetEnvironmentVariable($n, $saved[$n]) }
        Remove-Item -Recurse -Force $folders -ErrorAction SilentlyContinue
    }
    if ($tested -ne 0) { Write-Error "tests failed" }
}

if ($Only -eq 'all') {
    Write-Host "All checks passed." -ForegroundColor Green
} else {
    Write-Host "Check part '$Only' passed." -ForegroundColor Green
}
