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
    # The tests run with the folders Glowfly keeps its own files in (APPDATA, LOCALAPPDATA and
    # XDG_* on Windows and Linux; HOME on macOS) and HOME, where git and libgit2 find the global
    # git config, pointed at a fresh temporary folder, so a test that gets past runner::testing's
    # stand-ins can't change the developer's settings, recent repositories, tabs, themes, caches,
    # logs or global git config. HOME is used rather than GIT_CONFIG_GLOBAL because libgit2
    # ignores that variable (and the fixtures then send every command to the CLI). The system git
    # config is left as it is: it isn't the developer's, and tests need what it sets up (Git for
    # Windows' LFS filter). The temporary global config holds only an identity, as CI's does, for
    # tests that commit without setting one. Cargo and rustup keep their folders under the real HOME.
    if (-not $env:CARGO_HOME) { $env:CARGO_HOME = Join-Path $HOME ".cargo" }
    if (-not $env:RUSTUP_HOME) { $env:RUSTUP_HOME = Join-Path $HOME ".rustup" }
    $names = "APPDATA", "LOCALAPPDATA", "XDG_CONFIG_HOME", "XDG_STATE_HOME", "XDG_CACHE_HOME", "HOME"
    $saved = @{}
    foreach ($n in $names) { $saved[$n] = [Environment]::GetEnvironmentVariable($n) }
    $folders = Join-Path ([IO.Path]::GetTempPath()) "glowfly-check-$PID-$([Guid]::NewGuid().ToString('N').Substring(0, 8))"
    try {
        foreach ($n in $names) {
            $dir = New-Item -ItemType Directory -Force (Join-Path $folders $n.ToLowerInvariant())
            [Environment]::SetEnvironmentVariable($n, $dir.FullName)
        }
        $identity = "[user]`n`tname = Glowfly Tests`n`temail = tests@example.invalid`n"
        Set-Content -LiteralPath (Join-Path $env:HOME ".gitconfig") -Encoding utf8NoBOM -NoNewline -Value $identity
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
