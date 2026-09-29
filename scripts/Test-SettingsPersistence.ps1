[CmdletBinding()]
param(
    [string]$BinaryPath = (Join-Path (Split-Path $PSScriptRoot -Parent) 'target\release\gpui-convenience-tools.exe')
)

$ErrorActionPreference = 'Stop'
$binary = [IO.Path]::GetFullPath($BinaryPath)
if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) { throw "Release binary not found: $binary" }

$tempBase = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$testRoot = [IO.Path]::GetFullPath((Join-Path $tempBase ('gct-settings-e2e-' + [guid]::NewGuid().ToString('N'))))
if (-not $testRoot.StartsWith($tempBase, [StringComparison]::OrdinalIgnoreCase) -or
    -not [IO.Path]::GetFileName($testRoot).StartsWith('gct-settings-e2e-')) {
    throw "Unsafe test root: $testRoot"
}

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class GctSettingsE2EWindow {
    public delegate bool EnumProc(IntPtr hwnd, IntPtr context);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc callback, IntPtr context);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint processId);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd, uint message, IntPtr wParam, IntPtr lParam);
}
'@

function Start-And-Close-IsolatedApp {
    param([string]$Executable, [string]$DataRoot)
    $previousDataRoot = $env:GPUI_CONVENIENCE_TOOLS_DATA_DIR
    try {
        $env:GPUI_CONVENIENCE_TOOLS_DATA_DIR = $DataRoot
        $process = Start-Process -FilePath $Executable -ArgumentList '--tray' -WindowStyle Hidden -PassThru
    }
    finally {
        if ($null -eq $previousDataRoot) {
            Remove-Item Env:\GPUI_CONVENIENCE_TOOLS_DATA_DIR -ErrorAction SilentlyContinue
        } else {
            $env:GPUI_CONVENIENCE_TOOLS_DATA_DIR = $previousDataRoot
        }
    }

    try {
        Start-Sleep -Seconds 2
        if ($process.HasExited) { throw "Isolated app exited before validation: $($process.ExitCode)" }
        $actual = (Get-CimInstance Win32_Process -Filter "ProcessId = $($process.Id)").ExecutablePath
        if (-not $actual.Equals($Executable, [StringComparison]::OrdinalIgnoreCase)) {
            throw "Unexpected validation process path: $actual"
        }
        $pidToClose = [uint32]$process.Id
        $script:posted = 0
        $callback = [GctSettingsE2EWindow+EnumProc]{
            param($hwnd, $context)
            $ownerPid = [uint32]0
            [void][GctSettingsE2EWindow]::GetWindowThreadProcessId($hwnd, [ref]$ownerPid)
            if ($ownerPid -eq $pidToClose -and
                [GctSettingsE2EWindow]::PostMessage($hwnd, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)) {
                $script:posted++
            }
            return $true
        }
        [void][GctSettingsE2EWindow]::EnumWindows($callback, [IntPtr]::Zero)
        if ($script:posted -eq 0) { throw 'No validation window received a normal close request' }
        if (-not $process.WaitForExit(10000)) { throw 'Isolated app did not exit normally' }
        return $process.Id
    }
    finally {
        if (-not $process.HasExited) {
            Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
            $process.WaitForExit()
        }
    }
}

New-Item -ItemType Directory -Path $testRoot | Out-Null
try {
    $legacyPath = Join-Path $testRoot 'config.json'
    $settingsPath = Join-Path $testRoot 'settings.json'
    $seed = [ordered]@{
        service_enabled = $false
        targets = @()
        kakao_reclaim_ad_space = $true
        theme_mode = 'dark'
        scan_interval_secs = 37
        favorite_services = @('fixture-service')
        sync_enabled = $false
    } | ConvertTo-Json -Depth 5
    $malformedLegacy = $seed + '}'
    [IO.File]::WriteAllText($legacyPath, $malformedLegacy)

    $firstPid = Start-And-Close-IsolatedApp -Executable $binary -DataRoot $testRoot
    if (-not (Test-Path -LiteralPath $settingsPath -PathType Leaf)) {
        throw 'First app exit did not create settings.json'
    }
    $first = Get-Content -LiteralPath $settingsPath -Raw | ConvertFrom-Json
    if (-not $first.kakao_reclaim_ad_space -or $first.scan_interval_secs -ne 37 -or
        $first.service_enabled -or $first.sync_enabled -or $first.theme_mode -ne 'dark' -or
        $first.favorite_services[0] -ne 'fixture-service') {
        throw 'First app exit did not preserve all seeded settings'
    }
    if ([IO.File]::ReadAllText($legacyPath) -cne $malformedLegacy) {
        throw 'Malformed legacy config was modified'
    }

    $secondPid = Start-And-Close-IsolatedApp -Executable $binary -DataRoot $testRoot
    $second = Get-Content -LiteralPath $settingsPath -Raw | ConvertFrom-Json
    if (-not $second.kakao_reclaim_ad_space -or $second.scan_interval_secs -ne 37 -or
        $second.service_enabled -or $second.sync_enabled -or $second.theme_mode -ne 'dark' -or
        $second.favorite_services[0] -ne 'fixture-service') {
        throw 'Restart did not restore the saved settings'
    }
    if ([IO.File]::ReadAllText($legacyPath) -cne $malformedLegacy) {
        throw 'Restart modified the malformed legacy config'
    }

    [pscustomobject]@{
        Result = 'SETTINGS_E2E_PASSED'
        FirstProcessId = $firstPid
        SecondProcessId = $secondPid
        LegacyPreserved = $true
        ReclaimEnabledAfterRestart = [bool]$second.kakao_reclaim_ad_space
        ScanIntervalAfterRestart = [int]$second.scan_interval_secs
        BinarySha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $binary).Hash
    }
}
finally {
    if (Test-Path -LiteralPath $testRoot) {
        Remove-Item -LiteralPath $testRoot -Recurse -Force
    }
}
