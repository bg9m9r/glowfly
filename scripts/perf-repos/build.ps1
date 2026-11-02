# The synthetic repositories of make-perf-repos.ps1 (dot-sourced by it), written with
# git fast-import from the streams in Streams.cs, and the git helper both builders use.
# Each Build-* function makes its repository in an empty or missing folder $Dir.

# Git settings for every git call while building: no user hooks, signing or prompts, a fixed
# identity for the few commits made with git itself, and local submodule clones allowed.
$script:BuildGitConfig = @(
    '-c', 'user.name=Glowfly Perf', '-c', 'user.email=perf@example.com',
    '-c', 'commit.gpgsign=false', '-c', 'tag.gpgsign=false',
    '-c', 'protocol.file.allow=always', '-c', 'advice.detachedHead=false', '-c', 'core.autocrlf=false'
)

# Runs git with the build settings; throws when it fails.
function Invoke-Git {
    $hooks = Join-Path ([IO.Path]::GetTempPath()) 'glowfly-perf-no-hooks'
    & git @script:BuildGitConfig -c "core.hooksPath=$hooks" @args
    if ($LASTEXITCODE -ne 0) { throw "git $($args -join ' ') failed (exit $LASTEXITCODE)" }
}

# The count $N at the chosen scale, never below $Min.
function Get-Scaled([double]$N, [double]$Min = 1) {
    [long][Math]::Max($Min, [Math]::Round($N * $script:Scale))
}

# An empty repository at $Dir whose HEAD is main.
function New-Repo([string]$Dir) {
    Invoke-Git init -q -b main $Dir
    if ($IsWindows) { Invoke-Git -C $Dir config core.longpaths true }
}

# Feeds git fast-import in $Repo the stream $Write writes (a scriptblock given the input stream).
function Invoke-FastImport([string]$Repo, [scriptblock]$Write) {
    if (-not ('GlowflyPerfRepos.FastImport' -as [type])) {
        Add-Type -Path (Join-Path $PSScriptRoot 'Streams.cs')
    }
    $psi = [Diagnostics.ProcessStartInfo]::new('git')
    foreach ($a in @('-C', $Repo, 'fast-import', '--quiet')) { $psi.ArgumentList.Add($a) }
    $psi.UseShellExecute = $false
    $psi.RedirectStandardInput = $true
    $process = [Diagnostics.Process]::Start($psi)
    $stream = [IO.BufferedStream]::new($process.StandardInput.BaseStream, 1MB)
    try {
        & $Write $stream
        $stream.Flush()
    } finally {
        $stream.Dispose()
        $process.WaitForExit()
    }
    if ($process.ExitCode -ne 0) { throw "git fast-import failed in $Repo (exit $($process.ExitCode))" }
}

# A new repository at $Dir from the stream $Write, with main checked out.
function New-ImportedRepo([string]$Dir, [scriptblock]$Write) {
    New-Repo $Dir
    Invoke-FastImport $Dir $Write
    Invoke-Git -C $Dir reset -q --hard main
}

function Build-LinearRepo([string]$Dir) {
    $commits = Get-Scaled 1000000
    New-ImportedRepo $Dir { param($s) [GlowflyPerfRepos.FastImport]::Linear($s, $commits, 2000) }
}

function Build-RefRepo([string]$Dir) {
    $commits = Get-Scaled 100000
    $tags = Get-Scaled 100000
    $branches = Get-Scaled 20000
    New-ImportedRepo $Dir { param($s) [GlowflyPerfRepos.FastImport]::Refs($s, $commits, $tags, $branches) }
    # As a clone has them: in packed-refs, not 120k loose files.
    Invoke-Git -C $Dir pack-refs --all
}

function Build-WideTreeRepo([string]$Dir) {
    $files = Get-Scaled 500000 100
    New-ImportedRepo $Dir { param($s) [GlowflyPerfRepos.FastImport]::WideTree($s, $files) }
    [GlowflyPerfRepos.FastImport]::ChangeWideTree($Dir, $files)
    # Stage the changes in the first folder, so the tree has staged and unstaged changes.
    Invoke-Git -C $Dir add -A -- d000/s00
}

function Build-OctopusRepo([string]$Dir) {
    $rounds = Get-Scaled 100
    $topics = Get-Scaled 2000
    New-ImportedRepo $Dir { param($s) [GlowflyPerfRepos.FastImport]::Octopus($s, 60, $rounds, $topics) }
}

function Build-BigFileRepo([string]$Dir) {
    $text = Get-Scaled 100MB 1KB
    $binary = Get-Scaled 50MB 1KB
    $count = Get-Scaled 200
    $medium = Get-Scaled 1MB 1KB
    New-ImportedRepo $Dir { param($s) [GlowflyPerfRepos.FastImport]::BigFiles($s, $text, $binary, $count, $medium) }
}

function Build-LfsRepo([string]$Dir) {
    $commits = Get-Scaled 200
    New-Repo $Dir
    Invoke-FastImport $Dir { param($s) [GlowflyPerfRepos.FastImport]::LfsPointers($s, $commits, 25) }
    # Check out the pointers themselves: no LFS objects exist to fetch, whether or not git-lfs is
    # installed (an empty filter command means no filter).
    Invoke-Git -C $Dir -c filter.lfs.process= -c filter.lfs.smudge= -c filter.lfs.required=false reset -q --hard main
}

# Submodules three deep (super -> libs/a -> vendor/b -> deep/c) beside a plain one (libs/d), all
# cloned from the repositories in $Dir/upstream. The superproject ($Dir/super) ends with libs/a
# checked out ahead of the commit it records and an untracked file in the deepest one.
function Build-SubmoduleRepo([string]$Dir) {
    $commits = Get-Scaled 20
    $up = Join-Path $Dir 'upstream'
    foreach ($name in 'c', 'b', 'a', 'd') {
        New-ImportedRepo (Join-Path $up $name) { param($s) [GlowflyPerfRepos.FastImport]::Linear($s, $commits, 5) }
    }
    # Relative URLs resolve against the folder of a repository without a remote.
    foreach ($pair in @(@('b', '../c', 'deep/c'), @('a', '../b', 'vendor/b'))) {
        $repo = Join-Path $up $pair[0]
        Invoke-Git -C $repo submodule --quiet add $pair[1] $pair[2]
        Invoke-Git -C $repo commit -q --no-verify -m "Add $($pair[2]) as a submodule"
    }
    $super = Join-Path $Dir 'super'
    New-ImportedRepo $super { param($s) [GlowflyPerfRepos.FastImport]::Linear($s, $commits, 5) }
    Invoke-Git -C $super submodule --quiet add ../upstream/a libs/a
    Invoke-Git -C $super submodule --quiet add ../upstream/d libs/d
    Invoke-Git -C $super commit -q --no-verify -m 'Add libs/a and libs/d as submodules'
    Invoke-Git -C $super submodule --quiet update --init --recursive
    $a = Join-Path $up 'a'
    Invoke-Git -C $a commit -q --no-verify --allow-empty -m 'Move a forward'
    Invoke-Git -C $super submodule --quiet update --remote libs/a
    Invoke-Git -C $super commit -q --no-verify -am 'Move libs/a forward'
    Invoke-Git -C $a commit -q --no-verify --allow-empty -m 'Move a forward again'
    Invoke-Git -C $super submodule --quiet update --remote libs/a
    $deep = Join-Path $super 'libs/a/vendor/b/deep/c/untracked.txt'
    [IO.File]::WriteAllText($deep, "an untracked file three submodules down`n")
}
