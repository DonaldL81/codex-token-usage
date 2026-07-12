param()

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $PSCommandPath
$appRoot = (Resolve-Path -LiteralPath (Join-Path $scriptDir "..")).Path
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $appRoot "..")).Path
$versionPath = Join-Path $projectRoot "VERSION"

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

function Stop-ProcessesForPath {
    param([string]$Path)

    $resolvedPath = [System.IO.Path]::GetFullPath($Path)
    $processes = Get-CimInstance Win32_Process |
        Where-Object {
            $_.ExecutablePath -and
            ([System.IO.Path]::GetFullPath($_.ExecutablePath) -ieq $resolvedPath)
        }
    foreach ($process in $processes) {
        Stop-Process -Id $process.ProcessId -Force
        Start-Sleep -Milliseconds 300
    }
}

if (-not (Test-Path -LiteralPath $versionPath)) {
    throw "VERSION file not found: $versionPath"
}

$version = (Get-Content -LiteralPath $versionPath -Raw -Encoding UTF8).Trim()
if ($version -notmatch '^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$') {
    throw "Invalid VERSION value: '$version'. Expected semantic version like 0.3.1."
}

Push-Location $appRoot
try {
    Invoke-Checked "npm" @("run", "sync:version")
    Invoke-Checked "npm" @("run", "desktop:build")

    $sourceExe = Join-Path $appRoot "src-tauri\target\release\codex-token-usage.exe"
    if (-not (Test-Path -LiteralPath $sourceExe)) {
        throw "Release EXE not found: $sourceExe"
    }

    $destName = "CodexTokenUsage-$version-windows-x64-portable.exe"
    $destPath = Join-Path $projectRoot $destName
    Stop-ProcessesForPath -Path $destPath
    Copy-Item -LiteralPath $sourceExe -Destination $destPath -Force

    $oldPortableFiles = Get-ChildItem -LiteralPath $projectRoot -Filter "CodexTokenUsage-*-portable.exe" -File |
        Where-Object { $_.Name -ne $destName }
    foreach ($file in $oldPortableFiles) {
        Stop-ProcessesForPath -Path $file.FullName
        Remove-Item -LiteralPath $file.FullName -Force
    }

    Write-Host "Portable package created: $destPath"
}
finally {
    Pop-Location
}
