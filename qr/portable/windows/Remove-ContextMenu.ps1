[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
& (Join-Path $PSScriptRoot 'localsend-qr.exe') context-menu remove
exit $LASTEXITCODE
