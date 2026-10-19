[CmdletBinding()]
param([string]$CorePath)
$ErrorActionPreference = 'Stop'
$taskHelper = Join-Path $PSScriptRoot 'localsend-qr.exe'
$taskArguments = @('context-menu', 'install')
if ($CorePath) { $taskArguments += @('--core-path', $CorePath) }
& $taskHelper @taskArguments
exit $LASTEXITCODE
