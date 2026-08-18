#Requires -Version 5.1
<#
.SYNOPSIS
  Fill winget manifest InstallerUrl / InstallerSha256 / PackageVersion from dist/ or a release tag.

.EXAMPLE
  ./scripts/release/package.ps1
  ./scripts/release/fill-winget-manifest.ps1
#>
param(
    [string]$Version = "",
    [string]$Sha256 = "",
    [string]$Repo = "winzsh/winzsh",
    [string]$DistDir = "",
    [string]$ManifestDir = ""
)

$ErrorActionPreference = "Stop"
$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "../..")
$Utf8NoBom = New-Object System.Text.UTF8Encoding $false

if (-not $DistDir) { $DistDir = Join-Path $RepoRoot "dist" }
if (-not $ManifestDir) {
    $ManifestDir = Join-Path $RepoRoot "packaging\winget\WinZSH.WinZSH"
}

if (-not $Version) {
    $cargo = Get-Content -LiteralPath (Join-Path $RepoRoot "Cargo.toml") -Raw
    if ($cargo -match '(?ms)\[workspace\.package\].*?^version\s*=\s*"([^"]+)"') {
        $Version = $Matches[1]
    } else {
        throw "Pass -Version or ensure [workspace.package] version is set"
    }
}

$setupName = "WinZSH-Setup-x86_64.exe"
$installerUrl = "https://github.com/$Repo/releases/download/v$Version/$setupName"

if (-not $Sha256) {
    $setupPath = Join-Path $DistDir $setupName
    if (-not (Test-Path -LiteralPath $setupPath)) {
        throw "Missing $setupPath - run scripts/release/package.ps1 first, or pass -Sha256"
    }
    $Sha256 = (Get-FileHash -LiteralPath $setupPath -Algorithm SHA256).Hash.ToUpperInvariant()
} else {
    $Sha256 = $Sha256.ToUpperInvariant()
}

function Write-Utf8NoBom([string]$Path, [string]$Content) {
    [System.IO.File]::WriteAllText($Path, $Content.TrimEnd() + "`n", $Utf8NoBom)
}

function Set-YamlField([string]$Path, [string]$Key, [string]$Value) {
    $raw = [System.IO.File]::ReadAllText($Path)
    $pattern = "(?m)^(\s*)($([regex]::Escape($Key))):\s*.*$"
    if ($raw -notmatch $pattern) {
        throw "Key '$Key' not found in $Path"
    }
    $replacement = '${1}${2}: ' + $Value
    $raw = [regex]::Replace($raw, $pattern, $replacement, 1)
    Write-Utf8NoBom $Path $raw
}

$installer = Join-Path $ManifestDir "WinZSH.WinZSH.installer.yaml"
$versionFile = Join-Path $ManifestDir "WinZSH.WinZSH.yaml"
$locale = Join-Path $ManifestDir "WinZSH.WinZSH.locale.en-US.yaml"

foreach ($f in @($installer, $versionFile, $locale)) {
    if (-not (Test-Path -LiteralPath $f)) { throw "Missing manifest: $f" }
    Set-YamlField $f "PackageVersion" $Version
}

Set-YamlField $installer "InstallerUrl" $installerUrl
Set-YamlField $installer "InstallerSha256" $Sha256

Write-Host "Updated winget manifests for $Version" -ForegroundColor Green
Write-Host "  URL:  $installerUrl"
Write-Host ("  SHA256: {0}..." -f $Sha256.Substring(0, 12))
Write-Host ""
Write-Host "Validate locally (optional):"
Write-Host "  winget validate --manifest packaging\winget\WinZSH.WinZSH"
Write-Host "Submit to microsoft/winget-pkgs under:"
Write-Host ("  manifests/w/WinZSH/WinZSH/{0}/" -f $Version)
