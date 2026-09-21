<#
.SYNOPSIS
    광고 창 후보와 원래 창 상태를 읽기 전용으로 점검한다.

.DESCRIPTION
    지정한 PID의 조상·자손 프로세스를 찾고, 해당 프로세스가 소유한 최상위 창의
    클래스·소유자·도구 창 여부·표시 상태·좌표를 출력한다. IncludeChildWindows를
    지정하면 최상위 창 아래의 자식 창도 읽는다. AD-001 후보 판정과 AD-002 상태
    캡처에 필요한 정보를 확인하지만, 창을 숨기거나 이동하지 않는다.

.EXAMPLE
    pwsh -File .\scripts\Verify-AdWindowState.ps1 -ProcessId 26440

.EXAMPLE
    pwsh -File .\scripts\Verify-AdWindowState.ps1 -ProcessId 26440 -IncludeChildWindows

.EXAMPLE
    pwsh -File .\scripts\Verify-AdWindowState.ps1 -ProcessId 26440 -IncludeChildWindows -ClassFilter Chrome_WidgetWin_1
#>

[CmdletBinding()]
param(
    [Parameter()]
    [int]$ProcessId = 26440,

    [Parameter()]
    [switch]$IncludeChildWindows,

    [Parameter()]
    [string]$ClassFilter = 'Chrome_WidgetWin_1'
)

$ErrorActionPreference = 'Stop'

$processes = @(Get-CimInstance Win32_Process)
$byId = @{}
$children = @{}
foreach ($process in $processes) {
    $byId[[int]$process.ProcessId] = $process
    $parentId = [int]$process.ParentProcessId
    if (-not $children.ContainsKey($parentId)) {
        $children[$parentId] = [System.Collections.Generic.List[int]]::new()
    }
    $children[$parentId].Add([int]$process.ProcessId)
}

if (-not $byId.ContainsKey($ProcessId)) {
    throw "PID $ProcessId was not found in Win32_Process."
}

$relatedIds = [System.Collections.Generic.HashSet[int]]::new()
$null = $relatedIds.Add($ProcessId)

# 조상 프로세스
$cursor = $ProcessId
while ($byId.ContainsKey($cursor)) {
    $parentId = [int]$byId[$cursor].ParentProcessId
    if ($parentId -le 0 -or -not $relatedIds.Add($parentId)) {
        break
    }
    $cursor = $parentId
}

# 자손 프로세스
$pending = [System.Collections.Generic.Queue[int]]::new()
$pending.Enqueue($ProcessId)
while ($pending.Count -gt 0) {
    $parentId = $pending.Dequeue()
    if (-not $children.ContainsKey($parentId)) {
        continue
    }
    foreach ($childId in $children[$parentId]) {
        if ($relatedIds.Add($childId)) {
            $pending.Enqueue($childId)
        }
    }
}

# 창 열거는 앱 관계를 보여 주는 조상까지만 포함한다. 셸(explorer.exe)과 그 조상은
# 관련 PID 증거에는 남기되 창 목록에서 제외해 결과가 셸 창으로 오염되지 않게 한다.
$windowProcessIds = [System.Collections.Generic.HashSet[int]]::new()
$null = $windowProcessIds.Add($ProcessId)
$cursor = $ProcessId
while ($byId.ContainsKey($cursor)) {
    $parentId = [int]$byId[$cursor].ParentProcessId
    if ($parentId -le 0 -or -not $byId.ContainsKey($parentId)) {
        break
    }
    $parent = $byId[$parentId]
    if ($parent.Name -in @('explorer.exe', 'winlogon.exe', 'services.exe', 'smss.exe', 'csrss.exe', 'System')) {
        break
    }
    $null = $windowProcessIds.Add($parentId)
    $cursor = $parentId
}
$pending = [System.Collections.Generic.Queue[int]]::new()
$pending.Enqueue($ProcessId)
while ($pending.Count -gt 0) {
    $parentId = $pending.Dequeue()
    if (-not $children.ContainsKey($parentId)) {
        continue
    }
    foreach ($childId in $children[$parentId]) {
        if ($windowProcessIds.Add($childId)) {
            $pending.Enqueue($childId)
        }
    }
}

Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public static class AdWindowVerificationNative {
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

    [DllImport("user32.dll")]
    public static extern bool EnumWindows(EnumWindowsProc callback, IntPtr lParam);

    [DllImport("user32.dll")]
    public static extern bool EnumChildWindows(IntPtr parent, EnumWindowsProc callback, IntPtr lParam);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern int GetClassName(IntPtr hWnd, StringBuilder className, int maxCount);

    [DllImport("user32.dll")]
    public static extern IntPtr GetWindow(IntPtr hWnd, uint command);

    [DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW")]
    public static extern IntPtr GetWindowLongPtr(IntPtr hWnd, int index);

    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);

    [DllImport("user32.dll")]
    public static extern bool IsIconic(IntPtr hWnd);

    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr hWnd);

    [DllImport("user32.dll")]
    public static extern bool IsZoomed(IntPtr hWnd);

    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);

    [StructLayout(LayoutKind.Sequential)]
    public struct RECT {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }
}
'@

$windowRows = [System.Collections.Generic.List[object]]::new()
$topLevelHandles = [System.Collections.Generic.List[IntPtr]]::new()
$relatedLookup = [System.Collections.Generic.HashSet[int]]$windowProcessIds

function Get-WindowRow {
    param(
        [IntPtr]$Hwnd,
        [string]$Kind,
        [IntPtr]$ParentHwnd
    )

    [uint32]$windowProcessId = 0
    [void][AdWindowVerificationNative]::GetWindowThreadProcessId($Hwnd, [ref]$windowProcessId)
    if (-not $relatedLookup.Contains([int]$windowProcessId)) {
        return $null
    }

    $classNameBuffer = [System.Text.StringBuilder]::new(256)
    $classLength = [AdWindowVerificationNative]::GetClassName($Hwnd, $classNameBuffer, $classNameBuffer.Capacity)
    $className = if ($classLength -gt 0) { $classNameBuffer.ToString() } else { '' }
    $owner = [AdWindowVerificationNative]::GetWindow($Hwnd, 4) # GW_OWNER
    $exStyle = [Int64][AdWindowVerificationNative]::GetWindowLongPtr($Hwnd, -20) # GWL_EXSTYLE
    $visible = [AdWindowVerificationNative]::IsWindowVisible($Hwnd)
    $toolWindow = (($exStyle -band 0x80) -ne 0) # WS_EX_TOOLWINDOW

    $rect = [AdWindowVerificationNative+RECT]::new()
    $hasRect = [AdWindowVerificationNative]::GetWindowRect($Hwnd, [ref]$rect)
    $width = if ($hasRect) { $rect.Right - $rect.Left } else { 0 }
    $height = if ($hasRect) { $rect.Bottom - $rect.Top } else { 0 }
    $showState = if (-not $visible) { 'Hidden' } elseif ([AdWindowVerificationNative]::IsIconic($Hwnd)) { 'Minimized' } elseif ([AdWindowVerificationNative]::IsZoomed($Hwnd)) { 'Maximized' } else { 'Normal' }
    $classMatches = if ($ClassFilter -eq 'auto:webview') {
        $className -match '(?i)chrome_widgetwin_|webview'
    } else {
        $className.Equals($ClassFilter, [StringComparison]::OrdinalIgnoreCase)
    }
    $candidate = if ($Kind -eq 'Child') {
        $IncludeChildWindows -and $visible -and $classMatches -and $ParentHwnd -ne [IntPtr]::Zero -and $ClassFilter -ne 'auto:webview'
    } else {
        $visible -and $classMatches -and (($owner -ne [IntPtr]::Zero) -or $toolWindow)
    }

    [pscustomobject]@{
        WindowKind = $Kind
        Hwnd = ('0x{0:X}' -f $Hwnd.ToInt64())
        Parent = ('0x{0:X}' -f $ParentHwnd.ToInt64())
        Pid = [int]$windowProcessId
        Class = $className
        Visible = $visible
        Owner = ('0x{0:X}' -f $owner.ToInt64())
        ToolWindow = $toolWindow
        State = $showState
        X = if ($hasRect) { $rect.Left } else { $null }
        Y = if ($hasRect) { $rect.Top } else { $null }
        Width = $width
        Height = $height
        AdCandidate = $candidate
    }
}

$callback = [AdWindowVerificationNative+EnumWindowsProc] {
    param($hWnd, $unused)

    $row = Get-WindowRow -Hwnd $hWnd -Kind 'TopLevel' -ParentHwnd ([IntPtr]::Zero)
    if ($null -ne $row) {
        $windowRows.Add($row)
        $topLevelHandles.Add($hWnd)
    }

    return $true
}

[void][AdWindowVerificationNative]::EnumWindows($callback, [IntPtr]::Zero)

if ($IncludeChildWindows) {
    $childCallback = [AdWindowVerificationNative+EnumWindowsProc] {
        param($hWnd, $parentHwnd)

        $row = Get-WindowRow -Hwnd $hWnd -Kind 'Child' -ParentHwnd $parentHwnd
        if ($null -ne $row) {
            $windowRows.Add($row)
        }

        return $true
    }

    foreach ($topLevelHwnd in $topLevelHandles) {
        [void][AdWindowVerificationNative]::EnumChildWindows($topLevelHwnd, $childCallback, $topLevelHwnd)
    }
}

Write-Host "읽기 전용 광고 창 상태 검증"
Write-Host "기준 PID: $ProcessId"
Write-Host "클래스 필터: $ClassFilter"
Write-Host "관련 PID: $($relatedIds.Count)개 (조상·자손 포함)"
Write-Host ""

$processRows = foreach ($id in ($relatedIds | Sort-Object)) {
    $process = $byId[$id]
    [pscustomobject]@{
        Pid = $id
        ParentPid = [int]$process.ParentProcessId
        Name = $process.Name
        CommandLine = $process.CommandLine
    }
}
$processRows | Format-Table Pid, ParentPid, Name -AutoSize

if ($IncludeChildWindows) {
    Write-Host "기준 PID와 앱 조상·자손의 최상위·자식 창 (셸 조상 제외)"
} else {
    Write-Host "기준 PID와 앱 조상·자손의 최상위 창 (셸 조상 제외)"
}
if ($windowRows.Count -eq 0) {
    Write-Host '(없음)'
} else {
    $windowRows |
        Sort-Object Pid, WindowKind, Hwnd |
        Format-Table WindowKind, Hwnd, Parent, Pid, Class, Visible, Owner, ToolWindow, State, X, Y, Width, Height, AdCandidate -AutoSize |
        Out-String -Width 260 |
        Write-Host
}

Write-Host ""
Write-Host "AD-002 검증 범위: 상태 캡처에 필요한 정보만 읽었으며 창 조작 API는 호출하지 않음."
