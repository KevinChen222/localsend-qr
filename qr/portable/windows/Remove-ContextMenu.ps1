[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
foreach ($taskClass in @('*', 'Directory')) {
    $taskKey = 'HKCU:\Software\Classes\{0}\shell\LocalSendQR' -f $taskClass
    if (Test-Path -LiteralPath $taskKey) { Remove-Item -LiteralPath $taskKey -Recurse -Force }
}
Write-Host 'LocalSend QR context menu removed for the current user.'
