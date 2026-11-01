# Format check, lint (warnings are errors), and tests. Run before committing.
#
# The tests run on RUST_TEST_THREADS threads; when it isn't set, on twice the logical cores (at
# most 32). The suites mostly wait on the git processes they start rather than on the CPU, so
# more threads than cores finish sooner. Set it lower when other builds share the machine.
#
# -Part runs a share of the check, for CI's Windows jobs: `lint` (fmt and clippy), `core`
# (glowfly-core's tests), `flows` (glowfly-flows') and `rest` (every other test binary). A test
# part still builds every test binary as a full run does (`--workspace`, so the features and the
# build cache are the same), then runs only its own. With no -Part it runs everything.
param(
    [ValidateSet('all', 'lint', 'core', 'flows', 'rest')]
    [string[]]$Part = @('all')
)
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

if (-not $env:RUST_TEST_THREADS) {
    $env:RUST_TEST_THREADS = [string][Math]::Min(2 * [Environment]::ProcessorCount, 32)
}
$all = $Part -contains 'all'

if ($all -or $Part -contains 'lint') {
    cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) { Write-Error "cargo fmt found unformatted code (run: cargo fmt --all)" }

    cargo clippy --workspace --all-targets -- -D warnings
    if ($LASTEXITCODE -ne 0) { Write-Error "clippy failed" }
}

if ($all) {
    Write-Host "Test threads: $env:RUST_TEST_THREADS"
    cargo test --workspace
    if ($LASTEXITCODE -ne 0) { Write-Error "tests failed" }
} elseif ($Part | Where-Object { $_ -ne 'lint' }) {
    Write-Host "Test threads: $env:RUST_TEST_THREADS"
    $built = cargo test --workspace --no-run --message-format=json-render-diagnostics
    if ($LASTEXITCODE -ne 0) { Write-Error "the test build failed" }
    $failed = @()
    foreach ($line in $built) {
        if (-not $line.StartsWith('{')) { continue }
        $unit = $line | ConvertFrom-Json
        if ($unit.reason -ne 'compiler-artifact' -or -not $unit.profile.test -or -not $unit.executable) { continue }
        $crate = Split-Path $unit.manifest_path
        $share = switch (Split-Path $crate -Leaf) { 'glowfly-core' { 'core' } 'glowfly-flows' { 'flows' } default { 'rest' } }
        if ($Part -notcontains $share) { continue }
        # As cargo runs a test binary: from its crate's folder, which it also names.
        Write-Host "     Running $($unit.target.name) ($($unit.executable))" -ForegroundColor Green
        Push-Location $crate
        $env:CARGO_MANIFEST_DIR = $crate
        & $unit.executable
        if ($LASTEXITCODE -ne 0) { $failed += $unit.executable }
        Pop-Location
    }
    if ($failed) { Write-Error "tests failed in: $($failed -join ', ')" }
}

Write-Host "All checks passed." -ForegroundColor Green
