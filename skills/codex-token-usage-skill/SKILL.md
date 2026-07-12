---
name: codex-token-usage-skill
description: 下载、安装、启动或更新 Codex Token Usage / Codex token 用量统计 Windows 单文件工具。Use when the user asks to open/start/launch Codex Token Usage, install or update the latest portable version, download the Windows package, or set it up on a new computer.
---

# Codex Token Usage Skill

## Overview

This skill installs, opens, or updates the latest Codex Token Usage Windows portable app from the official GitHub Release.

Use it as the distribution entry for the desktop tool, not for editing the app source code or changing token-statistics logic.

## When To Use

- The user asks to install, download, open, start, launch, update, or reinstall Codex Token Usage.
- The user wants the latest Windows portable package.
- The user is setting up this token-usage viewer on a new Windows computer.
- The user wants a stable local entry instead of running a versioned EXE from the project root.

## Quick Start

Run the bundled PowerShell installer:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/download-codex-token-usage.ps1
```

Default behavior:

1. Read the latest GitHub Release from `DonaldL81/codex-token-usage`.
2. Download the `CodexTokenUsage-*-windows-x64-portable.exe` asset.
3. Install it to `%LOCALAPPDATA%\Programs\CodexTokenUsage\CodexTokenUsage.exe`.
4. Launch the stable entry.

## Common Operations

Open an already installed stable entry:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/download-codex-token-usage.ps1 -LaunchOnly
```

Download only without installing or launching:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/download-codex-token-usage.ps1 -NoRun
```

Force redownload:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/download-codex-token-usage.ps1 -Force
```

Install to a custom directory:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/download-codex-token-usage.ps1 -OutputDir "D:\Tools\CodexTokenUsage"
```

## Boundaries

- GitHub Latest Release is the primary version and download source.
- The stable entry is `CodexTokenUsage.exe` under the selected install directory.
- The installer script does not delete Codex original logs or this tool's SQLite ledger.
- Do not build from source unless the user explicitly asks for development or packaging work.
- If GitHub release metadata or the release asset is unavailable, report the failure instead of guessing another package.
