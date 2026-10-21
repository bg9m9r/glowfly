# Installs, upgrades, starts, update-checks and uninstalls the Windows MSI, silently, and checks
# what each step leaves behind. Run by the Installers workflow (.github/workflows/installers.yml)
# after scripts/package-windows.ps1 built an older MSI (tag v0.0.1) and the current one from the
# same program.
#
#   pwsh scripts/installer-check-windows.ps1 -Msi dist/trellis-v0.1.0-x86_64-pc-windows-msvc.msi `
#     -OldMsi <dir>/trellis-v0.0.1-x86_64-pc-windows-msvc.msi `
#     -Zip dist/trellis-v0.1.0-x86_64-pc-windows-msvc.zip -Version 0.1.0 -Out <work folder>
#
# It changes the user's installed programs, shortcuts and registry, so it refuses to run outside
# GitHub Actions unless given -AllowLocal, and always refuses when Trellis is already installed.
# The installs are per-user and need no administrator rights. msiexec logs and every program
# run's output go to -Out.
param(
    [Parameter(Mandatory = $true)][string]$Msi,
    [Parameter(Mandatory = $true)][string]$OldMsi,
    [Parameter(Mandatory = $true)][string]$Zip,
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][string]$Out,
    [switch]$AllowLocal
)
$ErrorActionPreference = 'Stop'

if (-not $IsWindows) { throw 'installer-check-windows.ps1 runs only on Windows' }
if ($env:GITHUB_ACTIONS -ne 'true' -and -not $AllowLocal) {
    throw 'installer-check-windows.ps1 installs and uninstalls Trellis; run it on CI or pass -AllowLocal'
}
$Msi = (Resolve-Path -LiteralPath $Msi).Path
$OldMsi = (Resolve-Path -LiteralPath $OldMsi).Path
$Zip = (Resolve-Path -LiteralPath $Zip).Path
New-Item -ItemType Directory -Force $Out | Out-Null
$Out = (Resolve-Path -LiteralPath $Out).Path
$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path

$wxs = Get-Content -Raw -LiteralPath (Join-Path $root 'packaging/windows/trellis.wxs')
$m = [regex]::Match($wxs, 'UpgradeCode="(?<code>[0-9A-Fa-f-]{36})"')
if (-not $m.Success) { throw "no UpgradeCode in trellis.wxs" }
$upgradeCode = '{' + $m.Groups['code'].Value.ToUpperInvariant() + '}'

$installDir = Join-Path $env:LOCALAPPDATA 'Programs\Trellis'
$exe = Join-Path $installDir 'glowfly.exe'
$startLnk = Join-Path ([Environment]::GetFolderPath('Programs')) 'Trellis.lnk'
$desktopLnk = Join-Path ([Environment]::GetFolderPath('DesktopDirectory')) 'Trellis.lnk'
$installerKey = 'HKCU:\Software\Trellis\Installer'
$schemeKey = 'HKCU:\Software\Classes\trellis'
$folderKey = 'HKCU:\Software\Classes\Directory\shell\Trellis'
$backgroundKey = 'HKCU:\Software\Classes\Directory\Background\shell\Trellis'
$userDirs = @((Join-Path $env:APPDATA 'Trellis'), (Join-Path $env:LOCALAPPDATA 'Trellis'))

$wi = New-Object -ComObject WindowsInstaller.Installer
$shell = New-Object -ComObject WScript.Shell

# The product codes installed under Trellis's upgrade code.
function Get-RelatedProduct {
    $list = $wi.RelatedProducts($upgradeCode)
    @(for ($i = 0; $i -lt $list.Count; $i++) { $list.Item($i) })
}

function Assert-That([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw "check failed: $Message" }
}

# Exactly one Trellis installed, at $Want.
function Assert-Product([string]$Want) {
    $codes = @(Get-RelatedProduct)
    Assert-That ($codes.Count -eq 1) "expected one installed Trellis, found $($codes.Count)"
    $got = $wi.ProductInfo($codes[0], 'VersionString')
    Assert-That ($got -eq $Want) "installed version is $got, expected $Want"
}

function Invoke-Msiexec([string]$Action, [string]$Package, [string]$Log) {
    $logPath = Join-Path $Out $Log
    $arguments = @($Action, "`"$Package`"", '/qn', '/norestart', '/l*v', "`"$logPath`"")
    $p = Start-Process -FilePath msiexec.exe -ArgumentList $arguments -Wait -PassThru
    if ($p.ExitCode -ne 0) { throw "msiexec $Action $(Split-Path -Leaf $Package) exited with $($p.ExitCode); see $logPath" }
}

# Runs $Program with $Arguments, keeping its output as <Out>/<Name>.out and .err. A release
# build is a GUI program, so its output reaches only a caller that redirects it.
function Invoke-Program([string]$Program, [string[]]$Arguments, [string]$Name) {
    $outFile = Join-Path $Out "$Name.out"
    $errFile = Join-Path $Out "$Name.err"
    $p = Start-Process -FilePath $Program -ArgumentList $Arguments -Wait -PassThru -NoNewWindow `
        -RedirectStandardOutput $outFile -RedirectStandardError $errFile
    @{ Code = $p.ExitCode; Out = "$(Get-Content -Raw -LiteralPath $outFile)"; Err = "$(Get-Content -Raw -LiteralPath $errFile)" }
}

function Get-DefaultValue([string]$Key) {
    if (-not (Test-Path -LiteralPath $Key)) { return $null }
    (Get-Item -LiteralPath $Key).GetValue('')
}

function Assert-Installed {
    $files = @('glowfly.exe', 'LICENSE-MIT', 'LICENSE-APACHE', 'README.md', 'THIRD-PARTY-NOTICES.md')
    if (Test-Path -LiteralPath (Join-Path $root 'CHANGELOG.md')) { $files += 'CHANGELOG.md' }
    foreach ($f in $files) {
        Assert-That (Test-Path -LiteralPath (Join-Path $installDir $f) -PathType Leaf) "$f isn't in $installDir"
    }
    foreach ($lnk in $startLnk, $desktopLnk) {
        Assert-That (Test-Path -LiteralPath $lnk -PathType Leaf) "no shortcut at $lnk"
        $target = $shell.CreateShortcut($lnk).TargetPath
        Assert-That ($target -eq $exe) "$lnk points at $target, not $exe"
    }
    foreach ($value in 'program', 'docs', 'shortcut', 'desktop', 'folders', 'links') {
        $got = (Get-ItemProperty -LiteralPath $installerKey -Name $value -ErrorAction SilentlyContinue).$value
        Assert-That ($got -eq 1) "$installerKey has no $value = 1"
    }
    $open = Get-DefaultValue "$schemeKey\shell\open\command"
    Assert-That ($open -eq "`"$exe`" `"%1`"") "the trellis:// command is $open"
    foreach ($key in $folderKey, $backgroundKey) {
        $command = Get-DefaultValue "$key\command"
        Assert-That ($command -eq "`"$exe`" `"%V`"") "$key\command is $command"
    }
}

function Assert-Removed {
    Assert-That (-not (Test-Path -LiteralPath $installDir)) "$installDir is still there"
    foreach ($path in $startLnk, $desktopLnk) {
        Assert-That (-not (Test-Path -LiteralPath $path)) "$path is still there"
    }
    foreach ($key in $installerKey, $schemeKey, $folderKey, $backgroundKey) {
        Assert-That (-not (Test-Path -LiteralPath $key)) "$key is still there"
    }
    $codes = @(Get-RelatedProduct)
    Assert-That ($codes.Count -eq 0) "$($codes.Count) Trellis products are still installed"
}

# --- Guards: never touch a real install ------------------------------------------------------
if (@(Get-RelatedProduct).Count -gt 0 -or (Test-Path -LiteralPath $installDir)) {
    throw 'Trellis is already installed; uninstall it first'
}
$core = ($Version -split '[-+]')[0]
Assert-That ([version]$core -gt [version]'0.0.1') "the current version $Version must be newer than the old package's 0.0.1"
$missingBefore = @($userDirs | Where-Object { -not (Test-Path -LiteralPath $_) })

$installed = $false
try {
    # --- Install the old version, then upgrade ---------------------------------------------
    Invoke-Msiexec '/i' $OldMsi 'install-old.log'
    $installed = $true
    Assert-Product '0.0.1'
    Write-Host 'ok install 0.0.1'
    Invoke-Msiexec '/i' $Msi 'upgrade.log'
    Assert-Product $Version
    Write-Host "ok upgrade to $Version"
    Assert-Installed
    Write-Host 'ok files, shortcuts and registry'

    # --- Start the installed program through each shortcut ---------------------------------
    $n = 0
    foreach ($lnk in $startLnk, $desktopLnk) {
        $n++
        $target = $shell.CreateShortcut($lnk).TargetPath
        $run = Invoke-Program $target @('--version') "version-$n"
        Assert-That ($run.Code -eq 0) "$target --version exited with $($run.Code): $($run.Err)"
        Assert-That ($run.Out.Trim() -eq "trellis $Version") "$target --version printed '$($run.Out.Trim())'"
    }
    Write-Host 'ok --version from both shortcuts'

    # --- The update check, with the installed program --------------------------------------
    $feedCheck = Join-Path $PSScriptRoot 'update-feed-check.ps1'
    pwsh -NoProfile -File $feedCheck -Trellis $exe -Version $Version -Installer $Msi -Archive $Zip -Expect installer -Out (Join-Path $Out 'feed')
    if ($LASTEXITCODE -ne 0) { throw "update-feed-check.ps1 failed (exit $LASTEXITCODE)" }
    foreach ($dir in $missingBefore) {
        Assert-That (-not (Test-Path -LiteralPath $dir)) "the command-line modes created $dir"
    }

    # --- Uninstall --------------------------------------------------------------------------
    Invoke-Msiexec '/x' $Msi 'uninstall.log'
    $installed = $false
    Assert-Removed
    foreach ($dir in $missingBefore) {
        Assert-That (-not (Test-Path -LiteralPath $dir)) "$dir was created"
    }
    Write-Host 'ok uninstall removed everything'
} catch {
    Write-Host "::error::$($_.Exception.Message)"
    # Leave no install behind on a local run.
    if ($installed) {
        foreach ($code in @(Get-RelatedProduct)) {
            Start-Process -FilePath msiexec.exe -ArgumentList '/x', $code, '/qn', '/norestart', '/l*v', "`"$(Join-Path $Out 'cleanup.log')`"" -Wait | Out-Null
        }
    }
    exit 1
}
Write-Host 'all installer checks passed'
exit 0
