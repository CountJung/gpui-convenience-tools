<#
.SYNOPSIS
    Claude Code(Windows)용 로컬 시각 검증 하네스.

.DESCRIPTION
    Claude Code에는 ChatGPT 데스크톱의 Computer Use(`sky.*`) 같은 화면 조작 도구가 없다.
    대신 이 스크립트가 Win32 `PrintWindow` + `SendInput`으로 창 단위 캡처와 입력을 제공하고,
    Claude는 저장된 PNG를 Read 도구로 직접 관찰한다. 이것은 Computer Use 우회가 아니라
    Claude 표면에서 유일하게 존재하는 실제 화면 검증 경로다.

    전체 데스크톱을 캡처하지 않는다. `PW_RENDERFULLCONTENT`로 대상 창만 캡처하므로
    다른 창에 가려져 있어도 되고, 사용자의 다른 화면 내용은 파일에 남지 않는다.

.PARAMETER Action
    Start   해시를 기록한 바이너리를 작업 전용 임시 `APPDATA`에서 실행하고 세션을 만든다.
    Capture 대상 창만 PNG로 캡처한다.
    Wheel   대상 창의 지정 지점에 휠 입력을 보낸다.
    Click   대상 창의 지정 지점을 좌클릭한다.
    Resize  대상 창 크기를 바꾼다(최소 지원 크기 회귀 확인용).
    Stop    기록된 PID와 작업 전용 임시 루트만 정리한다.

.EXAMPLE
    ./Invoke-ClaudeVisualCheck.ps1 -Action Start
    ./Invoke-ClaudeVisualCheck.ps1 -Action Capture -Name sidebar-before
    ./Invoke-ClaudeVisualCheck.ps1 -Action Wheel -X 0.12 -Y 0.65 -Delta -8
    ./Invoke-ClaudeVisualCheck.ps1 -Action Capture -Name sidebar-after
    ./Invoke-ClaudeVisualCheck.ps1 -Action Stop
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("Start", "Capture", "Wheel", "Click", "Resize", "Stop")]
    [string]$Action,

    [string]$BinaryPath,

    # Start 전용. 격리된 데이터 루트에 미리 넣어 둘 `config.json` 경로.
    # 특정 상태(예: 동기화 작업이 등록된 화면)를 재현할 때 쓴다.
    [string]$SeedConfig,

    [string]$Name = "capture",
    [double]$X = 0.5,
    [double]$Y = 0.5,
    [int]$Delta = -3,
    [int]$Width = 1000,
    [int]$Height = 700,
    [int]$SettleMs = 700
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version 2.0

$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$validationRoot = Join-Path $repoRoot "target\visual-validation"
$sessionPath = Join-Path $validationRoot "claude-session.json"
$captureRoot = Join-Path $validationRoot "captures"
$tempPrefix = "gpui-claude-visual-"

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class ClaudeVisualInterop {
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hwnd, IntPtr hdc, uint flags);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hwnd, out RECT rect);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr hwnd, ref POINT point);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr hwnd, int x, int y, int w, int h, bool repaint);
    [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hwnd, int cmd);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, IntPtr pid);
    [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint from, uint to, bool attach);
    [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT point);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll", SetLastError = true)] public static extern uint SendInput(uint count, INPUT[] inputs, int size);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT {
        public int dx, dy; public uint mouseData, dwFlags, time; public IntPtr extraInfo;
    }
    // x64에서 INPUT은 정확히 40바이트여야 한다. 필드를 더하면 SendInput이 조용히 0을 반환한다.
    [StructLayout(LayoutKind.Sequential)] public struct INPUT { public uint type; public MOUSEINPUT mi; }
}
"@

[void][ClaudeVisualInterop]::SetProcessDPIAware()

$PW_RENDERFULLCONTENT = 2
$MOUSEEVENTF_WHEEL = 0x0800
$MOUSEEVENTF_LEFTDOWN = 0x0002
$MOUSEEVENTF_LEFTUP = 0x0004

# `ConvertFrom-Json`은 ISO-8601 문자열을 Kind=Unspecified인 DateTime으로 이미 변환해 둔다.
# 그 값을 다시 문자열로 만들어 `Parse(...).ToUniversalTime()` 하면 UTC 값을 현지 시각으로
# 오해해 시간대만큼 어긋난다(KST에서 9시간). 항상 이 함수로 UTC 인스턴트를 얻는다.
function ConvertTo-UtcInstant($value) {
    if ($value -is [DateTime]) {
        if ($value.Kind -eq [DateTimeKind]::Unspecified) {
            return [DateTime]::SpecifyKind($value, [DateTimeKind]::Utc)
        }
        return $value.ToUniversalTime()
    }
    return [DateTime]::Parse(
        [string]$value,
        [Globalization.CultureInfo]::InvariantCulture,
        [Globalization.DateTimeStyles]::AdjustToUniversal -bor
            [Globalization.DateTimeStyles]::AssumeUniversal)
}

function Read-Session {
    if (-not (Test-Path -LiteralPath $sessionPath)) {
        throw "활성 세션이 없다. 먼저 -Action Start 를 실행할 것."
    }
    $session = Get-Content -Raw -LiteralPath $sessionPath | ConvertFrom-Json
    if ($session.state -ne "ACTIVE") {
        throw "세션 상태가 ACTIVE가 아니다: $($session.state)"
    }
    $hwnd = [IntPtr][int64]$session.windowHandle
    if (-not [ClaudeVisualInterop]::IsWindow($hwnd)) {
        throw "기록된 창 핸들이 더 이상 유효하지 않다. -Action Stop 후 다시 시작할 것."
    }
    return $session
}

function Get-WindowRect([IntPtr]$hwnd) {
    $rect = New-Object ClaudeVisualInterop+RECT
    if (-not [ClaudeVisualInterop]::GetWindowRect($hwnd, [ref]$rect)) {
        throw "GetWindowRect 실패."
    }
    return $rect
}

# 입력 좌표는 클라이언트 영역 기준 0~1 비율이다. 창 크기가 바뀌어도 같은 지점을 가리킨다.
function Resolve-ClientPoint([IntPtr]$hwnd, [double]$fx, [double]$fy) {
    if ($fx -lt 0 -or $fx -gt 1 -or $fy -lt 0 -or $fy -gt 1) {
        throw "X·Y는 클라이언트 영역 기준 0~1 비율이어야 한다."
    }
    $client = New-Object ClaudeVisualInterop+RECT
    if (-not [ClaudeVisualInterop]::GetClientRect($hwnd, [ref]$client)) {
        throw "GetClientRect 실패."
    }
    $point = New-Object ClaudeVisualInterop+POINT
    $point.X = [int]([Math]::Round(($client.Right - $client.Left) * $fx))
    $point.Y = [int]([Math]::Round(($client.Bottom - $client.Top) * $fy))
    if (-not [ClaudeVisualInterop]::ClientToScreen($hwnd, [ref]$point)) {
        throw "ClientToScreen 실패."
    }
    return $point
}

# SendInput은 데스크톱 전역 입력이다. 대상 창이 포그라운드가 아니면 사용자가 보고 있는 다른
# 창으로 입력이 새어 들어가므로, 확인에 실패하면 입력을 보내지 않고 중단한다.
function Assert-ForegroundTarget([IntPtr]$hwnd) {
    $SW_RESTORE = 9
    $currentThread = [ClaudeVisualInterop]::GetCurrentThreadId()

    for ($attempt = 0; $attempt -lt 5; $attempt++) {
        $foreground = [ClaudeVisualInterop]::GetForegroundWindow()
        if ($foreground -eq $hwnd) { return }

        # Windows는 포그라운드를 소유하지 않은 프로세스의 `SetForegroundWindow`를 무시한다.
        # 현재 포그라운드 스레드에 입력 큐를 붙이는 동안에만 전환이 허용된다.
        $foregroundThread = [ClaudeVisualInterop]::GetWindowThreadProcessId($foreground, [IntPtr]::Zero)
        $attached = $false
        if ($foregroundThread -ne 0 -and $foregroundThread -ne $currentThread) {
            $attached = [ClaudeVisualInterop]::AttachThreadInput($currentThread, $foregroundThread, $true)
        }
        try {
            [void][ClaudeVisualInterop]::ShowWindow($hwnd, $SW_RESTORE)
            [void][ClaudeVisualInterop]::BringWindowToTop($hwnd)
            [void][ClaudeVisualInterop]::SetForegroundWindow($hwnd)
        }
        finally {
            if ($attached) {
                [void][ClaudeVisualInterop]::AttachThreadInput($currentThread, $foregroundThread, $false)
            }
        }
        Start-Sleep -Milliseconds 400
    }

    $foreground = [ClaudeVisualInterop]::GetForegroundWindow()
    if ($foreground -ne $hwnd) {
        throw ("대상 창을 포그라운드로 올리지 못했다(현재 포그라운드=$foreground). " +
            "다른 창으로 입력이 새는 것을 막기 위해 입력을 보내지 않았다.")
    }
}

function Send-MouseInput([uint32]$flags, [uint32]$data) {
    $input = New-Object ClaudeVisualInterop+INPUT
    $input.type = 0
    $mouse = New-Object ClaudeVisualInterop+MOUSEINPUT
    $mouse.dwFlags = $flags
    $mouse.mouseData = $data
    $input.mi = $mouse
    $size = [Runtime.InteropServices.Marshal]::SizeOf([Type]'ClaudeVisualInterop+INPUT')
    $sent = [ClaudeVisualInterop]::SendInput(1, @($input), $size)
    if ($sent -ne 1) {
        throw ("SendInput이 입력을 받아들이지 않았다(sent=$sent, " +
            "err=$([Runtime.InteropServices.Marshal]::GetLastWin32Error())). " +
            "대상 앱이 관리자 권한으로 실행 중이면 UIPI가 입력을 차단한다.")
    }
}

function Save-WindowCapture([IntPtr]$hwnd, [string]$captureName) {
    New-Item -ItemType Directory -Force -Path $captureRoot | Out-Null
    $rect = Get-WindowRect $hwnd
    $w = $rect.Right - $rect.Left
    $h = $rect.Bottom - $rect.Top
    if ($w -le 0 -or $h -le 0) { throw "창 크기가 유효하지 않다: ${w}x${h}" }

    $bitmap = New-Object System.Drawing.Bitmap $w, $h
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $hdc = $graphics.GetHdc()
    try {
        # PW_RENDERFULLCONTENT가 있어야 GPU 렌더링(GPUI/Blade) 내용이 비트맵에 들어온다.
        $ok = [ClaudeVisualInterop]::PrintWindow($hwnd, $hdc, $PW_RENDERFULLCONTENT)
    }
    finally {
        $graphics.ReleaseHdc($hdc)
        $graphics.Dispose()
    }
    if (-not $ok) { $bitmap.Dispose(); throw "PrintWindow 실패." }

    $stamp = [DateTime]::Now.ToString("HHmmss")
    $path = Join-Path $captureRoot ("{0}-{1}.png" -f $captureName, $stamp)
    $bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $bitmap.Dispose()

    return [ordered]@{
        capture = $path
        window = ("{0}x{1}" -f $w, $h)
        capturedAtUtc = [DateTime]::UtcNow.ToString("o")
    }
}

switch ($Action) {
    "Start" {
        if (Test-Path -LiteralPath $sessionPath) {
            $existing = Get-Content -Raw -LiteralPath $sessionPath | ConvertFrom-Json
            if ($existing.state -eq "ACTIVE") {
                throw "이미 활성 세션이 있다. -Action Stop 으로 정리한 뒤 다시 시작할 것."
            }
        }

        if ([string]::IsNullOrWhiteSpace($BinaryPath)) {
            $BinaryPath = Join-Path $repoRoot "target\release\gpui-convenience-tools.exe"
        }
        $BinaryPath = [System.IO.Path]::GetFullPath($BinaryPath)
        if (-not (Test-Path -LiteralPath $BinaryPath -PathType Leaf)) {
            throw "검증 대상 바이너리가 없다: $BinaryPath (cargo build --release 먼저 실행)"
        }
        $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $BinaryPath).Hash.ToLowerInvariant()

        $sessionId = [Guid]::NewGuid().ToString("N")
        $sessionRoot = Join-Path ([System.IO.Path]::GetTempPath()) ($tempPrefix + $sessionId)
        $appData = Join-Path $sessionRoot "appdata"
        New-Item -ItemType Directory -Force -Path $appData | Out-Null
        New-Item -ItemType Directory -Force -Path (Join-Path $sessionRoot "source") | Out-Null
        New-Item -ItemType Directory -Force -Path (Join-Path $sessionRoot "target") | Out-Null

        if (-not [string]::IsNullOrWhiteSpace($SeedConfig)) {
            $seedPath = [System.IO.Path]::GetFullPath($SeedConfig)
            if (-not (Test-Path -LiteralPath $seedPath -PathType Leaf)) {
                throw "시드 config를 찾을 수 없다: $seedPath"
            }
            Copy-Item -LiteralPath $seedPath -Destination (Join-Path $appData "config.json") -Force
        }

        # 앱은 `dirs::config_dir()`(= `SHGetKnownFolderPath`)로 데이터 루트를 찾으므로
        # `APPDATA`만 바꿔서는 격리되지 않는다. 전용 변수를 프로세스 범위로 지정해야
        # 사용자의 실제 config.json·로그를 건드리지 않는다.
        $previousAppData = $env:APPDATA
        $previousDataDir = $env:GPUI_CONVENIENCE_TOOLS_DATA_DIR
        try {
            $env:APPDATA = $appData
            $env:GPUI_CONVENIENCE_TOOLS_DATA_DIR = $appData
            $process = Start-Process -FilePath $BinaryPath -PassThru
        }
        finally {
            if ($null -eq $previousAppData) { Remove-Item Env:\APPDATA -ErrorAction SilentlyContinue }
            else { $env:APPDATA = $previousAppData }
            if ($null -eq $previousDataDir) {
                Remove-Item Env:\GPUI_CONVENIENCE_TOOLS_DATA_DIR -ErrorAction SilentlyContinue
            }
            else { $env:GPUI_CONVENIENCE_TOOLS_DATA_DIR = $previousDataDir }
        }

        $hwnd = [IntPtr]::Zero
        for ($i = 0; $i -lt 60; $i++) {
            Start-Sleep -Milliseconds 500
            $process.Refresh()
            if ($process.HasExited) { throw "검증 프로세스가 창을 만들기 전에 종료됐다." }
            if ($process.MainWindowHandle -ne [IntPtr]::Zero) { $hwnd = $process.MainWindowHandle; break }
        }
        if ($hwnd -eq [IntPtr]::Zero) {
            Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
            Remove-Item -Recurse -Force $sessionRoot -ErrorAction SilentlyContinue
            throw "30초 안에 주 창이 나타나지 않았다."
        }

        [void][ClaudeVisualInterop]::MoveWindow($hwnd, 120, 120, $Width, $Height, $true)
        Start-Sleep -Milliseconds $SettleMs
        $rect = Get-WindowRect $hwnd

        New-Item -ItemType Directory -Force -Path $validationRoot | Out-Null
        $session = [ordered]@{
            state = "ACTIVE"
            surface = "CLAUDE_LOCAL"
            processId = $process.Id
            processStartTimeUtc = $process.StartTime.ToUniversalTime().ToString("o")
            windowHandle = [int64]$hwnd
            windowTitle = $process.MainWindowTitle
            windowSize = ("{0}x{1}" -f ($rect.Right - $rect.Left), ($rect.Bottom - $rect.Top))
            binaryPath = $BinaryPath
            binarySha256 = $hash
            sessionRoot = $sessionRoot
            appData = $appData
            captureRoot = $captureRoot
            startedAtUtc = [DateTime]::UtcNow.ToString("o")
        }
        $session | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $sessionPath -Encoding UTF8
        $session | ConvertTo-Json -Depth 4
    }

    "Capture" {
        $session = Read-Session
        $hwnd = [IntPtr][int64]$session.windowHandle
        Save-WindowCapture $hwnd $Name | ConvertTo-Json -Depth 3
    }

    "Wheel" {
        $session = Read-Session
        $hwnd = [IntPtr][int64]$session.windowHandle
        Assert-ForegroundTarget $hwnd

        $origin = New-Object ClaudeVisualInterop+POINT
        [void][ClaudeVisualInterop]::GetCursorPos([ref]$origin)
        $point = Resolve-ClientPoint $hwnd $X $Y
        [void][ClaudeVisualInterop]::SetCursorPos($point.X, $point.Y)
        Start-Sleep -Milliseconds 250

        $notches = [Math]::Abs($Delta)
        $step = if ($Delta -lt 0) { [uint32](4294967296 - 120) } else { [uint32]120 }
        try {
            for ($i = 0; $i -lt $notches; $i++) {
                Send-MouseInput $MOUSEEVENTF_WHEEL $step
                Start-Sleep -Milliseconds 120
            }
        }
        finally {
            [void][ClaudeVisualInterop]::SetCursorPos($origin.X, $origin.Y)
        }
        Start-Sleep -Milliseconds $SettleMs
        [ordered]@{
            action = "Wheel"; at = ("{0},{1}" -f $point.X, $point.Y)
            notches = $notches; direction = $(if ($Delta -lt 0) { "down" } else { "up" })
        } | ConvertTo-Json -Depth 3
    }

    "Click" {
        $session = Read-Session
        $hwnd = [IntPtr][int64]$session.windowHandle
        Assert-ForegroundTarget $hwnd

        $origin = New-Object ClaudeVisualInterop+POINT
        [void][ClaudeVisualInterop]::GetCursorPos([ref]$origin)
        $point = Resolve-ClientPoint $hwnd $X $Y
        [void][ClaudeVisualInterop]::SetCursorPos($point.X, $point.Y)
        Start-Sleep -Milliseconds 250
        try {
            Send-MouseInput $MOUSEEVENTF_LEFTDOWN 0
            Start-Sleep -Milliseconds 60
            Send-MouseInput $MOUSEEVENTF_LEFTUP 0
        }
        finally {
            [void][ClaudeVisualInterop]::SetCursorPos($origin.X, $origin.Y)
        }
        Start-Sleep -Milliseconds $SettleMs
        [ordered]@{ action = "Click"; at = ("{0},{1}" -f $point.X, $point.Y) } | ConvertTo-Json -Depth 3
    }

    "Resize" {
        $session = Read-Session
        $hwnd = [IntPtr][int64]$session.windowHandle
        if (-not [ClaudeVisualInterop]::MoveWindow($hwnd, 120, 120, $Width, $Height, $true)) {
            throw "MoveWindow 실패."
        }
        Start-Sleep -Milliseconds $SettleMs
        $rect = Get-WindowRect $hwnd
        [ordered]@{
            action = "Resize"
            requested = ("{0}x{1}" -f $Width, $Height)
            actual = ("{0}x{1}" -f ($rect.Right - $rect.Left), ($rect.Bottom - $rect.Top))
        } | ConvertTo-Json -Depth 3
    }

    "Stop" {
        if (-not (Test-Path -LiteralPath $sessionPath)) {
            [ordered]@{ state = "CLEANED"; note = "정리할 세션이 없다." } | ConvertTo-Json -Depth 2
            break
        }
        $session = Get-Content -Raw -LiteralPath $sessionPath | ConvertFrom-Json
        $sessionRoot = [System.IO.Path]::GetFullPath([string]$session.sessionRoot)
        $tempRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath()).TrimEnd("\") + "\"

        # 기록된 임시 루트 밖이나 이름이 다른 디렉터리는 절대 지우지 않는다.
        if (-not $sessionRoot.StartsWith($tempRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "임시 디렉터리 밖의 세션은 정리하지 않는다: $sessionRoot"
        }
        if (-not ([System.IO.Path]::GetFileName($sessionRoot)).StartsWith($tempPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "예상하지 못한 세션 디렉터리는 정리하지 않는다: $sessionRoot"
        }

        # PID 재사용으로 남의 프로세스를 죽이지 않도록 경로와 시작 시각을 함께 확인한다.
        $process = Get-Process -Id ([int]$session.processId) -ErrorAction SilentlyContinue
        if ($null -ne $process) {
            $expectedBinary = [System.IO.Path]::GetFullPath([string]$session.binaryPath)
            $actualBinary = [System.IO.Path]::GetFullPath([string]$process.Path)
            if (-not $actualBinary.Equals($expectedBinary, [System.StringComparison]::OrdinalIgnoreCase)) {
                throw "PID가 다른 실행 파일의 것이다. 종료하지 않는다."
            }
            $expectedStart = ConvertTo-UtcInstant $session.processStartTimeUtc
            if ([Math]::Abs(($process.StartTime.ToUniversalTime() - $expectedStart).TotalSeconds) -gt 1.0) {
                throw "PID 시작 시각이 기록과 다르다. 종료하지 않는다."
            }
            Stop-Process -Id $process.Id -Force
            $process.WaitForExit()
        }

        if (Test-Path -LiteralPath $sessionRoot) {
            Remove-Item -LiteralPath $sessionRoot -Recurse -Force
        }

        $result = [ordered]@{
            state = "CLEANED"
            processId = [int]$session.processId
            sessionRoot = $sessionRoot
            sessionRootExists = (Test-Path -LiteralPath $sessionRoot)
            captureRoot = $captureRoot
            cleanedAtUtc = [DateTime]::UtcNow.ToString("o")
        }
        $result | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath $sessionPath -Encoding UTF8
        $result | ConvertTo-Json -Depth 3
    }
}
