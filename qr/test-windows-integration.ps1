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
$taskOldPause = $env:LOCALSEND_QR_NO_PAUSE
$taskOldPolicy = Get-ExecutionPolicy -Scope Process
$env:LOCALSEND_QR_NO_PAUSE = '1'
try {
    # Reproduce the user's blocked-script environment. Native CMD/EXE entry
    # points must still register, relocate and remove without changing policy.
    Set-ExecutionPolicy -Scope Process -ExecutionPolicy Restricted -Force
    foreach ($taskLocation in @($taskFirst, $taskSecond)) {
        & (Join-Path $taskLocation 'Register-ContextMenu.cmd')
        if ($LASTEXITCODE -ne 0) { throw 'CMD registration failed under Restricted policy' }
        $taskExpected = '"{0}" share --pause-on-error -- "%1"' -f (Join-Path $taskLocation 'localsend-qr.exe')
        foreach ($taskClass in @('*', 'Directory')) {
            $taskKey = 'HKCU:\Software\Classes\{0}\shell\LocalSendQR\command' -f $taskClass
            if ((Get-Item -LiteralPath $taskKey).GetValue('') -ne $taskExpected) { throw 'Wrong path after registration or relocation' }
        }
    }
    if ((Get-ExecutionPolicy -Scope Process) -ne 'Restricted') { throw 'Registration changed execution policy' }
    & (Join-Path $taskSecond 'localsend-qr.exe') context-menu install --core-path (Join-Path $taskFirst 'localsend-qr.exe')
    if ($LASTEXITCODE -ne 0) { throw 'Explicit core path registration failed' }
    $taskOverrideExpected = '"{0}" share --pause-on-error -- "%1"' -f (Join-Path $taskFirst 'localsend-qr.exe')
    if ((Get-Item -LiteralPath 'HKCU:\Software\Classes\*\shell\LocalSendQR\command').GetValue('') -ne $taskOverrideExpected) { throw 'Core override was not registered' }
    Set-ExecutionPolicy -Scope Process -ExecutionPolicy $taskOldPolicy -Force
    # A child -File invocation must resolve its own directory, unlike the
    # v0.1.0 parameter initializer that could see an empty PSScriptRoot.
    & powershell.exe -NoLogo -NoProfile -NonInteractive -File (Join-Path $taskSecond 'Register-ContextMenu.ps1')
    if ($LASTEXITCODE -ne 0) { throw 'Standalone -File registration failed' }
} finally {
    & (Join-Path $taskSecond 'Remove-ContextMenu.cmd')
    Set-ExecutionPolicy -Scope Process -ExecutionPolicy $taskOldPolicy -Force
    $env:LOCALSEND_QR_NO_PAUSE = $taskOldPause
}
foreach ($taskClass in @('*', 'Directory')) {
    if (Test-Path -LiteralPath ('HKCU:\Software\Classes\{0}\shell\LocalSendQR' -f $taskClass)) { throw 'Unregistration failed' }
}
Write-Host 'PASS: CMD registration under Restricted policy, relocation, core override, standalone -File and removal.'
