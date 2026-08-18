# Verifies a subset of hard dependency edges from docs/architecture/dependency-rules.md.
$ErrorActionPreference = "Stop"

function Get-DepNames([string]$crate) {
    $toml = Get-Content -Raw "crates/$crate/Cargo.toml"
    $names = @()
    foreach ($line in ($toml -split "`n")) {
        if ($line -match '^(winzsh-[\w-]+)\s*=\s*\{\s*workspace\s*=\s*true') {
            $names += $Matches[1]
        }
    }
    return $names
}

function Assert-DependsOn([string]$crate, [string]$dep) {
    $deps = Get-DepNames $crate
    if ($deps -notcontains $dep) {
        throw "Dependency rule violated: $crate must depend on $dep"
    }
}

function Assert-NotDependsOn([string]$crate, [string]$dep) {
    $deps = Get-DepNames $crate
    if ($deps -contains $dep) {
        throw "Dependency rule violated: $crate must NOT depend on $dep"
    }
}

Assert-DependsOn "winzsh" "winzsh-cli"
Assert-DependsOn "winzsh-cli" "winzsh-installer"
Assert-DependsOn "winzsh-cli" "winzsh-detect"
Assert-DependsOn "winzsh-cli" "winzsh-runtime-gen"
Assert-DependsOn "winzsh-installer" "winzsh-powershell"
Assert-DependsOn "winzsh-installer" "winzsh-runtime-gen"
Assert-DependsOn "winzsh-installer" "winzsh-doctor"
Assert-DependsOn "winzsh-powershell" "winzsh-shell-host"
Assert-DependsOn "winzsh-runtime-gen" "winzsh-prompt"
Assert-DependsOn "winzsh-runtime-gen" "winzsh-plugin"
Assert-DependsOn "winzsh-registry" "winzsh-plugin"
Assert-DependsOn "winzsh-ai" "winzsh-core"
Assert-DependsOn "winzsh-cli" "winzsh-ai"
Assert-DependsOn "winzsh-sync" "winzsh-history"
Assert-DependsOn "winzsh-cli" "winzsh-sync"
Assert-DependsOn "winzsh-agent" "winzsh-history"
Assert-DependsOn "winzsh-agent" "winzsh-registry"
Assert-DependsOn "winzsh-cli" "winzsh-agent"
Assert-DependsOn "winzsh-cli" "winzsh-shell-host"

Assert-NotDependsOn "winzsh-core" "winzsh-cli"
Assert-NotDependsOn "winzsh-prompt" "winzsh-cli"
Assert-NotDependsOn "winzsh-plugin" "winzsh-registry"
Assert-NotDependsOn "winzsh" "winzsh-installer"

# Network crates must not be pulled into foundation engines.
Assert-NotDependsOn "winzsh-prompt" "winzsh-registry"
Assert-NotDependsOn "winzsh-prompt" "winzsh-update"
Assert-NotDependsOn "winzsh-core" "winzsh-registry"

Write-Host "Crate dependency edge checks passed."
