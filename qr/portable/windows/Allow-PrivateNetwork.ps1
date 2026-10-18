# Optional. Run this script in an Administrator PowerShell window.
# Firewall changes are deliberately separate from context-menu registration.
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$taskCore = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot 'localsend-qr.exe')).ProviderPath
$taskRule = 'LocalSendQR-Portable-Private'
if (Get-NetFirewallRule -Name $taskRule -ErrorAction SilentlyContinue) {
    Get-NetFirewallRule -Name $taskRule | Get-NetFirewallApplicationFilter | Set-NetFirewallApplicationFilter -Program $taskCore | Out-Null
} else {
    New-NetFirewallRule -Name $taskRule -DisplayName 'LocalSend QR (Private LAN)' -Direction Inbound -Action Allow -Program $taskCore -Protocol TCP -Profile Private -RemoteAddress LocalSubnet | Out-Null
}
Write-Host 'Allowed on Private networks for local-subnet clients only.'
