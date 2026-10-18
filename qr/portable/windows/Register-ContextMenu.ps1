[CmdletBinding()]
param([string]$CorePath = (Join-Path $PSScriptRoot 'localsend-qr.exe'))
$ErrorActionPreference = 'Stop'
$taskCore = (Resolve-Path -LiteralPath $CorePath).ProviderPath
if (-not (Test-Path -LiteralPath $taskCore -PathType Leaf)) { throw 'Core executable not found' }
$taskCommand = '"{0}" share --pause-on-error -- "%1"' -f $taskCore
foreach ($taskClass in @('*', 'Directory')) {
    $taskKey = 'HKCU:\Software\Classes\{0}\shell\LocalSendQR' -f $taskClass
    New-Item -Path $taskKey -Force | Out-Null
    Set-Item -LiteralPath $taskKey -Value ([string]::Concat([char]0x901A,[char]0x8FC7,[char]0x4E8C,[char]0x7EF4,[char]0x7801,[char]0x4F20,[char]0x9001))
    New-ItemProperty -LiteralPath $taskKey -Name 'Icon' -Value ('"{0}",0' -f $taskCore) -PropertyType String -Force | Out-Null
    New-ItemProperty -LiteralPath $taskKey -Name 'MultiSelectModel' -Value 'Single' -PropertyType String -Force | Out-Null
    New-Item -Path ($taskKey + '\command') -Force | Out-Null
    Set-Item -LiteralPath ($taskKey + '\command') -Value $taskCommand
    if ((Get-Item -LiteralPath ($taskKey + '\command')).GetValue('') -ne $taskCommand) { throw 'Context menu verification failed' }
}
Write-Host "Registered for the current user: $taskCore"
Write-Host 'Windows 11: use Show more options. After moving the portable folder, run this script again.'
