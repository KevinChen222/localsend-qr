@echo off
setlocal DisableDelayedExpansion
"%~dp0localsend-qr.exe" context-menu remove
set "taskExitCode=%ERRORLEVEL%"
if not defined LOCALSEND_QR_NO_PAUSE pause
exit /b %taskExitCode%
