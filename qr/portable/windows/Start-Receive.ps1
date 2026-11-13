[CmdletBinding()]
param([Parameter(Position=0)][string]$Directory = '.')
$ErrorActionPreference = 'Stop'
$taskCore = Join-Path $PSScriptRoot 'localsend-qr.exe'
& $taskCore receive --pause-on-error -- $Directory
exit $LASTEXITCODE
