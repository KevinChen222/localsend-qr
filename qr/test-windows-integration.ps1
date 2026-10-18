$ErrorActionPreference = 'Stop'
$taskBundle = Get-ChildItem -LiteralPath (Join-Path $PSScriptRoot '../dist') -Directory | Where-Object Name -Like '*windows-x64' | Select-Object -First 1
if (-not $taskBundle) { throw 'Portable bundle missing' }
$taskScratch = Join-Path ([IO.Path]::GetTempPath()) ('localsend-qr-shell-' + [guid]::NewGuid().ToString('N'))
$taskFirst = Join-Path $taskScratch 'first [folder] & space'
$taskSecond = Join-Path $taskScratch 'moved folder'
New-Item -ItemType Directory -Path $taskFirst, $taskSecond -Force | Out-Null
foreach ($taskDestination in @($taskFirst, $taskSecond)) {
    Get-ChildItem -LiteralPath $taskBundle.FullName -File | ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $taskDestination }
}
try {
    foreach ($taskLocation in @($taskFirst, $taskSecond)) {
        & (Join-Path $taskLocation 'Register-ContextMenu.ps1')
        $taskExpected = '"{0}" share --pause-on-error -- "%1"' -f (Join-Path $taskLocation 'localsend-qr.exe')
        foreach ($taskClass in @('*', 'Directory')) {
            $taskKey = 'HKCU:\Software\Classes\{0}\shell\LocalSendQR\command' -f $taskClass
            if ((Get-Item -LiteralPath $taskKey).GetValue('') -ne $taskExpected) { throw 'Wrong path after registration or relocation' }
        }
    }
} finally {
    & (Join-Path $taskSecond 'Remove-ContextMenu.ps1')
}
foreach ($taskClass in @('*', 'Directory')) {
    if (Test-Path -LiteralPath ('HKCU:\Software\Classes\{0}\shell\LocalSendQR' -f $taskClass)) { throw 'Unregistration failed' }
}
Write-Host 'PASS: Windows PowerShell 5.1 registration, quoted paths, relocation and removal.'
