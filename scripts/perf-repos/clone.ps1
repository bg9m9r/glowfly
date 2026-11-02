# The real-history repositories of make-perf-repos.ps1 (dot-sourced by it): a full clone made
# with init and fetch, so running again after a failed or stopped download picks it up.

# A full clone of $Url at $Dir: fetched again until it succeeds, then the remote's default
# branch checked out. Paths this file system can't hold are left out of the checkout (a sparse
# checkout) and listed in $SkippedList: on Windows, names Windows reserves; on a case-insensitive
# file system, every path of a set differing only in case (sparse patterns ignore case there, so
# one of a set can't be kept on its own).
function Build-Clone([string]$Dir, [string]$Url, [string]$SkippedList) {
    if (-not (Test-Path -LiteralPath (Join-Path $Dir '.git'))) {
        Invoke-Git init -q $Dir
    }
    $origin = Invoke-GitRaw -C $Dir config --get remote.origin.url
    if (-not $origin) {
        Invoke-Git -C $Dir remote add origin $Url
    } elseif ($origin -ne $Url) {
        throw "$Dir fetches from $origin, not $Url; remove it to clone $Url"
    }
    if ($IsWindows) { Invoke-Git -C $Dir config core.longpaths true }
    for ($try = 1; ; $try++) {
        Invoke-GitRaw -C $Dir fetch --progress --tags origin
        if ($LASTEXITCODE -eq 0) { break }
        if ($try -ge 3) { throw "fetching $Url failed $try times; run the script again to carry on" }
        Write-Host "Fetching $Url failed; trying again ($($try + 1) of 3)."
    }
    Invoke-Git -C $Dir remote set-head origin --auto
    $head = (Invoke-GitRaw -C $Dir symbolic-ref --short refs/remotes/origin/HEAD)
    if ($LASTEXITCODE -ne 0 -or -not $head) { throw "$Url names no default branch" }
    $branch = $head.Substring('origin/'.Length)

    $paths = Get-TreePath $Dir $head
    $reserved = @(if ($IsWindows) { $paths | Where-Object { -not (Test-WindowsPath $_) } })
    $twins = @(if (Test-CaseInsensitive (Join-Path $Dir '.git')) { Get-CaseTwin $paths })
    $skipped = @($reserved) + @($twins | Where-Object { $reserved -notcontains $_ })
    if ($skipped.Count -gt 0) {
        Write-Host "Leaving $($skipped.Count) paths this file system can't hold out of the checkout (listed in $SkippedList)."
        [IO.File]::WriteAllText($SkippedList, ($skipped -join "`n") + "`n")
        $patterns = @('/*') + @($skipped | ForEach-Object { '!/' + (ConvertTo-SparsePattern $_) })
        $sparse = (Invoke-GitRaw -C $Dir rev-parse --git-path info/sparse-checkout)
        if (-not [IO.Path]::IsPathRooted($sparse)) { $sparse = Join-Path $Dir $sparse }
        New-Item -ItemType Directory -Force (Split-Path -LiteralPath $sparse) | Out-Null
        [IO.File]::WriteAllText($sparse, ($patterns -join "`n") + "`n")
        Invoke-Git -C $Dir config core.sparseCheckout true
        # Git for Windows refuses a reserved name even in the index unless this is off; the
        # sparse checkout still keeps such a path out of the folder. It stays off: with it on
        # again, checking out any other revision holding the name fails ("invalid path"). The
        # names it guards against (.git and its short name git~1) are left out above instead.
        if ($reserved.Count -gt 0) { Invoke-Git -C $Dir config core.protectNTFS false }
    }
    Invoke-Git -C $Dir checkout -q -B $branch $head
}

# A path as a sparse-checkout (gitignore) pattern matching only itself.
function ConvertTo-SparsePattern([string]$Path) {
    [regex]::Replace($Path, '[\\*?\[!#]|\s$', { param($m) '\' + $m.Value })
}

# Every file path in $Rev's tree.
function Get-TreePath([string]$Dir, [string]$Rev) {
    $encoding = [Console]::OutputEncoding
    try {
        [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
        $raw = (Invoke-GitRaw -C $Dir ls-tree -r -z --name-only $Rev) -join "`n"
        if ($LASTEXITCODE -ne 0) { throw "git ls-tree $Rev failed" }
    } finally {
        [Console]::OutputEncoding = $encoding
    }
    $raw -split "`0" | Where-Object { $_ -ne '' }
}

# The paths of $Paths that differ from another only in case.
function Get-CaseTwin([string[]]$Paths) {
    $count = [Collections.Generic.Dictionary[string, int]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($path in $Paths) { $count[$path] = 1 + $(if ($count.ContainsKey($path)) { $count[$path] } else { 0 }) }
    $Paths | Where-Object { $count[$_] -gt 1 }
}

# Whether Windows can hold every name in $Path.
function Test-WindowsPath([string]$Path) {
    foreach ($name in $Path -split '/') {
        if ($name -match '[<>:"|?*\\\x00-\x1f]' -or $name -match '[. ]$') { return $false }
        if ($name -match '^(con|prn|aux|nul|com[0-9]|lpt[0-9])\s*(\..*)?$') { return $false }
        # What NTFS reads as the .git folder, which core.protectNTFS would otherwise refuse.
        if ($name -match '^(\.git|git~[0-9]+)$') { return $false }
    }
    $true
}

# Whether the file system holding $Dir treats names differing only in case as one.
function Test-CaseInsensitive([string]$Dir) {
    $probe = Join-Path $Dir 'CaseProbe.tmp'
    [IO.File]::WriteAllText($probe, '')
    try { Test-Path -LiteralPath (Join-Path $Dir 'caseprobe.tmp') } finally { Remove-Item -LiteralPath $probe }
}
