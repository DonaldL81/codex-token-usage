param(
    [switch]$DesktopBuildDebug,
    [switch]$SkipPortableCheck
)

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $PSCommandPath
$appRoot = (Resolve-Path -LiteralPath (Join-Path $scriptDir "..")).Path
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $appRoot "..")).Path

function Invoke-Checked {
    param(
        [string]$Command,
        [string[]]$Arguments
    )
    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$Command $($Arguments -join ' ') failed with exit code $LASTEXITCODE."
    }
}

Push-Location $appRoot
try {
    $versionPath = Join-Path $projectRoot "VERSION"
    if (-not (Test-Path -LiteralPath $versionPath)) {
        throw "VERSION file not found: $versionPath"
    }
    $version = (Get-Content -LiteralPath $versionPath -Raw -Encoding UTF8).Trim()
    if ($version -notmatch '^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$') {
        throw "Invalid VERSION value: '$version'. Expected semantic version like 0.3.1."
    }

    $package = Get-Content -Path "package.json" -Raw -Encoding UTF8 | ConvertFrom-Json
    $tauri = Get-Content -Path "src-tauri/tauri.conf.json" -Raw -Encoding UTF8 | ConvertFrom-Json
    $cargoText = Get-Content -Path "src-tauri/Cargo.toml" -Raw -Encoding UTF8
    $cargoVersion = [regex]::Match($cargoText, '(?m)^version\s*=\s*"([^"]+)"').Groups[1].Value

    if ($package.version -ne $version -or $cargoVersion -ne $version -or $tauri.version -ne $version) {
        throw "Version mismatch: VERSION=$version, package=$($package.version), cargo=$cargoVersion, tauri=$($tauri.version). Run npm run sync:version."
    }

    if (Test-Path -LiteralPath "package-lock.json") {
        $packageLockText = Get-Content -Path "package-lock.json" -Raw -Encoding UTF8
        $packageLockVersion = [regex]::Match(
            $packageLockText,
            '"name"\s*:\s*"codex-token-usage"\s*,\s*"version"\s*:\s*"([^"]+)"'
        ).Groups[1].Value
        $rootPackageVersion = [regex]::Match(
            $packageLockText,
            '""\s*:\s*\{\s*"name"\s*:\s*"codex-token-usage"\s*,\s*"version"\s*:\s*"([^"]+)"'
        ).Groups[1].Value
        if ($packageLockVersion -ne $version -or $rootPackageVersion -ne $version) {
            throw "Version mismatch in package-lock.json: root=$packageLockVersion, packageRoot=$rootPackageVersion, VERSION=$version. Run npm run sync:version."
        }
    }

    if (Test-Path -LiteralPath "src-tauri/Cargo.lock") {
        $cargoLockText = Get-Content -Path "src-tauri/Cargo.lock" -Raw -Encoding UTF8
        $cargoLockVersion = [regex]::Match(
            $cargoLockText,
            '(?ms)\[\[package\]\]\s+name\s*=\s*"codex-token-usage"\s+version\s*=\s*"([^"]+)"'
        ).Groups[1].Value
        if ($cargoLockVersion -ne $version) {
            throw "Version mismatch in Cargo.lock: cargoLock=$cargoLockVersion, VERSION=$version. Run npm run sync:version."
        }
    }

    $gitignore = Get-Content -Path (Join-Path $projectRoot ".gitignore") -Raw -Encoding UTF8
    $requiredIgnores = @("_archive/", "reports/", "node_modules/", "app/src-tauri/target/", "dist/", "/CodexTokenUsage-*-portable.exe")
    foreach ($item in $requiredIgnores) {
        if (-not $gitignore.Contains($item)) {
            throw ".gitignore is missing required entry: $item"
        }
    }

    if (-not $SkipPortableCheck) {
        $expectedPortableName = "CodexTokenUsage-$version-windows-x64-portable.exe"
        $portableFiles = @(Get-ChildItem -LiteralPath $projectRoot -Filter "CodexTokenUsage-*-portable.exe" -File)
        $unexpectedPortableFiles = @($portableFiles | Where-Object { $_.Name -ne $expectedPortableName })
        if ($unexpectedPortableFiles.Count -gt 0) {
            throw "Root contains non-current portable EXE: $($unexpectedPortableFiles.Name -join ', '). Expected only $expectedPortableName."
        }
        $expectedPortablePath = Join-Path $projectRoot $expectedPortableName
        if (-not (Test-Path -LiteralPath $expectedPortablePath)) {
            throw "Current portable EXE not found at project root: $expectedPortableName"
        }
    }

    Invoke-Checked "npm" @("run", "build")
    Invoke-Checked "npm" @("run", "test:rust")

    if ($DesktopBuildDebug) {
        Invoke-Checked "npm" @("run", "desktop:build:debug")
    }

    Write-Host "Release verification passed for version $version."
}
finally {
    Pop-Location
}
