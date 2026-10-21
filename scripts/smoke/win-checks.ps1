# The checks of one smoke pass for scripts/smoke-windows.ps1, run in order by win-pass.ps1's
# Invoke-Pass. The labels and probe fields looked up here are dry-run on every OS by the journey
# test crates/trellis-app/src/journey/probe.rs (with the drawn Windows frame at zoom 150); keep
# the two in step. Defines functions only.

# Virtual keys (the runner's layout is en-US). Alt is never sent.
$script:VkReturn = 0x0D
$script:VkShift = 0x10
$script:VkControl = 0x11
$script:VkL = 0x4C
# The menu item "Create branch here" with its ellipsis (U+2026), spelled by code point so this
# file stays ASCII.
$script:CreateBranchLabel = "Create branch here$([char] 0x2026)"

# The zoom is the scale (the runner's monitor is fixed at 96 DPI), and the monitor's own scale
# is GetDpiForWindow / 96.
function Test-Scale {
    $zoom = $script:ZoomPercent
    $dpi = [TrellisSmoke.Native]::Dpi($script:Hwnd)
    $native = $dpi / 96.0
    Wait-Probe "zoom_percent $zoom, native_pixels_per_point $native (GetDpiForWindow $dpi / 96) and pixels_per_point $native x $($zoom / 100)" {
        param($p)
        $p.zoom_percent -eq $zoom -and [Math]::Abs($p.native_pixels_per_point - $native) -le 0.01 -and
        [Math]::Abs($p.pixels_per_point - $native * $zoom / 100) -le 0.01
    } | Out-Null
}

# Whether a click just sent reached the app: the window got the focus, and then (within a short
# grace) the selection became $Want.
function Measure-Click([string] $Want) {
    $focused = Test-ProbeWithin { param($p) $p.focused -eq $true } $script:SmokeTimeout
    if (-not $focused) { return 'DROPPED (the window never got the focus)' }
    if (Test-ProbeWithin { param($p) $p.selection -eq $Want } 3) { return 'delivered' }
    return 'DROPPED'
}

# The click that gives the window the focus must also reach it, (a) when the click itself
# focuses the window and (b) right after the OS activated it. Both results are recorded (the
# Input reliability evidence) before the check can fail.
function Test-FirstClick {
    $results = [ordered]@{}
    $null = [TrellisSmoke.Native]::BringToFront($script:ThiefHwnd)
    Wait-Probe '(a) the window to lose the focus to the thief' { param($p) $p.focused -eq $false } | Out-Null
    Move-PointerTo 'Add b.txt' 'central'
    Start-Sleep -Milliseconds 200
    [TrellisSmoke.Native]::Button(1, $true)
    Start-Sleep -Milliseconds 50
    [TrellisSmoke.Native]::Button(1, $false)
    $results['click to focus'] = Measure-Click "commit:$($script:BOid)"

    $null = [TrellisSmoke.Native]::BringToFront($script:ThiefHwnd)
    Wait-Probe '(b) the window to lose the focus to the thief' { param($p) $p.focused -eq $false } | Out-Null
    if (-not [TrellisSmoke.Native]::BringToFront($script:Hwnd)) { Write-SmokeLog '(b) SetForegroundWindow did not make Trellis the foreground window' }
    Wait-Probe '(b) focus from SetForegroundWindow' { param($p) $p.focused -eq $true } | Out-Null
    Wait-Settled
    Invoke-WidgetClick 'Add a.txt' 'central'
    $results['click after SetForegroundWindow'] = Measure-Click "commit:$($script:AOid)"

    $record = Join-Path $script:SmokeOut 'first-click.txt'
    foreach ($case in $results.Keys) {
        $key = $case -replace ' ', '-'
        Add-Content -LiteralPath $record -Value "$($script:Pass) ${key}: $($results[$case])"
        Write-Host "::notice::smoke $($script:Pass) first-click, ${case}: $($results[$case])"
    }
    $dropped = @($results.Keys | Where-Object { $results[$_] -ne 'delivered' })
    if ($dropped.Count -gt 0) {
        Invoke-SmokeFailure $script:Check "the first click was dropped: $(($results.Keys | ForEach-Object { "${_}: $($results[$_])" }) -join '; ')"
    }
}

# Typing real keys into the commit summary, then committing the staged file only.
function Test-CommitBox {
    $summary = "Smoke: typed with real keys (scale $($script:Scale))"
    Invoke-WidgetClick 'Uncommitted changes' 'central'
    Wait-Probe "the working tree's row to be selected" { param($p) $p.selection -eq 'wip' } | Out-Null
    Wait-Settled
    Invoke-WidgetClick 'Commit summary' 'details' -Role 'TextInput'
    Wait-Probe 'the commit summary to have the keyboard focus' {
        param($p)
        $null -ne $p.keyboard_focus -and $p.keyboard_focus.label -eq 'Commit summary'
    } | Out-Null
    Send-Text $summary
    Wait-Probe "the typed summary to read exactly '$summary' (dropped, doubled or wrongly shifted keys)" {
        param($p)
        $p.draft_summary -ceq $summary
    } | Out-Null
    Wait-Settled
    Invoke-WidgetClick 'prefix:Commit 1 file' 'details'
    Wait-Probe "the toast 'Commit succeeded'" {
        param($p)
        @($p.toasts | Where-Object { $_.ok -and "$($_.text)".StartsWith('Commit succeeded') }).Count -gt 0
    } | Out-Null
    Wait-Disk "the commit's summary on disk to be '$summary'" { (Invoke-Git log -1 --format=%s).Out -ceq $summary }
    $files = (Invoke-Git show --name-only --format= HEAD).Out
    if ($files -cne 'todo.txt') { Invoke-SmokeFailure $script:Check "the commit holds '$($files -replace "`n", ' ')', not only todo.txt" }
    $changed = (Invoke-Git diff --name-only).Out
    if ($changed -cne 'notes.txt') { Invoke-SmokeFailure $script:Check "notes.txt is no longer the only unstaged change (git diff --name-only: '$changed')" }
}

# A right-click opens the commit's menu; its item opens a dialog whose field takes typing and
# Return.
function Test-ContextMenu {
    $head = (Invoke-Git rev-parse HEAD).Out
    $subject = (Invoke-Git log -1 --format=%s).Out
    Invoke-WidgetClick $subject 'central' -Button 3
    Wait-Widget $script:CreateBranchLabel 'any'
    Wait-Settled
    Invoke-WidgetClick $script:CreateBranchLabel 'any'
    Wait-Probe 'the new-branch dialog with its field focused' {
        param($p)
        $p.modal -eq $true -and $null -ne $p.keyboard_focus -and $p.keyboard_focus.role -eq 'TextInput'
    } | Out-Null
    Send-Text 'smoke-branch'
    Wait-Settled
    Send-Chord $script:VkReturn
    Wait-Disk "refs/heads/smoke-branch at $head" {
        $ref = Invoke-Git rev-parse --verify -q refs/heads/smoke-branch
        $ref.Code -eq 0 -and $ref.Out -eq $head
    }
    Wait-Probe 'the dialog to close' { param($p) $p.modal -eq $false } | Out-Null
}

# Ctrl+Shift+L hides and shows the left panel, saved in the settings.
function Test-Shortcut {
    $chord = $script:VkControl, $script:VkShift, $script:VkL
    Send-Chord $chord
    Wait-SettingsFile 'show_left_panel false' { param($s) $s.show_left_panel -eq $false }
    Wait-Probe 'the left panel to be hidden' { param($p) $null -eq $p.panels.refs } | Out-Null
    Wait-Settled
    Send-Chord $chord
    Wait-SettingsFile 'show_left_panel true' { param($s) $s.show_left_panel -eq $true }
    Wait-Probe 'the left panel to be back' { param($p) $null -ne $p.panels.refs } | Out-Null
}

# Dragging the sidebar's edge 100 points to the right widens it, and the width is saved.
function Test-SidebarDrag {
    $start = Read-Probe
    $refs = $start.panels.refs
    $old = $refs[2] - $refs[0]
    $at = ConvertTo-ScreenPoint $start $refs[2] (($refs[1] + $refs[3]) / 2)
    $step = ConvertTo-Px 20 $start
    Invoke-PointerDrag $at.X $at.Y ($step * 5) 0 5
    Wait-Probe "the sidebar to be 100 points wider than $old (within 4)" {
        param($p)
        $null -ne $p.panels.refs -and [Math]::Abs(($p.panels.refs[2] - $p.panels.refs[0]) - $old - 100) -le 4
    } | Out-Null
    Wait-Settled
    $now = (Read-Probe).panels.refs
    $width = $now[2] - $now[0]
    Wait-SettingsFile "panel_sizes.sidebar within 4 of $width" {
        param($s)
        $null -ne $s.panel_sizes.sidebar -and [Math]::Abs($s.panel_sizes.sidebar - $width) -le 4
    }
}

# Dragging Trellis's own bottom-right resize corner resizes the window; it lays out to its new
# size and still maps the pointer right.
function Test-WindowResize {
    $c = [TrellisSmoke.Native]::ClientOnScreen($script:Hwnd)
    $wa = [TrellisSmoke.Native]::WorkArea()
    $dpiScale = [TrellisSmoke.Native]::Dpi($script:Hwnd) / 96.0
    $minW = [Math]::Ceiling(960 * $dpiScale)
    $minH = [Math]::Ceiling(540 * $dpiScale)
    if ($c[2] - 240 -ge $minW -and $c[3] - 160 -ge $minH) {
        $dx, $dy = -240, -160
    } elseif ($c[0] + $c[2] + 240 -le $wa[0] + $wa[2] -and $c[1] + $c[3] + 160 -le $wa[1] + $wa[3]) {
        $dx, $dy = 240, 160
    } else {
        Invoke-SmokeFailure $script:Check "setup: the $($c[2]) x $($c[3]) window can neither shrink by 240 x 160 (minimum $minW x $minH) nor grow inside the work area"
    }
    $wantW = $c[2] + $dx
    $wantH = $c[3] + $dy
    # The corner zone is 12 physical pixels at 96 DPI; winit's sizing loop follows the moves.
    Invoke-PointerDrag ($c[0] + $c[2] - 4) ($c[1] + $c[3] - 4) $dx $dy 6 -PauseAfterPress 200
    Wait-Until "the client area to be $wantW x $wantH (within 4)" { [TrellisSmoke.Native]::ClientOnScreen($script:Hwnd) } {
        param($r)
        [Math]::Abs($r[2] - $wantW) -le 4 -and [Math]::Abs($r[3] - $wantH) -le 4
    } { "; client area: $([TrellisSmoke.Native]::ClientOnScreen($script:Hwnd) -join ', ')" } 0 | Out-Null
    Wait-Probe 'probe.screen x pixels_per_point to equal the client size (within 2)' {
        param($p)
        $r = [TrellisSmoke.Native]::ClientOnScreen($script:Hwnd)
        [Math]::Abs($p.screen[0] * $p.pixels_per_point - $r[2]) -le 2 -and [Math]::Abs($p.screen[1] * $p.pixels_per_point - $r[3]) -le 2
    } | Out-Null
    Wait-Settled
    Wait-Probe 'the layout to fit the new size' {
        param($p)
        $refs = $p.panels.refs
        $detail = $p.panels.detail
        $null -ne $refs -and $null -ne $detail -and $refs[2] -le $detail[0] -and $detail[2] -le $p.screen[0] + 1
    } | Out-Null
    Invoke-WidgetClick 'Add b.txt' 'central'
    Wait-Probe "a click after the resize to select 'Add b.txt'" { param($p) $p.selection -eq "commit:$($script:BOid)" } | Out-Null
}
