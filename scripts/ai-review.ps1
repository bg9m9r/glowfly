# Posts the pr-reviewer agent's verdict on a pull request: one review with event COMMENT, then the
# required `AI review` status on the commit that was reviewed. It is the only way the agent changes
# anything on GitHub, so its limits hold whatever a PR's text says:
#
#   - the repository, the review event and the status context are fixed here;
#   - the status goes on -Sha only while it is still the PR's head (a newer push needs a new review);
#   - only the owner's, Dependabot's and release-please's PRs can pass. Anyone else's gets `pending`
#     ("Needs the owner's review") even when the agent approves;
#   - it reads -Body (.md) and -Comments (.json) only directly from the `glowfly-review` folder in the
#     temp folder, never through a link and at most 64 KB each, so it can't be pointed at other files
#     to post them publicly.
#
#   pwsh scripts/ai-review.ps1 -Pr 123 -Sha <head sha> -Verdict approve -Body review.md
#   pwsh scripts/ai-review.ps1 -Pr 123 -Sha <head sha> -Verdict changes -Body review.md -Comments inline.json
#
# -Comments is an optional JSON array of {"path", "line", "body"} for findings that fail the PR.
param(
    [Parameter(Mandatory = $true)][int]$Pr,
    [Parameter(Mandatory = $true)][ValidatePattern('^[0-9a-f]{40}$')][string]$Sha,
    [Parameter(Mandatory = $true)][ValidateSet('approve', 'changes')][string]$Verdict,
    [Parameter(Mandatory = $true)][string]$Body,
    [string]$Comments
)
$ErrorActionPreference = 'Stop'
$repo = 'bg9m9r/glowfly'
$trusted = @('bg9m9r', 'app/dependabot', 'app/github-actions')
$folder = [System.IO.Path]::GetFullPath((Join-Path ([System.IO.Path]::GetTempPath()) 'glowfly-review'))

# The text of a review file, refusing anything not directly in $folder, a link, another type or a big
# file.
function Read-ReviewFile([string]$Path, [string]$Extension) {
    $full = [System.IO.Path]::GetFullPath($Path)
    $inside = [System.IO.Path]::GetDirectoryName($full) -eq $folder
    if (-not $inside -or [System.IO.Path]::GetExtension($full) -ne $Extension -or (Get-Item -LiteralPath $folder -Force).LinkType) {
        throw "Review files are $Extension files directly in $folder, which isn't a link."
    }
    $item = Get-Item -LiteralPath $full -Force
    if ($item.PSIsContainer -or $item.LinkType -or $item.Length -gt 64KB) { throw "$full isn't a small plain file." }
    Get-Content -Raw -LiteralPath $full
}

$reviewBody = Read-ReviewFile $Body '.md'
$pull = gh pr view $Pr --repo $repo --json headRefOid,author,state | ConvertFrom-Json
if ($pull.state -ne 'OPEN') { throw "PR #$Pr is $($pull.state), not open." }
if ($pull.headRefOid -ne $Sha) { throw "PR #$Pr's head is now $($pull.headRefOid); review that commit." }

$inline = @()
if ($Comments) {
    foreach ($c in (Read-ReviewFile $Comments '.json' | ConvertFrom-Json)) {
        if (-not $c.path -or -not $c.line -or -not $c.body) { throw 'Each inline comment needs path, line and body.' }
        $inline += @{ path = [string]$c.path; line = [int]$c.line; side = 'RIGHT'; body = [string]$c.body }
    }
}
$review = @{ commit_id = $Sha; event = 'COMMENT'; body = $reviewBody; comments = $inline }
$file = New-TemporaryFile
try {
    $review | ConvertTo-Json -Depth 5 | Set-Content -Encoding utf8NoBOM $file
    gh api "repos/$repo/pulls/$Pr/reviews" --input $file --silent
    if ($LASTEXITCODE -ne 0) { throw 'Posting the review failed.' }
} finally {
    Remove-Item $file
}

if ($Verdict -eq 'changes') {
    $state, $text = 'failure', 'Changes requested by the AI reviewer'
} elseif ($trusted -contains $pull.author.login) {
    $state, $text = 'success', 'Approved by the AI reviewer'
} else {
    $state, $text = 'pending', "Needs the owner's review"
}
gh api "repos/$repo/statuses/$Sha" -f state=$state -f context='AI review' -f description=$text --silent
if ($LASTEXITCODE -ne 0) { throw 'Setting the AI review status failed.' }
Write-Output "PR #${Pr}: $state ($text) on $Sha"
