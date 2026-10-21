# The dry run's self-test for scripts/smoke-windows.ps1, dot-sourced by Invoke-DryRun: widget
# matching, pixel and screen math, SMOKE_EXPECT_FAIL parsing, the zoom steps, the seeded
# settings and first-click's recording, against an inline sample probe. No window, no input. Each assertion throws with what
# it expected.

function Assert-Smoke([bool] $Condition, [string] $Message) {
    if (-not $Condition) { throw "self-test: $Message" }
}

# Find-Widget must fail (a SmokeFailure) for this lookup.
function Assert-NoSingleMatch([scriptblock] $Lookup, [string] $What) {
    $failed = $false
    try { $null = & $Lookup } catch [SmokeFailure] { $failed = $true }
    Assert-Smoke $failed "$What should not single-match"
}

$sample = @'
{
  "v": 1, "seq": 7, "pixels_per_point": 1.5, "screen": [960, 586],
  "widgets": [
    { "label": "Add b.txt", "value": null, "role": "Button", "rect": [100, 200, 300, 220], "region": "central" },
    { "label": "Add notes", "value": null, "role": "Button", "rect": [100, 160, 300, 180], "region": "central" },
    { "label": "Add notes", "value": null, "role": "Label", "rect": [400, 160, 500, 180], "region": "central" },
    { "label": null, "value": "Uncommitted changes", "role": "Label", "rect": [100, 120, 300, 140], "region": "central" },
    { "label": "Commit summary", "value": null, "role": "TextInput", "rect": [700, 100, 940, 136], "region": "details" },
    { "label": "Commit summary", "value": null, "role": "Label", "rect": [700, 80, 800, 96], "region": "details" },
    { "label": "Commit 1 file", "value": null, "role": "Button", "rect": [700, 500, 940, 536], "region": "details" },
    { "label": "Close window", "value": null, "role": "Button", "rect": [914, 0, 960, 44], "region": "title_bar" }
  ]
}
'@ | ConvertFrom-Json

$savedReporting = $script:Reporting
$savedCheck = $script:Check
$savedExpect = $script:ExpectFail
$savedOut = $script:SmokeOut
$savedPass = $script:Pass
$script:Reporting = $false
$script:Check = 'self-test'
$script:ClientOrigin = @(10, 20)
try {
    # Exact names, a value standing in for a missing label, regions and roles.
    $at = Find-Widget 'Add b.txt' 'central' -Probe $sample
    Assert-Smoke ($at.X -eq 310 -and $at.Y -eq 335) "Add b.txt's centre (200, 210) at 1.5 from (10, 20) is ($($at.X), $($at.Y)), not (310, 335)"
    $at = Find-Widget 'Uncommitted changes' 'central' -Probe $sample
    Assert-Smoke ($at.X -eq 310 -and $at.Y -eq 215) "a value-only widget is found by its value ($($at.X), $($at.Y))"
    $at = Find-Widget 'Commit summary' 'details' 'TextInput' -Probe $sample
    Assert-Smoke ($at.X -eq 1240 -and $at.Y -eq 197) "the summary field's centre is ($($at.X), $($at.Y)), not (1240, 197)"
    $at = Find-Widget 'prefix:Commit 1' 'details' -Probe $sample
    Assert-Smoke ($at.Y -eq 797) "a prefix finds the commit button ($($at.Y))"
    $at = Find-Widget 'Close window' 'any' -Probe $sample
    Assert-Smoke ($at.X -eq 1416) "'any' finds the title bar's close button ($($at.X))"
    $at = Find-Widget 'Add notes' 'central' -First -Probe $sample
    Assert-Smoke ($at.X -eq 310) "-First takes the first of two ($($at.X))"
    Assert-NoSingleMatch { Find-Widget 'Add notes' 'central' -Probe $sample } 'two widgets named Add notes'
    Assert-NoSingleMatch { Find-Widget 'Commit summary' 'details' -Probe $sample } 'a field and a label named Commit summary'
    Assert-NoSingleMatch { Find-Widget 'Add b.txt' 'details' -Probe $sample } 'Add b.txt outside its region'
    Assert-NoSingleMatch { Find-Widget 'Close window' 'central' -Probe $sample } 'a title bar button in central'
    Assert-NoSingleMatch { Find-Widget 'add b.txt' 'central' -Probe $sample } 'a name in another case'
    Assert-NoSingleMatch { Find-Widget 'Commit summary' 'details' 'Button' -Probe $sample } 'a name with another role'
    Assert-NoSingleMatch { Find-Widget 'prefix:Commit 2' 'details' -Probe $sample } 'a prefix nothing starts with'

    # Points to pixels, rounded half up as the Linux script does.
    Assert-Smoke ((ConvertTo-Px 3 $sample) -eq 5) "3 points at 1.5 are $(ConvertTo-Px 3 $sample) px, not 5"
    Assert-Smoke ((ConvertTo-Px 20 $sample) -eq 30) "20 points at 1.5 are $(ConvertTo-Px 20 $sample) px, not 30"
    Assert-Smoke ((ConvertTo-Px 0.3 $sample) -eq 0) "0.3 points at 1.5 are $(ConvertTo-Px 0.3 $sample) px, not 0"
    $corner = ConvertTo-ScreenPoint $sample 960 586
    Assert-Smoke ($corner.X -eq 1450 -and $corner.Y -eq 899) "the client's far corner is ($($corner.X), $($corner.Y)), not (1450, 899)"

    # SMOKE_EXPECT_FAIL is a whitespace-separated list of whole ids.
    $script:ExpectFail = "  first-click`tresize  "
    Assert-Smoke (Test-ExpectedFailure 'first-click') 'first-click is expected to fail'
    Assert-Smoke (Test-ExpectedFailure 'resize') 'resize is expected to fail'
    Assert-Smoke (-not (Test-ExpectedFailure 'click')) 'part of an id does not count'
    Assert-Smoke (-not (Test-ExpectedFailure 'scale')) 'an id not listed does not count'
    $script:ExpectFail = ''
    Assert-Smoke (-not (Test-ExpectedFailure 'scale')) 'an empty list expects nothing'

    # The scales are the app's zoom steps.
    Assert-Smoke ((Get-ZoomPercent '1') -eq 100 -and (Get-ZoomPercent '1.5') -eq 150) 'the scales 1 and 1.5 are zoom 100 and 150'
    $refused = $false
    try { $null = Get-ZoomPercent '1.3' } catch { $refused = $true }
    Assert-Smoke $refused 'the scale 1.3 (zoom 130) is not a zoom step'

    # The seeded settings round-trip.
    $seeded = Get-SeedSetting 150 '1.2.3' | ConvertFrom-Json
    Assert-Smoke ($seeded.zoom_percent -eq 150) "seeded zoom_percent is $($seeded.zoom_percent)"
    Assert-Smoke ($seeded.language -eq 'en') "seeded language is $($seeded.language)"
    Assert-Smoke ($seeded.updates.check -eq $false -and $null -eq $seeded.updates.last_checked) 'seeded updates check is off'
    Assert-Smoke ($seeded.updates.notes_seen -eq '1.2.3' -and $seeded.welcome_done -eq $true -and $seeded.reopen_last_repo -eq $false) 'seeded welcome, release notes and reopen'

    # first-click records each case as soon as it is known; a case whose setup fails is recorded
    # as not run and does not stop the next case from running and being recorded.
    $script:SmokeOut = Join-Path ([IO.Path]::GetTempPath()) "glowfly-smoke-selftest-$([guid]::NewGuid().ToString('N').Substring(0, 8))"
    New-Item -ItemType Directory -Force $script:SmokeOut | Out-Null
    $script:Pass = 'scale-1'
    $record = Join-Path $script:SmokeOut 'first-click.txt'
    $state = @{ Ran = $false }
    $lines = @(& {
            $a = Invoke-FirstClickCase 'click to focus' { 'delivered' }
            Assert-Smoke ($a -eq 'delivered') "a measured case returns its result ('$a')"
            Assert-Smoke ((Get-Content -LiteralPath $record) -ceq 'scale-1 click-to-focus: delivered') 'a measured case is in first-click.txt before the next case runs'
            $b = Invoke-FirstClickCase 'click after SetForegroundWindow' { Invoke-SmokeFailure 'first-click' 'timed out waiting for focus'; 'delivered' }
            Assert-Smoke ($b -ceq 'not run: timed out waiting for focus') "a case whose setup fails is not run ('$b')"
            $c = Invoke-FirstClickCase 'third' { throw 'boom' }
            Assert-Smoke ($c -ceq 'not run: unexpected error: boom') "a case with an unexpected error is not run ('$c')"
            $state.Ran = $true
        } 6>&1 | ForEach-Object { "$_" })
    Assert-Smoke $state.Ran 'every first-click case returned'
    $want = 'scale-1 click-to-focus: delivered', 'scale-1 click-after-SetForegroundWindow: not run: timed out waiting for focus', 'scale-1 third: not run: unexpected error: boom'
    $got = @(Get-Content -LiteralPath $record)
    Assert-Smoke (($got -join "`n") -ceq ($want -join "`n")) "first-click.txt holds every case: $($got -join ' | ')"
    $notices = @($lines | Where-Object { $_.StartsWith('::notice::') })
    Assert-Smoke ($notices.Count -eq 3 -and $notices[1] -ceq '::notice::smoke scale-1 first-click, click after SetForegroundWindow: not run: timed out waiting for focus') "a ::notice:: line per case: $($notices -join ' | ')"
} finally {
    $script:Reporting = $savedReporting
    $script:Check = $savedCheck
    $script:ExpectFail = $savedExpect
    $script:ClientOrigin = $null
    if ($script:SmokeOut -ne $savedOut) { Remove-Item -LiteralPath $script:SmokeOut -Recurse -Force -ErrorAction SilentlyContinue }
    $script:SmokeOut = $savedOut
    $script:Pass = $savedPass
}
