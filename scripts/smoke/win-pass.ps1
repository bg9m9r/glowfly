# One smoke pass for scripts/smoke-windows.ps1: a fresh fixture repository, settings and git
# config, the app started on them at one zoom, then each check in order under its own id, so a
# failure names it; a check listed in SMOKE_EXPECT_FAIL only warns. Also the Mesa staging and
# the dry run. Defines functions only; uses the $script: state of smoke-windows.ps1 and the
# helpers of win-lib.ps1.

# The zoom steps the app accepts, read from glowfly-core so the two can't drift apart.
function Get-ZoomStep {
    $source = Join-Path $script:Root 'crates\glowfly-core\src\settings\zoom.rs'
    $m = [regex]::Match((Get-Content -Raw -LiteralPath $source), 'ZOOM_STEPS:\s*\[u16;\s*\d+\]\s*=\s*\[([\d,\s]+)\]')
    if (-not $m.Success) { throw "no ZOOM_STEPS in $source" }
    return @($m.Groups[1].Value -split ',' | ForEach-Object { [int] $_.Trim() })
}

# The zoom percent for scale $Scale ("1.5" is 150); throws unless it is a zoom step.
function Get-ZoomPercent([string] $Scale) {
    $value = 0.0
    $culture = [Globalization.CultureInfo]::InvariantCulture
    if (-not [double]::TryParse($Scale, [Globalization.NumberStyles]::Float, $culture, [ref] $value)) {
        throw "SMOKE_SCALES: '$Scale' is not a number"
    }
    $percent = $value * 100
    $steps = Get-ZoomStep
    if ($percent -ne [Math]::Round($percent) -or $steps -notcontains [int] $percent) {
        throw "SMOKE_SCALES: '$Scale' is not a zoom step / 100 (steps: $($steps -join ' '))"
    }
    return [int] $percent
}

# The settings a pass starts with: no welcome, release notes or update check, English labels.
function Get-SeedSetting([int] $Zoom, [string] $Version) {
    $settings = [ordered]@{
        welcome_done     = $true
        reopen_last_repo = $false
        language         = 'en'
        zoom_percent     = $Zoom
        updates          = [ordered]@{ check = $false; last_checked = $null; notes_seen = $Version }
    }
    return ($settings | ConvertTo-Json -Depth 5)
}

function Invoke-Pass([string] $Scale) {
    $script:Scale = $Scale
    $script:ZoomPercent = Get-ZoomPercent $Scale
    $script:Pass = "scale-$Scale"
    $script:PassOut = Join-Path $script:SmokeOut $script:Pass
    $script:PassDir = Join-Path $script:Work $script:Pass
    $script:Check = 'startup'
    New-Item -ItemType Directory -Force $script:PassOut, $script:PassDir | Out-Null
    Write-SmokeLog "pass at scale $Scale (zoom $($script:ZoomPercent)%)"
    try {
        Initialize-PassEnv
        New-Fixture (Join-Path $script:PassDir 'repo')
        $script:AOid = (Invoke-Git rev-list --max-parents=0 HEAD).Out
        $script:BOid = (Invoke-Git rev-parse HEAD~1).Out
        Start-App
        Invoke-Check '01' 'scale' { Test-Scale }
        Invoke-Check '02' 'first-click' { Test-FirstClick }
        Invoke-Check '03' 'commit-box' { Test-CommitBox }
        Invoke-Check '04' 'context-menu' { Test-ContextMenu }
        Invoke-Check '05' 'shortcut' { Test-Shortcut }
        Invoke-Check '06' 'drag' { Test-SidebarDrag }
        Invoke-Check '07' 'resize' { Test-WindowResize }
        $script:Check = 'stop'
        Stop-App
    } catch [SmokeFailure] {
        throw
    } catch {
        Invoke-SmokeFailure $script:Check "unexpected error: $_ at $($_.ScriptStackTrace)"
    } finally {
        Stop-App
        Restore-PassEnv
    }
    Write-SmokeLog "pass at scale $Scale passed"
}

# Settles and runs check $Id; an expected failure only warns. Takes NN-<id>.png after.
function Invoke-Check([string] $Nn, [string] $Id, [scriptblock] $Body) {
    $script:Check = $Id
    Write-SmokeLog "check $Id"
    try {
        try {
            Wait-Settled
            & $Body
        } catch [SmokeFailure] {
            throw
        } catch {
            Invoke-SmokeFailure $Id "unexpected error: $_ at $($_.ScriptStackTrace)"
        }
    } catch [SmokeFailure] {
        if (-not (Test-ExpectedFailure $Id)) { throw }
        Write-SmokeLog "check $Id failed as expected; going on"
    }
    try {
        Save-WindowShot $script:Hwnd (Join-Path $script:PassOut "$Nn-$Id.png")
    } catch {
        Write-SmokeLog "could not take screenshot $Nn-${Id}: $_"
    }
}

# The app's and git's files all inside the pass folder, and settings that keep the welcome,
# release notes and update check away.
function Initialize-PassEnv {
    $names = 'APPDATA', 'LOCALAPPDATA', 'HOME', 'GIT_CONFIG_GLOBAL', 'GIT_CONFIG_NOSYSTEM', 'GLOWFLY_SMOKE_PROBE', 'RUST_LOG', 'GALLIUM_DRIVER'
    if ($script:SavedEnv.Count -eq 0) {
        foreach ($name in $names) { $script:SavedEnv[$name] = [Environment]::GetEnvironmentVariable($name) }
    }
    $script:AppData = Join-Path $script:PassDir 'appdata'
    $script:LocalAppData = Join-Path $script:PassDir 'localappdata'
    $script:Probe = Join-Path $script:PassDir 'probe.json'
    $home_ = Join-Path $script:PassDir 'home'
    $gitconfig = Join-Path $script:PassDir 'gitconfig'
    New-Item -ItemType Directory -Force (Join-Path $script:AppData 'Glowfly'), $script:LocalAppData, $home_ | Out-Null
    $env:APPDATA = $script:AppData
    $env:LOCALAPPDATA = $script:LocalAppData
    $env:HOME = $home_
    $env:GIT_CONFIG_GLOBAL = $gitconfig
    $env:GIT_CONFIG_NOSYSTEM = '1'
    $env:GLOWFLY_SMOKE_PROBE = $script:Probe
    $env:RUST_LOG = 'glowfly=debug'
    $env:GALLIUM_DRIVER = if ($script:MesaStaged) { 'llvmpipe' } else { $null }
    $config = [ordered]@{
        'user.name' = 'Smoke'; 'user.email' = 'smoke@example.com'; 'init.defaultBranch' = 'main'
        'commit.gpgsign' = 'false'; 'core.autocrlf' = 'false'
    }
    foreach ($key in $config.Keys) { Invoke-FixtureGit config --file $gitconfig $key $config[$key] }
    $version = (Get-Content -Raw -LiteralPath (Join-Path $script:Root 'version.txt')).Trim()
    $settings = Get-SeedSetting $script:ZoomPercent $version
    [IO.File]::WriteAllText((Join-Path $script:AppData 'Glowfly\settings.json'), $settings, [Text.UTF8Encoding]::new($false))
}

# Puts back the environment from before the first pass.
function Restore-PassEnv {
    foreach ($name in @($script:SavedEnv.Keys)) {
        [Environment]::SetEnvironmentVariable($name, $script:SavedEnv[$name])
    }
    $script:SavedEnv = @{}
}

# git for the fixture: throws with git's output when it fails.
function Invoke-FixtureGit {
    $PSNativeCommandUseErrorActionPreference = $false
    $out = & git @args 2>&1
    if ($LASTEXITCODE -ne 0) { throw "git $($args -join ' ') failed: $(($out | ForEach-Object { "$_" }) -join "`n")" }
}

# One commit of $File holding $Text at a fixed time, so its id is the same on every run.
function New-FixtureCommit([int] $Second, [string] $File, [string] $Text, [string] $Message) {
    [IO.File]::WriteAllText((Join-Path $script:Repo $File), "$Text`n", [Text.UTF8Encoding]::new($false))
    Invoke-FixtureGit -C $script:Repo add $File
    $env:GIT_AUTHOR_DATE = "2024-01-01T10:00:0$Second"
    $env:GIT_COMMITTER_DATE = "2024-01-01T10:00:0$Second"
    try {
        Invoke-FixtureGit -C $script:Repo commit -q -m $Message
    } finally {
        $env:GIT_AUTHOR_DATE = $null
        $env:GIT_COMMITTER_DATE = $null
    }
}

# Three commits, a staged new file and an unstaged edit, as on Linux.
function New-Fixture([string] $Dir) {
    $script:Repo = $Dir
    Invoke-FixtureGit init -q -b main $Dir
    New-FixtureCommit 1 'a.txt' 'a' 'Add a.txt'
    New-FixtureCommit 2 'b.txt' 'b' 'Add b.txt'
    New-FixtureCommit 3 'notes.txt' ((1..20 | ForEach-Object { "line $_" }) -join "`n") 'Add notes'
    $utf8 = [Text.UTF8Encoding]::new($false)
    [IO.File]::WriteAllText((Join-Path $Dir 'todo.txt'), "buy milk`n", $utf8)
    Invoke-FixtureGit -C $Dir add todo.txt
    [IO.File]::AppendAllText((Join-Path $Dir 'notes.txt'), "line 21`n", $utf8)
}

function Start-App {
    $script:Check = 'startup'
    $log = Join-Path $script:PassOut 'app.log'
    $err = Join-Path $script:PassOut 'app.err.log'
    $script:App = Start-Process -FilePath $script:RunBinary -ArgumentList "`"$($script:Repo)`"" -PassThru `
        -RedirectStandardOutput $log -RedirectStandardError $err
    $clock = [Diagnostics.Stopwatch]::StartNew()
    while (($script:Hwnd = [GlowflySmoke.Native]::FindMainWindow($script:App.Id)) -eq [IntPtr]::Zero) {
        if ($script:App.HasExited) {
            $hint = if ($script:MesaStaged) { '' } else { '; without SMOKE_MESA_DIR a machine with no OpenGL 2+ (the hosted runner) cannot start it' }
            Invoke-SmokeFailure 'startup' "the app exited with code $($script:App.ExitCode) before its window appeared (see app.log, app.err.log)$hint"
        }
        if ($clock.Elapsed.TotalSeconds -ge 60) { Invoke-SmokeFailure 'startup' 'no window of the app appeared within 60 s (see app.log, app.err.log)' }
        Start-Sleep -Milliseconds 200
    }
    # At the work area's top-left, leaving the bottom-right corner to the thief.
    $wa = [GlowflySmoke.Native]::WorkArea()
    $w = [Math]::Min(1440, $wa[2])
    $h = [Math]::Min(880, $wa[3] - 100)
    [GlowflySmoke.Native]::Place($script:Hwnd, $wa[0], $wa[1], $w, $h)
    Write-SmokeLog "window placed at $($wa[0]), $($wa[1]), $w x $h (work area $($wa[2]) x $($wa[3]))"
    $p = Wait-Probe "the repository to load with the row 'Add notes'" {
        param($p)
        $null -ne $p.repo -and $p.busy -eq $false -and @(Select-Widget $p 'Add notes' 'central' '').Count -gt 0
    } 60
    if ($p.modal) { Invoke-SmokeFailure 'startup' 'a startup dialog is in the way' }
    if ($p.chrome -ne 'windows') { Invoke-SmokeFailure 'startup' "the app draws the '$($p.chrome)' window frame, not 'windows'" }

    # A second window to take the foreground away, in the work area's bottom-right corner.
    $thief = Join-Path $script:SmokeDir 'win-thief.ps1'
    $title = 'Glowfly smoke focus thief'
    $thiefArgs = '-NoProfile', '-WindowStyle', 'Hidden', '-File', "`"$thief`"", '-X', ($wa[0] + $wa[2] - 220), '-Y', ($wa[1] + $wa[3] - 90), '-Title', "`"$title`""
    $script:Thief = Start-Process -FilePath ([Environment]::ProcessPath) -ArgumentList $thiefArgs -PassThru
    $clock = [Diagnostics.Stopwatch]::StartNew()
    while ($true) {
        $script:ThiefHwnd = [GlowflySmoke.Native]::FindMainWindow($script:Thief.Id)
        if ($script:ThiefHwnd -ne [IntPtr]::Zero -and [GlowflySmoke.Native]::Title($script:ThiefHwnd) -eq $title) { break }
        if ($script:Thief.HasExited -or $clock.Elapsed.TotalSeconds -ge 20) {
            Invoke-SmokeFailure 'startup' 'the focus thief window (win-thief.ps1) did not appear within 20 s'
        }
        Start-Sleep -Milliseconds 200
    }
    if (-not [GlowflySmoke.Native]::BringToFront($script:Hwnd)) { Write-SmokeLog 'SetForegroundWindow did not make Glowfly the foreground window' }
    Wait-Probe 'the window to have the focus' { param($p) $p.focused -eq $true } | Out-Null
}

# Closes the app as the window's close button would, killing it after 10 s, and the thief.
function Stop-App {
    if ($script:App) {
        if (-not $script:App.HasExited) {
            $null = $script:App.CloseMainWindow()
            if (-not $script:App.WaitForExit(10000)) {
                Write-SmokeLog 'the app did not exit within 10 s of WM_CLOSE; killing it'
                $script:App.Kill()
                $null = $script:App.WaitForExit(5000)
            }
        }
        $script:App = $null
    }
    if ($script:Thief) {
        if (-not $script:Thief.HasExited) { $script:Thief.Kill() }
        $script:Thief = $null
    }
    $script:Hwnd = [IntPtr]::Zero
    $script:ThiefHwnd = [IntPtr]::Zero
}

# The Mesa files a copy of the app runs with: opengl32.dll and libgallium_wgl.dll at least.
function Test-MesaDir {
    foreach ($dll in 'opengl32.dll', 'libgallium_wgl.dll') {
        if (-not (Test-Path -LiteralPath (Join-Path $script:MesaDir $dll))) { throw "SMOKE_MESA_DIR: no $dll in $($script:MesaDir)" }
    }
}

# Copies the app and Mesa's OpenGL into WORK\bin: Windows loads opengl32.dll from beside the exe
# first, so that copy renders with llvmpipe.
function Copy-MesaBinary {
    $bin = Join-Path $script:Work 'bin'
    New-Item -ItemType Directory -Force $bin | Out-Null
    Copy-Item -LiteralPath $script:Binary $bin
    foreach ($dll in 'opengl32.dll', 'libgallium_wgl.dll', 'libglapi.dll') {
        $from = Join-Path $script:MesaDir $dll
        if (Test-Path -LiteralPath $from) { Copy-Item -LiteralPath $from $bin }
    }
    $script:RunBinary = Join-Path $bin (Split-Path -Leaf $script:Binary)
    $script:MesaStaged = $true
    Write-SmokeLog "running a copy with Mesa llvmpipe: $($script:RunBinary)"
}

# Everything the real pass relies on, without a window or input.
function Invoke-DryRun {
    $files = @(Join-Path $script:Root 'scripts\smoke-windows.ps1') + @(Get-ChildItem -LiteralPath $script:SmokeDir -Filter '*.ps1' | ForEach-Object FullName)
    foreach ($file in $files) {
        $tokens = $null
        $errors = $null
        $null = [Management.Automation.Language.Parser]::ParseFile($file, [ref] $tokens, [ref] $errors)
        if ($errors) { throw "$file does not parse: $($errors -join '; ')" }
    }
    Write-SmokeLog "parsed $($files.Count) scripts"
    $want = if ([IntPtr]::Size -eq 8) { 40 } else { 28 }
    if ([GlowflySmoke.Native]::InputSize() -ne $want) { throw "INPUT is $([GlowflySmoke.Native]::InputSize()) bytes, not $want" }
    $v = [GlowflySmoke.Native]::VirtualScreen()
    $wa = [GlowflySmoke.Native]::WorkArea()
    if ($v[2] -le 0 -or $wa[2] -le 0) { throw "no screen: virtual screen $($v -join ','), work area $($wa -join ',')" }
    Write-SmokeLog "WinInput.cs compiled; virtual screen $($v[2]) x $($v[3]), work area $($wa[2]) x $($wa[3])"
    . (Join-Path $script:SmokeDir 'win-selftest.ps1')
    Write-SmokeLog 'self-test passed'

    New-Item -ItemType Directory -Force $script:Work | Out-Null
    $script:Pass = 'dry-run'
    $script:PassDir = Join-Path $script:Work 'dry-run'
    $script:ZoomPercent = Get-ZoomPercent '1.5'
    Initialize-PassEnv
    New-Fixture (Join-Path $script:PassDir 'repo')
    $shape = [ordered]@{
        'log --format=%s'                  = "Add notes`nAdd b.txt`nAdd a.txt"
        'diff --name-only'                 = 'notes.txt'
        'diff --cached --name-only'        = 'todo.txt'
        'rev-list --max-parents=0 HEAD'    = (Invoke-Git rev-parse HEAD~2).Out
        'config user.name'                 = 'Smoke'
        'show --name-only --format= HEAD~1' = 'b.txt'
    }
    foreach ($cmd in $shape.Keys) {
        $got = (Invoke-Git @($cmd -split ' ')).Out
        if ($got -cne $shape[$cmd]) { throw "fixture: git $cmd gives '$got', not '$($shape[$cmd])'" }
    }
    $seeded = Read-JsonFile (Join-Path $script:AppData 'Glowfly\settings.json')
    if ($seeded.zoom_percent -ne 150) { throw "seeded settings: zoom_percent is $($seeded.zoom_percent)" }
    Restore-PassEnv
    Write-SmokeLog "fixture checked: $((Invoke-Git log --format='%h %s').Out -replace "`n", ', ')"
}
