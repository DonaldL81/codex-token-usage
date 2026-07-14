@echo off
setlocal

cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0app\scripts\One-Click-Package.ps1" %*
set "EXIT_CODE=%ERRORLEVEL%"

echo.
if "%EXIT_CODE%"=="0" (
  echo One-click package finished.
) else (
  echo One-click package failed with exit code %EXIT_CODE%.
)

if not defined CODEX_TOKEN_USAGE_NO_PAUSE pause
exit /b %EXIT_CODE%
