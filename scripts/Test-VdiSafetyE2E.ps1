<#
.SYNOPSIS
    실제 VM을 건드리지 않는 O-5 릴리즈 프로세스 경계 검증.
.DESCRIPTION
    Start는 격리 앱을 실행하며 UI 조작은 하지 않는다. 화면/OS 대화상자는 지원되는
    Computer Use 도구로 별도 확인한다. Assert는 조회 호출·원본 불변을 검증한다.
    앱의 거부 안내·미연결 상태는 실제 화면 증거와 함께 판정해야 한다.
    Stop은 이 세션의 검증 PID만 종료한다. 로그와 fixture는 증거로 보존한다.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('Prepare', 'Start', 'Assert', 'Stop')][string]$Action,
    [ValidateSet('Running', 'QueryFailure', 'Locked', 'Picker')][string]$Scenario = 'Running',
    [string]$SessionPath,
    [string]$BinaryPath
)
$ErrorActionPreference = 'Stop'
$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$evidenceRoot = Join-Path $repoRoot 'target\visual-validation'

function Assert-PhysicalPath {
    param([string]$Path)
    $current = [IO.Path]::GetFullPath($Path)
    while ($current) {
        if (Test-Path -LiteralPath $current) {
            if ((Get-Item -LiteralPath $current -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw "Reparse point is not allowed in an isolated evidence path: $current"
            }
        }
        $current = [IO.Path]::GetDirectoryName($current)
    }
}

if ($Action -in @('Prepare', 'Start')) {
    if (-not $BinaryPath) { $BinaryPath = Join-Path $repoRoot 'target\release\gpui-convenience-tools.exe' }
    $BinaryPath = (Resolve-Path -LiteralPath $BinaryPath).Path
    Assert-PhysicalPath $evidenceRoot
    Assert-PhysicalPath $BinaryPath
    $sessionRoot = Join-Path $evidenceRoot ('o5-' + $Scenario.ToLowerInvariant() + '-' + [Guid]::NewGuid().ToString('N'))
    $dataRoot = Join-Path $sessionRoot 'appdata'
    New-Item -ItemType Directory -Path $dataRoot | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $sessionRoot 'source') | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $sessionRoot 'target') | Out-Null
    $vdi = Join-Path $sessionRoot 'source\fixture.vdi'
    # 실제 머신 이미지를 받지 않고 동봉 fixture로만 새 합성 VDI를 생성한다.
    $previousExportPath = $env:GPUI_CONVENIENCE_TOOLS_EXPORT_VDI_FIXTURE
    try {
        $env:GPUI_CONVENIENCE_TOOLS_EXPORT_VDI_FIXTURE = $vdi
        Push-Location -LiteralPath $repoRoot
        try {
            & cargo test -p gpui-convenience-tools exports_ntfs_vdi_fixture_when_requested --locked
            if ($LASTEXITCODE -ne 0) { throw 'Could not export bundled synthetic VDI fixture.' }
        }
        finally { Pop-Location }
    }
    finally {
        if ($null -eq $previousExportPath) { Remove-Item Env:\GPUI_CONVENIENCE_TOOLS_EXPORT_VDI_FIXTURE -ErrorAction SilentlyContinue }
        else { $env:GPUI_CONVENIENCE_TOOLS_EXPORT_VDI_FIXTURE = $previousExportPath }
    }
    $probe = Join-Path $sessionRoot 'vboxmanage-probe.exe'
    & rustc --edition=2021 -D warnings (Join-Path $PSScriptRoot 'testdata\vboxmanage-safety-probe.rs') -o $probe
    if ($LASTEXITCODE -ne 0) { throw 'Could not compile isolated VBoxManage probe.' }
    if ($Scenario -eq 'Locked') { New-Item -ItemType Directory -Path ($vdi + '.lck') | Out-Null }
    $tracePath = Join-Path $sessionRoot 'probe-calls.log'
    # 사용자 프로필, 광고 대상, 자동 동기화에 영향을 주지 않는 최소 설정.
    @{ service_enabled = $false; targets = @(); sync_enabled = $false; theme_mode = 'dark' } |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $dataRoot 'settings.json') -Encoding utf8
    $start = [Diagnostics.ProcessStartInfo]::new($BinaryPath)
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.Environment['GPUI_CONVENIENCE_TOOLS_DATA_DIR'] = $dataRoot
    $start.Environment['GPUI_CONVENIENCE_TOOLS_INITIAL_PANEL'] = 'virtual_disk'
    $start.Environment['GPUI_CONVENIENCE_TOOLS_VBOXMANAGE'] = $probe
    $start.Environment['GPUI_O5_PROBE_MODE'] = $Scenario
    $start.Environment['GPUI_O5_PROBE_TRACE'] = $tracePath
    $start.Environment['GPUI_O5_PROBE_VDI'] = $vdi
    # 자동 복사 등의 이전 검증 환경변수는 상속하지 않는다.
    @($start.Environment.Keys) | Where-Object { $_ -like 'GPUI_CONVENIENCE_TOOLS_VALIDATION_VDI_*' } |
        ForEach-Object { $start.Environment.Remove($_) | Out-Null }
    if ($Scenario -ne 'Picker') {
        $start.Environment['GPUI_CONVENIENCE_TOOLS_VALIDATION_VDI_PATH'] = $vdi
        $start.Environment['GPUI_CONVENIENCE_TOOLS_VALIDATION_VDI_TARGET'] = Join-Path $sessionRoot 'target'
    }
    $session = [ordered]@{
        scenario = $Scenario; sessionRoot = $sessionRoot; dataRoot = $dataRoot
        binaryPath = $BinaryPath; binaryHash = (Get-FileHash -LiteralPath $BinaryPath).Hash
        fixtureOrigin = 'bundled synthetic NTFS export'
        vdiPath = $vdi; vdiHash = (Get-FileHash -LiteralPath $vdi).Hash
        vdiLength = (Get-Item -LiteralPath $vdi).Length
        vdiModified = (Get-Item -LiteralPath $vdi).LastWriteTimeUtc.Ticks
        tracePath = $tracePath
    }
    if ($Action -eq 'Prepare') {
        @{ sessionRoot = $sessionRoot; vdiPath = $vdi; probePath = $probe; fixtureOrigin = $session.fixtureOrigin } | ConvertTo-Json
        return
    }
    $SessionPath = Join-Path $sessionRoot 'session.json'
    $process = [Diagnostics.Process]::Start($start)
    try {
        $null = $process.Handle
        $session['processId'] = $process.Id
        $session['processStarted'] = $process.StartTime.ToUniversalTime().Ticks
        $session | ConvertTo-Json | Set-Content -LiteralPath $SessionPath -Encoding utf8
    }
    catch {
        if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit(5000) | Out-Null }
        throw
    }
    @{ sessionPath = $SessionPath; processId = $process.Id; vdiPath = $vdi; scenario = $Scenario } | ConvertTo-Json
    return
}

$SessionPath = (Resolve-Path -LiteralPath $SessionPath).Path
$session = Get-Content -Raw -LiteralPath $SessionPath | ConvertFrom-Json
$expectedRoot = [IO.Path]::GetFullPath($evidenceRoot).TrimEnd('\') + '\'
$resolvedRoot = [IO.Path]::GetFullPath($session.sessionRoot)
Assert-PhysicalPath $resolvedRoot
if (-not $resolvedRoot.StartsWith($expectedRoot, [StringComparison]::OrdinalIgnoreCase) -or
    -not ([IO.Path]::GetFileName($resolvedRoot)).StartsWith('o5-') -or
    $SessionPath -ne (Join-Path $resolvedRoot 'session.json')) { throw 'Session is not an O-5 isolated evidence root.' }
$process = Get-Process -Id $session.processId -ErrorAction SilentlyContinue
if ($process) { $null = $process.Handle }
if ($process -and ($process.Path -ne $session.binaryPath -or
    $process.StartTime.ToUniversalTime().Ticks -ne $session.processStarted)) { throw 'Recorded PID now belongs to a different process.' }

if ($Action -eq 'Stop') {
    if ($process) {
        $process.Kill()
        if (-not $process.WaitForExit(5000)) { throw 'Verification process is still running.' }
    }
    "O5_SESSION_STOPPED pid=$($session.processId) evidence=$resolvedRoot"
    return
}
if (-not $process) { throw 'Verification app is not running.' }
$disk = Get-Item -LiteralPath $session.vdiPath
Assert-PhysicalPath $session.vdiPath
if ((Get-FileHash -LiteralPath $session.vdiPath).Hash -ne $session.vdiHash -or
    $disk.Length -ne $session.vdiLength -or $disk.LastWriteTimeUtc.Ticks -ne $session.vdiModified) { throw 'Source changed.' }
$trace = @(if (Test-Path -LiteralPath $session.tracePath) { Get-Content -LiteralPath $session.tracePath })
if ($session.scenario -eq 'Running' -and (@($trace).Count -ne 2 -or
    $trace[0] -ne 'list runningvms' -or $trace[1] -ne 'showvminfo 00000000-0000-0000-0000-000000000005 --machinereadable')) {
    throw 'Running VM probe did not traverse the expected process boundary.'
}
if ($session.scenario -eq 'QueryFailure' -and (@($trace).Count -ne 1 -or $trace[0] -ne 'list runningvms')) { throw 'Failed-query trace mismatch.' }
if ($session.scenario -eq 'Locked' -and @($trace).Count -ne 0) { throw 'Lock guard should reject before querying VM metadata.' }
if (@(Get-ChildItem -LiteralPath (Join-Path $resolvedRoot 'target') -Force).Count -ne 0) { throw 'Unexpected copy output.' }
"O5_PROBE_ASSERT_PASSED scenario=$($session.scenario) sourceUnchanged=true pid=$($session.processId); app UI acceptance must be observed separately"
