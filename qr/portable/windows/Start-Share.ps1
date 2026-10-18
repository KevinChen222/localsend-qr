[CmdletBinding()]
param([Parameter(Position=0, Mandatory=$true, ValueFromRemainingArguments=$true)][string[]]$Paths)
$ErrorActionPreference = 'Stop'
$taskCore = Join-Path $PSScriptRoot 'localsend-qr.exe'
& $taskCore share --pause-on-error -- @Paths
exit $LASTEXITCODE
