param(
    [string]$OutputDir = "",
    [switch]$NoRun,
    [switch]$Force,
    [switch]$KeepRunning,
    [switch]$LaunchOnly
)

$ErrorActionPreference = "Stop"

$repo = "DonaldL81/codex-token-usage"
$branch = "main"
$rawBaseUrl = "https://raw.githubusercontent.com/$repo/$branch"
$latestReleaseUrl = "https://api.github.com/repos/$repo/releases/latest"
$userAgent = "codex-token-usage-skill"
$appDirName = "CodexTokenUsage"
$stablePortableName = "CodexTokenUsage.exe"
$productAssetPrefix = "CodexTokenUsage"

function Resolve-OutputDir {
    param([string]$Value)

    if ($Value) {
        if (Test-Path -LiteralPath $Value) {
            return (Resolve-Path -LiteralPath $Value).Path
        }
        return [System.IO.Path]::GetFullPath($Value)
    }

    if ($env:LOCALAPPDATA) {
        return (Join-Path (Join-Path $env:LOCALAPPDATA "Programs") $appDirName)
    }

    if ($env:USERPROFILE) {
        return (Join-Path (Join-Path $env:USERPROFILE "Downloads") $appDirName)
    }

    return (Join-Path (Get-Location).Path $appDirName)
}

function Get-VersionFromRepository {
    $versionUrl = "$rawBaseUrl/VERSION"
    $response = Invoke-WebRequest -Uri $versionUrl -Headers @{ "User-Agent" = $userAgent } -UseBasicParsing
    $version = $response.Content.Trim()
    if ($version -notmatch "^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$") {
        throw "Cannot read a valid version from repository VERSION."
    }
    return $version
}

function New-PackageInfoFromVersion {
    param([Parameter(Mandatory = $true)][string]$Version)

    $name = "$productAssetPrefix-$Version-windows-x64-portable.exe"
    $escapedName = [uri]::EscapeDataString($name)
    [pscustomobject]@{
        Name = $name
        Url = "https://github.com/$repo/releases/download/v$Version/$escapedName"
        Source = "GitHub Release version fallback"
        Version = $Version
    }
}

function Select-ReleaseAsset {
    param([Parameter(Mandatory = $true)]$Assets)

    $all = @($Assets | Where-Object { $_.name -and $_.browser_download_url })
    $asset = $all |
        Where-Object {
            $name = $_.name.ToString().ToLowerInvariant()
            $name.Contains($productAssetPrefix.ToLowerInvariant()) -and
                $name.Contains("portable") -and
                $name.EndsWith(".exe")
        } |
        Select-Object -First 1
    if ($asset) {
        return $asset
    }

    $asset = $all |
        Where-Object {
            $name = $_.name.ToString().ToLowerInvariant()
            $name.Contains($productAssetPrefix.ToLowerInvariant()) -and $name.EndsWith(".exe")
        } |
        Select-Object -First 1
    if ($asset) {
        return $asset
    }

    return ($all | Where-Object { $_.name.ToString().ToLowerInvariant().EndsWith(".exe") } | Select-Object -First 1)
}

function Get-LatestReleasePackageInfo {
    try {
        $release = Invoke-RestMethod -Uri $latestReleaseUrl -Headers @{
            "User-Agent" = $userAgent
            "Accept" = "application/vnd.github+json"
        }
        $version = ($release.tag_name -replace "^v", "")
        if (-not ($version -match "^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$")) {
            return $null
        }

        $asset = Select-ReleaseAsset -Assets $release.assets
        if (-not $asset) {
            return $null
        }

        [pscustomobject]@{
            Name = $asset.name
            Url = $asset.browser_download_url
            Source = "GitHub Latest Release"
            Version = $version
        }
    } catch {
        return $null
    }
}

function Test-RunningTarget {
    param([string]$TargetPath)

    $resolvedTarget = [System.IO.Path]::GetFullPath($TargetPath)
    $processes = @(Get-CimInstance Win32_Process -ErrorAction SilentlyContinue | Where-Object {
        $_.ExecutablePath -and ([System.IO.Path]::GetFullPath($_.ExecutablePath) -ieq $resolvedTarget)
    })
    return ($processes.Count -gt 0)
}

function Stop-ProcessesForPath {
    param([string]$TargetPath)

    $resolvedTarget = [System.IO.Path]::GetFullPath($TargetPath)
    $processes = @(Get-CimInstance Win32_Process -ErrorAction SilentlyContinue | Where-Object {
        $_.ExecutablePath -and ([System.IO.Path]::GetFullPath($_.ExecutablePath) -ieq $resolvedTarget)
    })
    foreach ($process in $processes) {
        Stop-Process -Id $process.ProcessId -Force
        Start-Sleep -Milliseconds 300
    }
}

function Install-PortablePackage {
    param(
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$Target,
        [switch]$KeepTargetRunning
    )

    if (-not (Test-Path -LiteralPath $Source)) {
        throw "Package does not exist: $Source"
    }
    if ((Get-Item -LiteralPath $Source).Length -lt 131072) {
        throw "Package file is unexpectedly small."
    }

    $targetDir = Split-Path -Parent $Target
    New-Item -ItemType Directory -Force -Path $targetDir | Out-Null
    if (-not $KeepTargetRunning) {
        Stop-ProcessesForPath -TargetPath $Target
    }

    $tempPath = "$Target.copying"
    $backupPath = "$Target.previous"
    if (Test-Path -LiteralPath $tempPath) {
        Remove-Item -LiteralPath $tempPath -Force
    }
    Copy-Item -LiteralPath $Source -Destination $tempPath -Force
    if ((Get-Item -LiteralPath $tempPath).Length -lt 131072) {
        Remove-Item -LiteralPath $tempPath -Force
        throw "Copied package is unexpectedly small."
    }

    if (Test-Path -LiteralPath $backupPath) {
        Remove-Item -LiteralPath $backupPath -Force
    }
    if (Test-Path -LiteralPath $Target) {
        Move-Item -LiteralPath $Target -Destination $backupPath -Force
    }

    try {
        Move-Item -LiteralPath $tempPath -Destination $Target -Force
    } catch {
        if ((Test-Path -LiteralPath $backupPath) -and -not (Test-Path -LiteralPath $Target)) {
            Move-Item -LiteralPath $backupPath -Destination $Target -Force
        }
        throw
    }

    if (Test-Path -LiteralPath $backupPath) {
        Remove-Item -LiteralPath $backupPath -Force
    }

    $markerPath = Join-Path $targetDir ".stable-entry"
    Set-Content -LiteralPath $markerPath -Value ("updated_at={0}`npath={1}" -f (Get-Date -Format "o"), $Target) -Encoding UTF8
}

$targetDir = Resolve-OutputDir -Value $OutputDir
if (-not (Test-Path -LiteralPath $targetDir)) {
    New-Item -ItemType Directory -Path $targetDir -Force | Out-Null
}

$stablePortablePath = Join-Path $targetDir $stablePortableName
$downloaded = $false
$installed = $false
$launched = $false
$runningProcess = $false
$asset = $null
$packagePath = $null

if ($LaunchOnly) {
    if (-not (Test-Path -LiteralPath $stablePortablePath)) {
        throw "Stable entry does not exist: $stablePortablePath"
    }
    Start-Process -FilePath $stablePortablePath -WorkingDirectory $targetDir | Out-Null
    $launched = $true
    Start-Sleep -Seconds 1
    $runningProcess = Test-RunningTarget -TargetPath $stablePortablePath
} else {
    $asset = Get-LatestReleasePackageInfo
    if (-not $asset) {
        $version = Get-VersionFromRepository
        $asset = New-PackageInfoFromVersion -Version $version
    }

    $packagePath = Join-Path $targetDir $asset.Name
    $existsBefore = Test-Path -LiteralPath $packagePath
    if ((-not $existsBefore) -or $Force) {
        Write-Host "Version: $($asset.Version)"
        Write-Host "Source: $($asset.Source)"
        Write-Host "Package: $($asset.Name)"
        Write-Host "DownloadingTo: $packagePath"

        Invoke-WebRequest -Uri $asset.Url -OutFile $packagePath -Headers @{ "User-Agent" = $userAgent } -UseBasicParsing
        $downloaded = $true
    } else {
        Write-Host "Version: $($asset.Version)"
        Write-Host "UsingExistingPackage: $packagePath"
    }

    if ((Get-Item -LiteralPath $packagePath).Length -lt 131072) {
        throw "Downloaded file is unexpectedly small."
    }

    if (-not $NoRun) {
        Install-PortablePackage -Source $packagePath -Target $stablePortablePath -KeepTargetRunning:$KeepRunning
        $installed = $true
        Start-Process -FilePath $stablePortablePath -WorkingDirectory $targetDir | Out-Null
        $launched = $true
        Start-Sleep -Seconds 1
        $runningProcess = Test-RunningTarget -TargetPath $stablePortablePath
    }
}

$reportedPath = if ($NoRun -and $packagePath) { $packagePath } else { $stablePortablePath }
$reportedFile = if (Test-Path -LiteralPath $reportedPath) { Get-Item -LiteralPath $reportedPath } else { $null }

Write-Host ""
Write-Host "Downloaded: $downloaded"
Write-Host "Installed: $installed"
Write-Host "Launched: $launched"
Write-Host "RunningProcess: $runningProcess"
Write-Host "PackagePath: $packagePath"
Write-Host "StablePath: $stablePortablePath"
if ($reportedFile) {
    Write-Host "Path: $($reportedFile.FullName)"
    Write-Host "SizeBytes: $($reportedFile.Length)"
}
