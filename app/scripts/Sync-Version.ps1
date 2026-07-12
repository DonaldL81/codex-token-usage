param()

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $PSCommandPath
$appRoot = (Resolve-Path -LiteralPath (Join-Path $scriptDir "..")).Path
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $appRoot "..")).Path
$versionPath = Join-Path $projectRoot "VERSION"
$utf8NoBom = [System.Text.UTF8Encoding]::new($false)

if (-not (Test-Path -LiteralPath $versionPath)) {
    throw "VERSION file not found: $versionPath"
}

$version = (Get-Content -LiteralPath $versionPath -Raw -Encoding UTF8).Trim()
if ($version -notmatch '^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$') {
    throw "Invalid VERSION value: '$version'. Expected semantic version like 0.3.1."
}

$packagePath = Join-Path $appRoot "package.json"
$packageText = Get-Content -LiteralPath $packagePath -Raw -Encoding UTF8
$packageText = [regex]::Replace(
    $packageText,
    '("version"\s*:\s*")[^"]+(")',
    "`${1}$version`${2}",
    1
)
[System.IO.File]::WriteAllText($packagePath, $packageText, $utf8NoBom)

$packageLockPath = Join-Path $appRoot "package-lock.json"
if (Test-Path -LiteralPath $packageLockPath) {
    $packageLockText = Get-Content -LiteralPath $packageLockPath -Raw -Encoding UTF8
    $packageLockText = [regex]::Replace(
        $packageLockText,
        '("name"\s*:\s*"codex-token-usage"\s*,\s*"version"\s*:\s*")[^"]+(")',
        "`${1}$version`${2}",
        1
    )
    $packageLockText = [regex]::Replace(
        $packageLockText,
        '(""\s*:\s*\{\s*"name"\s*:\s*"codex-token-usage"\s*,\s*"version"\s*:\s*")[^"]+(")',
        "`${1}$version`${2}",
        1
    )
    [System.IO.File]::WriteAllText($packageLockPath, $packageLockText, $utf8NoBom)
}

$tauriPath = Join-Path $appRoot "src-tauri\tauri.conf.json"
$tauriText = Get-Content -LiteralPath $tauriPath -Raw -Encoding UTF8
$tauriText = [regex]::Replace(
    $tauriText,
    '("version"\s*:\s*")[^"]+(")',
    "`${1}$version`${2}",
    1
)
[System.IO.File]::WriteAllText($tauriPath, $tauriText, $utf8NoBom)

$cargoPath = Join-Path $appRoot "src-tauri\Cargo.toml"
$cargoText = Get-Content -LiteralPath $cargoPath -Raw -Encoding UTF8
$updatedCargoText = [regex]::Replace(
    $cargoText,
    '(?m)^version\s*=\s*"[^"]+"',
    "version = `"$version`"",
    1
)
if ($updatedCargoText -eq $cargoText -and $cargoText -notmatch "(?m)^version\s*=") {
    throw "Cargo.toml version field not found: $cargoPath"
}
[System.IO.File]::WriteAllText($cargoPath, $updatedCargoText, $utf8NoBom)

$cargoLockPath = Join-Path $appRoot "src-tauri\Cargo.lock"
if (Test-Path -LiteralPath $cargoLockPath) {
    $cargoLockText = Get-Content -LiteralPath $cargoLockPath -Raw -Encoding UTF8
    $updatedCargoLockText = [regex]::Replace(
        $cargoLockText,
        '(?ms)(\[\[package\]\]\s+name\s*=\s*"codex-token-usage"\s+version\s*=\s*")[^"]+(")',
        "`${1}$version`${2}",
        1
    )
    [System.IO.File]::WriteAllText($cargoLockPath, $updatedCargoLockText, $utf8NoBom)
}

Write-Host "Version synchronized to $version."
