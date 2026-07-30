[CmdletBinding()]
param(
    [string]$SessionPath
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version 2.0

$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
if ([string]::IsNullOrWhiteSpace($SessionPath)) {
    $SessionPath = Join-Path $repoRoot "target\visual-validation\last-session.json"
}

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

$resolvedSession = (Resolve-Path -LiteralPath $SessionPath).Path
$session = Get-Content -Raw -LiteralPath $resolvedSession | ConvertFrom-Json
$sessionRoot = [System.IO.Path]::GetFullPath([string]$session.sessionRoot)
$tempPrefix = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath()).TrimEnd("\") + "\"
$sessionName = [System.IO.Path]::GetFileName($sessionRoot)

if (-not $sessionRoot.StartsWith($tempPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to clean a session outside the Windows temporary directory: $sessionRoot"
}
if (-not $sessionName.StartsWith("gpui-visual-validation-", [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to clean an unexpected session directory: $sessionRoot"
}

$process = Get-Process -Id ([int]$session.processId) -ErrorAction SilentlyContinue
if ($null -ne $process) {
    $expectedBinary = [System.IO.Path]::GetFullPath([string]$session.binaryPath)
    $actualBinary = [System.IO.Path]::GetFullPath([string]$process.Path)
    if (-not $actualBinary.Equals($expectedBinary, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "PID belongs to a different executable; refusing to stop it."
    }
    $expectedStartTime = ConvertTo-UtcInstant $session.processStartTimeUtc
    $actualStartTime = $process.StartTime.ToUniversalTime()
    if ([Math]::Abs(($actualStartTime - $expectedStartTime).TotalSeconds) -gt 1.0) {
        throw "PID start time does not match the recorded validation process."
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
    processRunning = [bool](Get-Process -Id ([int]$session.processId) -ErrorAction SilentlyContinue)
    sessionRootExists = (Test-Path -LiteralPath $sessionRoot)
    cleanedAtUtc = [DateTime]::UtcNow.ToString("o")
}

$resultJson = $result | ConvertTo-Json -Depth 3
$resultJson | Set-Content -LiteralPath (Join-Path $repoRoot "target\visual-validation\last-session.json") -Encoding UTF8
Write-Output $resultJson
