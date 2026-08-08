#Requires -Version 5.1
<#
.SYNOPSIS
  Build WinZSH from this repo and install it like a ready-to-use tool on this machine.

.DESCRIPTION
  1) Checks Rust + PowerShell
  2) cargo build -p winzsh --release
  3) Copies winzsh.exe to ~/.winzsh/bin and adds that folder to the user PATH
  4) Writes zsh-for-win.cmd launcher
  5) Runs `winzsh install` (profile hook + runtime)
  6) Sets theme modern (optional plugins can be added afterward)

.EXAMPLE
  cd zsh-for-win
  ./scripts/install-from-source.ps1
#>
$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $RepoRoot

function Assert-Command([string]$Name, [string]$Hint) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Missing '$Name'. $Hint"
    }
}

Write-Host "==> Checking prerequisites" -ForegroundColor Cyan
Assert-Command rustc "Install Rust from https://rustup.rs (stable 1.85+)."
Assert-Command cargo "Install Rust from https://rustup.rs."
$pwshOk = [bool](Get-Command pwsh -ErrorAction SilentlyContinue)
$psOk = [bool](Get-Command powershell -ErrorAction SilentlyContinue)
if (-not $pwshOk -and -not $psOk) {
    throw "Missing PowerShell. Install PowerShell 7: winget install --id Microsoft.PowerShell"
}
if (-not $pwshOk) {
    Write-Host "Note: pwsh not found; Windows PowerShell will be used (pwsh recommended)." -ForegroundColor Yellow
}

$rustc = & rustc --version
Write-Host "    $rustc"

Write-Host "==> Building release binary" -ForegroundColor Cyan
cargo build -p winzsh --release
$built = Join-Path $RepoRoot "target\release\winzsh.exe"
if (-not (Test-Path -LiteralPath $built)) {
    throw "Build succeeded but winzsh.exe not found at $built"
}

$bin = Join-Path $env:USERPROFILE ".winzsh\bin"
New-Item -ItemType Directory -Path $bin -Force | Out-Null
$dest = Join-Path $bin "winzsh.exe"
Copy-Item -LiteralPath $built -Destination $dest -Force
Write-Host "    Installed CLI: $dest"

$launcher = Join-Path $bin "zsh-for-win.cmd"
@'
@echo off
REM Nested WinZSH session from CMD/stock shells. Type "exit" to return.
set WINZSH_SHELL=1
where pwsh >nul 2>&1
if errorlevel 1 (
  powershell %*
  exit /b %ERRORLEVEL%
)
pwsh %*
'@ | Set-Content -LiteralPath $launcher -Encoding ascii
Write-Host "    Launcher: $launcher"

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (-not $userPath) { $userPath = "" }
$parts = @($userPath -split ';' | Where-Object { $_ -ne '' })
if ($parts -notcontains $bin) {
    $newPath = if ($userPath.TrimEnd(';')) { "$userPath;$bin" } else { $bin }
    [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
    Write-Host "    Added to user PATH: $bin"
} else {
    Write-Host "    Already on user PATH: $bin"
}
if ($env:PATH -notlike "*$bin*") {
    $env:PATH = "$bin;$env:PATH"
}

Write-Host "==> Running winzsh install" -ForegroundColor Cyan
& $dest install --force
& $dest theme set modern

Write-Host ""
Write-Host "WinZSH is ready." -ForegroundColor Green
Write-Host @"

Next steps:
  1. Close this terminal and open a NEW PowerShell tab (so PATH + profile load).
  2. You should see a normal PowerShell prompt.
  3. Run:  zsh-for-win
  4. Try:   Get-WinZshInfo
            gs
  5. Run:   exit     (back to stock PowerShell)

Optional extras:
  winzsh plugin add docker
  winzsh plugin add git
  winzsh plugin add rust
  winget install junegunn.fzf
  winget install ajeetdsouza.zoxide
  winzsh doctor

"@
