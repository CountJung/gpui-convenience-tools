[CmdletBinding()]
param(
    [int]$WarningThreshold = 800,
    [int]$ViolationThreshold = 1000
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version 2.0

if ($WarningThreshold -lt 1 -or $ViolationThreshold -le $WarningThreshold) {
    throw "WarningThreshold must be positive and lower than ViolationThreshold."
}

$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$sourceRoot = Join-Path $repoRoot "app\src"
$projectMapPath = Join-Path $repoRoot "docs\PROJECT_MAP.md"

if (-not (Test-Path -LiteralPath $sourceRoot -PathType Container)) {
    throw "Source root was not found: $sourceRoot"
}
if (-not (Test-Path -LiteralPath $projectMapPath -PathType Leaf)) {
    throw "Project map was not found: $projectMapPath"
}

$records = @(
    Get-ChildItem -LiteralPath $sourceRoot -Recurse -File -Filter "*.rs" |
        ForEach-Object {
            [PSCustomObject]@{
                File = $_.FullName
                RelativeFile = [System.IO.Path]::GetRelativePath($repoRoot, $_.FullName).Replace("\", "/")
                Lines = @(Get-Content -LiteralPath $_.FullName).Count
            }
        }
)

if ($records.Count -eq 0) {
    throw "No Rust source files were found under $sourceRoot"
}

$ordered = @($records | Sort-Object -Property @{Expression = "Lines"; Descending = $true}, "RelativeFile")
$totalLines = [int](($records | Measure-Object -Property Lines -Sum).Sum)
$maxRecord = $ordered[0]
$warnings = @($records | Where-Object {
        $_.Lines -ge $WarningThreshold -and $_.Lines -lt $ViolationThreshold
    })
$violations = @($records | Where-Object { $_.Lines -ge $ViolationThreshold })

if ($violations.Count -gt 0) {
    $details = $violations | ForEach-Object { "{0}={1}" -f $_.RelativeFile, $_.Lines }
    throw ("STRUCTURE_VIOLATION files at or above {0} lines: {1}" -f
        $ViolationThreshold, ($details -join ", "))
}

$projectMap = Get-Content -Raw -LiteralPath $projectMapPath
$measurementPattern = '\*\*최종 측정\*\*:.*?`app/src` 총 (\d+)개 파일 · ([\d,]+)줄'
$measurementMatches = [regex]::Matches($projectMap, $measurementPattern)
if ($measurementMatches.Count -ne 1) {
    throw "PROJECT_MAP must contain exactly one final app/src measurement."
}

$measurement = $measurementMatches[0]
$mappedFileCount = [int]$measurement.Groups[1].Value
$mappedLineCount = [int]($measurement.Groups[2].Value -replace ',', '')
if ($mappedFileCount -ne $records.Count -or $mappedLineCount -ne $totalLines) {
    throw ("PROJECT_MAP measurement is stale: mapped={0} files/{1} lines, actual={2} files/{3} lines" -f
        $mappedFileCount, $mappedLineCount, $records.Count, $totalLines)
}

$maxPattern = '최대 파일은 (\d+)줄\(`([^`]+)`\)'
$maxMatch = [regex]::Match($projectMap, $maxPattern)
if (-not $maxMatch.Success) {
    throw "PROJECT_MAP must contain the maximum-file measurement."
}

$mappedMaxLines = [int]$maxMatch.Groups[1].Value
$mappedMaxPath = $maxMatch.Groups[2].Value.Replace("\", "/")
$actualMaxPath = [System.IO.Path]::GetRelativePath($sourceRoot, $maxRecord.File).Replace("\", "/")
if ($mappedMaxLines -ne $maxRecord.Lines -or $mappedMaxPath -ne $actualMaxPath) {
    throw ("PROJECT_MAP maximum-file measurement is stale: mapped={0}={1}, actual={2}={3}" -f
        $mappedMaxPath, $mappedMaxLines, $actualMaxPath, $maxRecord.Lines)
}

$warningPattern = '🟡 경고 \*\*(\d+)개\*\*'
$warningMatch = [regex]::Match($projectMap, $warningPattern)
if (-not $warningMatch.Success) {
    throw "PROJECT_MAP must contain the warning-file count."
}

$mappedWarningCount = [int]$warningMatch.Groups[1].Value
if ($mappedWarningCount -ne $warnings.Count) {
    throw ("PROJECT_MAP warning count is stale: mapped={0}, actual={1}" -f
        $mappedWarningCount, $warnings.Count)
}

Write-Output ("STRUCTURE_VERIFIED files={0} lines={1} max={2} path={3} warnings={4}" -f
    $records.Count, $totalLines, $maxRecord.Lines, $maxRecord.RelativeFile, $warnings.Count)
