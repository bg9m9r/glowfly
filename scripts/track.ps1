# Moves an issue's card on the Glowfly Project (github.com/users/bg9m9r/projects/2) to a Status
# column, adding the issue to the Project first if it isn't on it. Needs `gh` signed in with the
# `project` scope (`gh auth refresh -s project`).
#
#   pwsh scripts/track.ps1 123 'In progress'
#   pwsh scripts/track.ps1 123 Inbox
#
# The Project doesn't move closed issues to Done, so run `pwsh scripts/track.ps1 123 Done` after the
# landing that closes the issue (the one whose commit says `Closes #123`).
param(
    [Parameter(Mandatory = $true)][int]$Issue,
    [Parameter(Mandatory = $true)]
    [ValidateSet('Inbox', 'Backlog', 'Ready', 'In progress', 'In review', 'Done')]
    [string]$Status
)
$ErrorActionPreference = "Stop"

$Owner = 'bg9m9r'
$Repo = 'glowfly'
$ProjectNumber = 2
$ProjectId = 'PVT_kwHOABvwFM4BoSp1'
$StatusField = 'PVTSSF_lAHOABvwFM4BoSp1zhmzbdI'
$StatusOptions = @{
    'Inbox'       = '50086577'
    'Backlog'     = '7a34acc5'
    'Ready'       = '74ed36ea'
    'In progress' = '32435649'
    'In review'   = '3c3d045b'
    'Done'        = 'd941fb7f'
}

$query = 'query($o:String!,$r:String!,$n:Int!){ repository(owner:$o,name:$r){ issue(number:$n){ url projectItems(first:20){ nodes{ id project{ id } } } } } }'
$json = gh api graphql -f query=$query -f o=$Owner -f r=$Repo -F n=$Issue
if ($LASTEXITCODE -ne 0) { Write-Error "could not read issue #$Issue" }
$found = ($json | ConvertFrom-Json).data.repository.issue
if (-not $found) { Write-Error "issue #$Issue not found in $Owner/$Repo" }

$item = $found.projectItems.nodes | Where-Object { $_.project.id -eq $ProjectId } | Select-Object -First 1
if ($item) {
    $itemId = $item.id
} else {
    $added = gh project item-add $ProjectNumber --owner $Owner --url $found.url --format json
    if ($LASTEXITCODE -ne 0) { Write-Error "could not add issue #$Issue to the Project" }
    $itemId = ($added | ConvertFrom-Json).id
}

gh project item-edit --id $itemId --project-id $ProjectId --field-id $StatusField `
    --single-select-option-id $StatusOptions[$Status] | Out-Null
if ($LASTEXITCODE -ne 0) { Write-Error "could not move issue #$Issue to $Status" }
Write-Host "#$Issue -> $Status"
