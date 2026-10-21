#Requires -Version 7.2
<#
.SYNOPSIS
Real-input smoke tests for the Windows build.

.DESCRIPTION
Drives the release build on the desktop it runs on with real input through SendInput
(scripts/smoke/WinInput.cs): a click that focuses the window, typing in the commit box and
committing, a context menu, a keyboard shortcut, a drag of the sidebar's edge and a drag of the
window's own resize corner. It runs one full pass per scale in SMOKE_SCALES, each through the
app's zoom on a fresh fixture repository, settings and git config. The app reports what it shows
through its opt-in probe file (GLOWFLY_SMOKE_PROBE, crates/glowfly-app/src/ui/probe.rs).

Real input goes to the desktop in use, so the real pass runs only in CI (GITHUB_ACTIONS=true) or
with -OnThisDesktop. -DryRun never starts the app or sends input: it parses the scripts, compiles
the Win32 helpers, runs the self-test and checks the fixture.

Environment:
  SMOKE_OUT          screenshots and, on failure, logs (default: <target>\smoke-windows)
  SMOKE_SCALES       scales to run, each a zoom step / 100 (default: "1 1.5")
  SMOKE_TIMEOUT      seconds any one wait may take (default: 20)
  SMOKE_EXPECT_FAIL  check ids whose failure is only a warning (default: none)
  SMOKE_MESA_DIR     a folder with Mesa's opengl32.dll and libgallium_wgl.dll, for a machine
                     without OpenGL 2+ (the hosted runner)

Exit codes: 0 all passed, 1 a check failed, 2 a usage, setup or environment error.
#>
[CmdletBinding()]
param(
    # The Glowfly executable (default: <CARGO_TARGET_DIR or target>\release\glowfly.exe).
    [string] $Binary,
    # Checks the scripts, the helpers and the fixture without a window or input.
    [switch] $DryRun,
    # Runs the real pass outside CI, on the desktop in use.
    [switch] $OnThisDesktop
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if (-not $IsWindows) {
    [Console]::Error.WriteLine('smoke-windows.ps1 runs only on Windows (it drives a real desktop)')
    exit 2
}

$script:Root = Split-Path -Parent $PSScriptRoot
$script:SmokeDir = Join-Path $PSScriptRoot 'smoke'
$here = (Get-Location).Path
$target = if ($env:CARGO_TARGET_DIR) { [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR, $script:Root) } else { Join-Path $script:Root 'target' }
$script:Binary = if ($Binary) { [IO.Path]::GetFullPath($Binary, $here) } else { Join-Path $target 'release\glowfly.exe' }
$script:SmokeOut = if ($env:SMOKE_OUT) { [IO.Path]::GetFullPath($env:SMOKE_OUT, $here) } else { Join-Path $target 'smoke-windows' }
$scaleText = if ($env:SMOKE_SCALES) { $env:SMOKE_SCALES } else { '1 1.5' }
$script:Scales = @($scaleText -split '\s+' | Where-Object { $_ })
$script:SmokeTimeout = if ($env:SMOKE_TIMEOUT) { [int] $env:SMOKE_TIMEOUT } else { 20 }
$script:ExpectFail = "$env:SMOKE_EXPECT_FAIL"
$script:MesaDir = "$env:SMOKE_MESA_DIR"

# The state the helpers share; every name is set here, as strict mode wants.
$script:Work = Join-Path ([IO.Path]::GetTempPath()) "glowfly-smoke-$([guid]::NewGuid().ToString('N').Substring(0, 8))"
$script:RunBinary = $script:Binary
$script:MesaStaged = $false
$script:Pass = 'setup'
$script:PassOut = $script:SmokeOut
$script:PassDir = $script:Work
$script:Check = 'setup'
$script:Scale = ''
$script:ZoomPercent = 100
$script:Probe = $null
$script:Repo = $null
$script:AOid = ''
$script:BOid = ''
$script:App = $null
$script:Thief = $null
$script:Hwnd = [IntPtr]::Zero
$script:ThiefHwnd = [IntPtr]::Zero
$script:AppData = $null
$script:LocalAppData = $null
$script:SavedEnv = @{}
$script:Reporting = $true
$script:ClientOrigin = $null

. (Join-Path $script:SmokeDir 'win-lib.ps1')
. (Join-Path $script:SmokeDir 'win-pass.ps1')
. (Join-Path $script:SmokeDir 'win-checks.ps1')
if (-not ('GlowflySmoke.Native' -as [type])) {
    Add-Type -Path (Join-Path $script:SmokeDir 'WinInput.cs')
}
Add-Type -AssemblyName System.Drawing.Common

if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    [Console]::Error.WriteLine('git is not on PATH')
    exit 2
}

if ($DryRun) {
    $status = 0
    try {
        Invoke-DryRun
        Write-SmokeLog 'dry run passed'
    } catch {
        Write-Host "::error::smoke dry run: $_"
        Write-Host $_.ScriptStackTrace
        $status = 1
    } finally {
        Restore-PassEnv
        Remove-Item -LiteralPath $script:Work -Recurse -Force -ErrorAction SilentlyContinue
    }
    exit $status
}

if ($env:GITHUB_ACTIONS -ne 'true' -and -not $OnThisDesktop) {
    [Console]::Error.WriteLine('smoke-windows.ps1 sends real mouse and keyboard input to this desktop; it runs in CI, or here with -OnThisDesktop (use -DryRun to check the scripts)')
    exit 2
}
if (-not (Test-Path -LiteralPath $script:Binary -PathType Leaf)) {
    [Console]::Error.WriteLine("no Glowfly at $($script:Binary) (build it with cargo build --release -p glowfly)")
    exit 2
}
try {
    foreach ($s in $script:Scales) { $null = Get-ZoomPercent $s }
    if ($script:MesaDir) { Test-MesaDir }
} catch {
    [Console]::Error.WriteLine("$_")
    exit 2
}

# Only this script's own results are cleared: SMOKE_OUT may hold other files.
New-Item -ItemType Directory -Force $script:SmokeOut | Out-Null
foreach ($old in @(Get-ChildItem -LiteralPath $script:SmokeOut -Directory -Filter 'scale-*') + @('work', 'first-click.txt')) {
    $path = if ($old -is [IO.FileSystemInfo]) { $old.FullName } else { Join-Path $script:SmokeOut $old }
    Remove-Item -LiteralPath $path -Recurse -Force -ErrorAction SilentlyContinue
}
New-Item -ItemType Directory -Force $script:Work | Out-Null

$status = 0
$oldLockTimeout = $null
try {
    if ($env:GITHUB_ACTIONS -eq 'true') {
        if (-not [GlowflySmoke.Native]::SetScreenSize(1920, 1080, 1280, 900)) {
            Write-SmokeLog 'no display mode of at least 1280 x 900 up to 1920 x 1080; going on with the window sizes clamped'
        }
        $v = [GlowflySmoke.Native]::VirtualScreen()
        Write-SmokeLog "screen $($v[2]) x $($v[3])"
    }
    # SetForegroundWindow from a background process needs this; the system may refuse it.
    try {
        $oldLockTimeout = [GlowflySmoke.Native]::ForegroundLockTimeout
        [GlowflySmoke.Native]::ForegroundLockTimeout = 0
    } catch {
        Write-SmokeLog "could not turn off the foreground lock timeout ($_); bringing windows to the front may fail"
    }
    if ($script:MesaDir) { Copy-MesaBinary }
    foreach ($s in $script:Scales) { Invoke-Pass $s }
    $script:Pass = ''
    Write-SmokeLog "all checks passed: $(($script:Scales | ForEach-Object { "scale $_" }) -join ', ')"
} catch [SmokeFailure] {
    $status = 1
} catch {
    $status = 1
    try {
        Invoke-SmokeFailure 'setup' "unexpected error: $_ at $($_.ScriptStackTrace)"
    } catch [SmokeFailure] {
        Write-SmokeLog 'setup failed'
    }
} finally {
    Stop-App
    Restore-PassEnv
    try { [GlowflySmoke.Native]::RestoreScreen() } catch { Write-SmokeLog "could not restore the display mode: $_" }
    if ($null -ne $oldLockTimeout) {
        try { [GlowflySmoke.Native]::ForegroundLockTimeout = $oldLockTimeout } catch { Write-SmokeLog "could not restore the foreground lock timeout: $_" }
    }
    if ($status -ne 0) {
        # Kept for the artifact, minus the copied binary and Mesa.
        Remove-Item -LiteralPath (Join-Path $script:Work 'bin') -Recurse -Force -ErrorAction SilentlyContinue
        Move-Item -LiteralPath $script:Work (Join-Path $script:SmokeOut 'work') -ErrorAction SilentlyContinue
        [Console]::Error.WriteLine("smoke: failed; screenshots and logs in $($script:SmokeOut)")
    } else {
        Remove-Item -LiteralPath $script:Work -Recurse -Force -ErrorAction SilentlyContinue
    }
}
exit $status
