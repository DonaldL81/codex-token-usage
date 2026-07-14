param(
    [switch]$SkipVerify,
    [switch]$DryRun
)

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $PSCommandPath
$appRoot = (Resolve-Path -LiteralPath (Join-Path $scriptDir "..")).Path
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $appRoot "..")).Path
$versionPath = Join-Path $projectRoot "VERSION"
$packageScript = Join-Path $scriptDir "Package-Portable.ps1"
$packageJsonPath = Join-Path $appRoot "package.json"

function Assert-File {
    param([string]$Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Required file not found: $Path"
    }
}

function Invoke-Checked {
    param(
        [string]$Command,
        [string[]]$Arguments
    )

    Write-Host ">> $Command $($Arguments -join ' ')"
    if ($DryRun) {
        return
    }

    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$Command $($Arguments -join ' ') failed with exit code $LASTEXITCODE."
    }
}

Assert-File $versionPath
Assert-File $packageScript
Assert-File $packageJsonPath

$version = (Get-Content -LiteralPath $versionPath -Raw -Encoding UTF8).Trim()
if ($version -notmatch '^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$') {
    throw "Invalid VERSION value: '$version'. Expected semantic version like 0.3.1."
}

Push-Location $appRoot
try {
    if (-not (Test-Path -LiteralPath (Join-Path $appRoot "node_modules") -PathType Container)) {
        Invoke-Checked "npm" @("ci")
    }

    Invoke-Checked "npm" @("run", "package:portable")

    if (-not $SkipVerify) {
        Invoke-Checked "npm" @("run", "verify:release")
    }
}
finally {
    Pop-Location
}

$artifactPath = Join-Path $projectRoot "CodexTokenUsage-$version-windows-x64-portable.exe"
if (-not $DryRun -and -not (Test-Path -LiteralPath $artifactPath -PathType Leaf)) {
    throw "Expected package was not created: $artifactPath"
}

Write-Host ""
if ($DryRun) {
    Write-Host "Dry run completed. No command was executed."
}
else {
    Write-Host "One-click package completed."
    Write-Host "Version: $version"
    Write-Host "Artifact: $artifactPath"
}
