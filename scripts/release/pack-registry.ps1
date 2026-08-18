#Requires -Version 5.1
<#
.SYNOPSIS
  Zip community plugins under registry/plugins/ into registry/packages/ and refresh index.json hashes.

.EXAMPLE
  ./scripts/release/pack-registry.ps1
#>
$ErrorActionPreference = "Stop"
$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "../..")
$pluginsRoot = Join-Path $RepoRoot "registry\plugins"
$packagesRoot = Join-Path $RepoRoot "registry\packages"
$indexPath = Join-Path $RepoRoot "registry\index.json"

New-Item -ItemType Directory -Path $packagesRoot -Force | Out-Null

$plugins = @()
Get-ChildItem -LiteralPath $pluginsRoot -Directory | ForEach-Object {
    $id = $_.Name
    $manifestPath = Join-Path $_.FullName "plugin.toml"
    if (-not (Test-Path -LiteralPath $manifestPath)) {
        Write-Warning "skip $id (no plugin.toml)"
        return
    }
    $raw = Get-Content -LiteralPath $manifestPath -Raw
    if ($raw -notmatch '(?m)^version\s*=\s*"([^"]+)"') {
        throw "plugin $id missing version"
    }
    $version = $Matches[1]
    $zipName = "$id-$version.zip"
    $zipPath = Join-Path $packagesRoot $zipName
    if (Test-Path -LiteralPath $zipPath) { Remove-Item -LiteralPath $zipPath -Force }

    # Zip contents at archive root (plugin.toml at top level).
    Compress-Archive -Path (Join-Path $_.FullName "*") -DestinationPath $zipPath -Force
    $sha = (Get-FileHash -LiteralPath $zipPath -Algorithm SHA256).Hash.ToLowerInvariant()
    Write-Host ("  {0}  {1}" -f $sha.Substring(0, 12), $zipName)

    $desc = ""
    if ($raw -match '(?m)^description\s*=\s*"([^"]*)"') { $desc = $Matches[1] }

    $plugins += [ordered]@{
        id            = $id
        version       = $version
        description   = $desc
        author        = "WinZSH Contributors"
        tags          = @("community")
        download_url  = "embedded:$id"
        sha256        = $sha
        signature     = $null
        homepage      = "https://github.com/winzsh/winzsh/tree/main/registry/plugins/$id"
        package       = "packages/$zipName"
    }
}

$index = [ordered]@{
    schema_version = 1
    updated_at     = (Get-Date).ToUniversalTime().ToString("o")
    plugins        = $plugins
}

$json = $index | ConvertTo-Json -Depth 6
# PowerShell ConvertTo-Json may use different formatting; write UTF-8 no BOM
$utf8 = New-Object System.Text.UTF8Encoding $false
[System.IO.File]::WriteAllText($indexPath, $json.TrimEnd() + "`n", $utf8)
Write-Host "Wrote $indexPath ($($plugins.Count) plugins)" -ForegroundColor Green
