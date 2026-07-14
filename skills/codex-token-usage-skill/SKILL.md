---
name: codex-token-usage-skill
description: 下载、安装、启动或更新 Codex Token 用量统计 Windows 单文件工具。适用于打开、安装、更新、重新安装最新版便携版，或在新电脑上完成本地部署。
---

# Codex Token 用量统计

## 功能说明

此 Skill 从官方 GitHub Release 下载、安装、启动或更新最新版 Codex Token 用量统计 Windows 便携版应用。

它用于分发和使用桌面工具，不用于修改应用源码或调整 Token 统计逻辑。

## 适用场景

- 用户希望安装、下载、打开、启动、更新或重新安装 Codex Token 用量统计。
- 用户需要最新版 Windows 便携版安装包。
- 用户需要在新的 Windows 电脑上部署本工具。
- 用户希望使用稳定本地入口，而不是直接运行带版本号的 EXE 文件。

## 快速开始

运行随附的 PowerShell 安装脚本：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/download-codex-token-usage.ps1
```

默认行为：

1. 从 `DonaldL81/codex-token-usage` 读取最新 GitHub Release。
2. 下载 `CodexTokenUsage-*-windows-x64-portable.exe` 安装包。
3. 安装到 `%LOCALAPPDATA%\Programs\CodexTokenUsage\CodexTokenUsage.exe`。
4. 安装完成后自动启动稳定入口；仅在用户明确要求“仅下载”时使用 `-NoRun`。
5. 应用首次成功启动后，自动创建或更新桌面快捷方式 `Codex Token Usage.lnk`。

## 常用操作

打开已安装的稳定入口：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/download-codex-token-usage.ps1 -LaunchOnly
```

仅下载，不安装也不启动：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/download-codex-token-usage.ps1 -NoRun
```

强制重新下载：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/download-codex-token-usage.ps1 -Force
```

安装到自定义目录：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/download-codex-token-usage.ps1 -OutputDir "D:\Tools\CodexTokenUsage"
```

## 使用边界

- GitHub Latest Release 是版本与下载来源的唯一依据。
- 稳定入口是所选安装目录下的 `CodexTokenUsage.exe`。
- 安装脚本不会删除 Codex 原始日志，也不会删除本工具的 SQLite 账本。
- 除非用户明确要求开发或打包，否则不从源码构建。
- GitHub Release 元数据或安装包不可用时，应报告失败，不要猜测或替换为其他下载来源。
