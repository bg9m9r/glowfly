# Format check, lint (warnings are errors), and tests. Run before committing.
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) { Write-Error "cargo fmt found unformatted code (run: cargo fmt --all)" }

cargo clippy --workspace --all-targets -- -D warnings
if ($LASTEXITCODE -ne 0) { Write-Error "clippy failed" }

cargo test --workspace
if ($LASTEXITCODE -ne 0) { Write-Error "tests failed" }

Write-Host "All checks passed." -ForegroundColor Green
