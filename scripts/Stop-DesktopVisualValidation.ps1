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
    $expectedStartTime = [DateTime]::Parse([string]$session.processStartTimeUtc).ToUniversalTime()
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
