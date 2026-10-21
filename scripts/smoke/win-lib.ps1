# Helpers for scripts/smoke-windows.ps1: logging and the failure report, waiting on the app's
# probe file, its settings and the disk, finding widgets by their accessible names, and real
# input through GlowflySmoke.Native (WinInput.cs, SendInput only). Defines functions only.
#
# Uses the $script: state smoke-windows.ps1 and win-pass.ps1 set: SmokeTimeout, ExpectFail,
# Pass, PassOut, Check, Probe, Repo, App, Hwnd, AppData, LocalAppData, Reporting, ClientOrigin.

class SmokeFailure : System.Exception {
    [string] $Id
    SmokeFailure([string] $id, [string] $message) : base($message) { $this.Id = $id }
}

function Write-SmokeLog([string] $Message) {
    Write-Host "[smoke $(Get-Date -Format 'HH:mm:ss') $($script:Pass)] $Message"
}

# Whether check $Id is listed in SMOKE_EXPECT_FAIL.
function Test-ExpectedFailure([string] $Id) {
    $ids = @("$($script:ExpectFail)" -split '\s+' | Where-Object { $_ })
    return $ids -contains $Id
}

# Reports check $Id as failed (once per check: the report, the annotation), then throws a
# SmokeFailure. An expected failure reports into expected-<id>\ as a warning.
function Invoke-SmokeFailure([string] $Id, [string] $Message) {
    $level = 'error'
    $dir = $script:PassOut
    if (Test-ExpectedFailure $Id) {
        $level = 'warning'
        $dir = Join-Path $script:PassOut "expected-$Id"
    }
    if ($script:Reporting) {
        New-Item -ItemType Directory -Force $dir | Out-Null
        $marker = Join-Path $dir ".reported-$Id"
        if (-not (Test-Path -LiteralPath $marker)) {
            New-Item -ItemType File $marker | Out-Null
            Save-FailureReport $dir
            Write-Host "::${level}::smoke $($script:Pass), check ${Id}: $Message"
            Write-SmokeLog "FAILED ${Id}: $Message"
        }
    }
    throw [SmokeFailure]::new($Id, $Message)
}

# Everything a failure report holds, into folder $Dir; each part on its own.
function Save-FailureReport([string] $Dir) {
    $steps = @(
        { if ($script:Hwnd -and $script:Hwnd -ne [IntPtr]::Zero) { Save-WindowShot $script:Hwnd (Join-Path $Dir 'failure.png') } },
        { Save-ScreenShot (Join-Path $Dir 'screen.png') },
        { if ($script:Probe -and (Test-Path -LiteralPath $script:Probe)) { Copy-Item -LiteralPath $script:Probe (Join-Path $Dir 'probe.json') } },
        {
            if ($Dir -ne $script:PassOut) {
                foreach ($log in 'app.log', 'app.err.log') {
                    $from = Join-Path $script:PassOut $log
                    if (Test-Path -LiteralPath $from) { Copy-Item -LiteralPath $from (Join-Path $Dir $log) }
                }
            }
        },
        {
            $logs = Join-Path $script:LocalAppData 'Glowfly\logs'
            if ($script:LocalAppData -and (Test-Path -LiteralPath $logs)) { Copy-Item -Path (Join-Path $logs '*') -Destination $Dir }
        },
        {
            $file = Join-Path $script:AppData 'Glowfly\settings.json'
            if ($script:AppData -and (Test-Path -LiteralPath $file)) { Copy-Item -LiteralPath $file (Join-Path $Dir 'settings.json') }
        },
        {
            if ($script:Repo -and (Test-Path -LiteralPath (Join-Path $script:Repo '.git'))) {
                (Invoke-Git log --all --oneline).Out | Set-Content -LiteralPath (Join-Path $Dir 'git-log.txt')
                (Invoke-Git status).Out | Set-Content -LiteralPath (Join-Path $Dir 'git-status.txt')
            }
        }
    )
    foreach ($step in $steps) {
        try { & $step } catch { Write-SmokeLog "report: $_" }
    }
}

# The window's client area as composed (even while covered) into a PNG.
function Save-WindowShot([IntPtr] $Hwnd, [string] $Path) {
    $r = [GlowflySmoke.Native]::ClientOnScreen($Hwnd)
    if ($r[2] -le 0 -or $r[3] -le 0) { return }
    $bmp = [System.Drawing.Bitmap]::new($r[2], $r[3])
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    try {
        $hdc = $g.GetHdc()
        $ok = [GlowflySmoke.Native]::PrintTo($Hwnd, $hdc)
        $g.ReleaseHdc($hdc)
        if (-not $ok) { $g.CopyFromScreen($r[0], $r[1], 0, 0, $bmp.Size) }
        $bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $g.Dispose()
        $bmp.Dispose()
    }
}

# The whole virtual screen into a PNG.
function Save-ScreenShot([string] $Path) {
    $v = [GlowflySmoke.Native]::VirtualScreen()
    $bmp = [System.Drawing.Bitmap]::new($v[2], $v[3])
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    try {
        $g.CopyFromScreen($v[0], $v[1], 0, 0, $bmp.Size)
        $bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $g.Dispose()
        $bmp.Dispose()
    }
}

# A JSON file parsed, read with sharing that never blocks its writer; $null while it is missing,
# held or unreadable.
function Read-JsonFile([string] $Path) {
    try {
        $share = [IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete
        $stream = [IO.FileStream]::new($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, $share)
        try {
            $text = [IO.StreamReader]::new($stream, [Text.UTF8Encoding]::new($false)).ReadToEnd()
        } finally {
            $stream.Dispose()
        }
        return $text | ConvertFrom-Json
    } catch {
        # Missing, held by a sharing race, or not JSON (yet).
        return $null
    }
}

function Read-Probe { Read-JsonFile $script:Probe }

# What a timeout message shows of the probe.
function Get-ProbeState {
    $p = Read-Probe
    if ($null -eq $p) { return '(no probe file)' }
    $keys = 'seq', 'focused', 'busy', 'selection', 'modal', 'context_menu', 'popup', 'keyboard_focus', 'draft_summary',
    'screen', 'pixels_per_point', 'native_pixels_per_point', 'panels', 'toasts'
    return ($p | Select-Object $keys | ConvertTo-Json -Compress -Depth 5)
}

# The app must still be running.
function Test-AppAlive {
    if ($script:App -and $script:App.HasExited) {
        Invoke-SmokeFailure $script:Check 'the app exited (see app.log, app.err.log)'
    }
}

# Waits until & $Test <value> is true, $Read giving the value every 100 ms.
function Wait-Until([string] $Desc, [scriptblock] $Read, [scriptblock] $Test, [scriptblock] $State, [int] $Seconds) {
    if ($Seconds -le 0) { $Seconds = $script:SmokeTimeout }
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $lastError = ''
    while ($true) {
        Test-AppAlive
        $value = & $Read
        $passed = $false
        if ($null -ne $value) {
            try { $passed = [bool] (& $Test $value) } catch { $lastError = "; the test failed with: $_" }
        }
        if ($passed) { return $value }
        if ($clock.Elapsed.TotalSeconds -ge $Seconds) {
            Invoke-SmokeFailure $script:Check "timed out after ${Seconds}s waiting for $Desc$(& $State)$lastError"
        }
        Start-Sleep -Milliseconds 100
    }
}

# Waits until & $Test <probe> is true; returns that probe.
function Wait-Probe([string] $Desc, [scriptblock] $Test, [int] $Seconds = 0) {
    Wait-Until $Desc { Read-Probe } $Test { "; probe: $(Get-ProbeState)" } $Seconds
}

# Waits until & $Test <settings.json> is true.
function Wait-SettingsFile([string] $Desc, [scriptblock] $Test) {
    $file = Join-Path $script:AppData 'Glowfly\settings.json'
    Wait-Until $Desc { Read-JsonFile $file } $Test { ' in settings.json' } 0 | Out-Null
}

# Waits until & $Test is true; for checks on the repository through git.
function Wait-Disk([string] $Desc, [scriptblock] $Test) {
    # Its own name: inside Wait-Until, $Test is Wait-Until's parameter.
    $onDisk = $Test
    Wait-Until $Desc { $true } { & $onDisk } { '' } 0 | Out-Null
}

# Whether & $Test <probe> becomes true within $Seconds, without failing the check.
function Test-ProbeWithin([scriptblock] $Test, [double] $Seconds) {
    $clock = [Diagnostics.Stopwatch]::StartNew()
    while ($clock.Elapsed.TotalSeconds -lt $Seconds) {
        Test-AppAlive
        $value = Read-Probe
        if ($null -ne $value) {
            try { if (& $Test $value) { return $true } } catch { $null = $_ }
        }
        Start-Sleep -Milliseconds 100
    }
    return $false
}

# Before every step: nothing running and the probe unchanged for three polls 100 ms apart.
function Wait-Settled {
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $last = $null
    $stable = 0
    while ($stable -lt 3) {
        Test-AppAlive
        $p = Read-Probe
        $now = if ($null -eq $p) { $null } elseif ($p.busy) { 'busy' } else { "$($p.seq)" }
        if ($null -ne $now -and $now -ne 'busy' -and $now -eq $last) { $stable++ } else { $stable = 0 }
        $last = $now
        if ($clock.Elapsed.TotalSeconds -ge $script:SmokeTimeout) {
            Invoke-SmokeFailure $script:Check "the app did not settle (busy, or the probe kept changing); probe: $(Get-ProbeState)"
        }
        Start-Sleep -Milliseconds 100
    }
}

# A widget's name as the scripts compare it: its label, else its value.
function Get-WidgetName($Widget) {
    if ($null -ne $Widget.label) { return [string] $Widget.label }
    return $Widget.value
}

# The widgets of $Probe named $Label (or starting with the text after "prefix:"), in $Region
# ("any" for all), with $Role when it is given.
function Select-Widget($Probe, [string] $Label, [string] $Region, [string] $Role) {
    $prefix = $Label.StartsWith('prefix:')
    $text = if ($prefix) { $Label.Substring(7) } else { $Label }
    @($Probe.widgets | Where-Object {
            $name = Get-WidgetName $_
            ($null -ne $name) -and ($(if ($prefix) { $name.StartsWith($text, [StringComparison]::Ordinal) } else { $name -ceq $text })) -and
            ($Region -eq 'any' -or $_.region -eq $Region) -and (-not $Role -or $_.role -eq $Role)
        })
}

# Where client point (x, y) in points is on screen, in pixels.
function ConvertTo-ScreenPoint($Probe, [double] $X, [double] $Y) {
    $origin = if ($script:ClientOrigin) { $script:ClientOrigin } else { [GlowflySmoke.Native]::ClientOnScreen($script:Hwnd) }
    $ppp = [double] $Probe.pixels_per_point
    return @{ X = $origin[0] + [int][Math]::Floor($X * $ppp + 0.5); Y = $origin[1] + [int][Math]::Floor($Y * $ppp + 0.5) }
}

# The centre of the one widget named $Label in $Region, as screen pixels @{ X; Y }. Fails
# unless exactly one matches (-First takes the first of many).
function Find-Widget([string] $Label, [string] $Region, [string] $Role = '', [switch] $First, $Probe = $null) {
    if ($null -eq $Probe) { $Probe = Read-Probe }
    if ($null -eq $Probe) { Invoke-SmokeFailure $script:Check 'no probe file to find widgets in' }
    $found = @(Select-Widget $Probe $Label $Region $Role)
    if ($found.Count -eq 0 -or ($found.Count -gt 1 -and -not $First)) {
        $names = @($Probe.widgets | Where-Object { $Region -eq 'any' -or $_.region -eq $Region } | ForEach-Object { Get-WidgetName $_ })
        $roleText = if ($Role) { $Role } else { 'any role' }
        Invoke-SmokeFailure $script:Check "$($found.Count) widgets named '$Label' ($roleText) in $Region; names there: $($names | ConvertTo-Json -Compress)"
    }
    $r = $found[0].rect
    return ConvertTo-ScreenPoint $Probe (($r[0] + $r[2]) / 2) (($r[1] + $r[3]) / 2)
}

# Waits until a widget named $Label is listed in $Region.
function Wait-Widget([string] $Label, [string] $Region, [string] $Role = '') {
    Wait-Probe "a widget named '$Label' in $Region" { param($p) @(Select-Widget $p $Label $Region $Role).Count -gt 0 } | Out-Null
}

# Moves the pointer onto the widget.
function Move-PointerTo([string] $Label, [string] $Region, [string] $Role = '') {
    $at = Find-Widget $Label $Region $Role
    [GlowflySmoke.Native]::MoveTo($at.X, $at.Y)
}

# Moves onto the widget and clicks it with $Button (1 left, 3 right).
function Invoke-WidgetClick([string] $Label, [string] $Region, [int] $Button = 1, [string] $Role = '') {
    Move-PointerTo $Label $Region $Role
    Start-Sleep -Milliseconds 100
    [GlowflySmoke.Native]::Button($Button, $true)
    Start-Sleep -Milliseconds 50
    [GlowflySmoke.Native]::Button($Button, $false)
}

# Presses the left button at screen pixel (x, y), moves by (dx, dy) in $Steps moves 50 ms apart,
# releases.
function Invoke-PointerDrag([int] $X, [int] $Y, [int] $Dx, [int] $Dy, [int] $Steps, [int] $PauseAfterPress = 100) {
    [GlowflySmoke.Native]::MoveTo($X, $Y)
    Start-Sleep -Milliseconds 200
    [GlowflySmoke.Native]::Button(1, $true)
    Start-Sleep -Milliseconds $PauseAfterPress
    for ($i = 1; $i -le $Steps; $i++) {
        [GlowflySmoke.Native]::MoveTo($X + [int][Math]::Truncate($Dx * $i / $Steps), $Y + [int][Math]::Truncate($Dy * $i / $Steps))
        Start-Sleep -Milliseconds 50
    }
    Start-Sleep -Milliseconds 100
    [GlowflySmoke.Native]::Button(1, $false)
}

# Pixels for a length in points, rounded as the Linux script's px.
function ConvertTo-Px([double] $Points, $Probe = $null) {
    if ($null -eq $Probe) { $Probe = Read-Probe }
    return [int][Math]::Floor($Points * [double] $Probe.pixels_per_point + 0.5)
}

# Types $Text as real key presses, 40 ms apart.
function Send-Text([string] $Text) { [GlowflySmoke.Native]::Text($Text, 40) }

# Presses the virtual keys as a chord (down in order, up in reverse).
function Send-Chord([uint16[]] $Keys) { [GlowflySmoke.Native]::Chord($Keys) }

# git in the fixture repository: @{ Out = trimmed output; Code = exit code }, never throwing.
function Invoke-Git {
    $PSNativeCommandUseErrorActionPreference = $false
    $out = & git -C $script:Repo @args 2>&1
    $code = $LASTEXITCODE
    return @{ Out = (($out | ForEach-Object { "$_" }) -join "`n").Trim(); Code = $code }
}
