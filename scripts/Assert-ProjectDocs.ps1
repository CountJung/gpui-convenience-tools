[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
Set-StrictMode -Version 2.0

$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$todoPath = Join-Path $repoRoot "docs\TODO.md"
$verificationPath = Join-Path $repoRoot "docs\VERIFICATION.md"

function Get-TaskIds {
    param(
        [Parameter(Mandatory = $true)]
        [AllowEmptyCollection()]
        [AllowEmptyString()]
        [string[]]$Lines,

        [Parameter(Mandatory = $true)]
        [string]$Pattern
    )

    @($Lines | ForEach-Object {
        if ($_ -match $Pattern) {
            $Matches[1]
        }
    })
}

function Find-LineIndex {
    param(
        [Parameter(Mandatory = $true)]
        [AllowEmptyCollection()]
        [AllowEmptyString()]
        [string[]]$Lines,

        [Parameter(Mandatory = $true)]
        [string]$Value
    )

    for ($index = 0; $index -lt $Lines.Count; $index++) {
        if ($Lines[$index] -eq $Value) {
            return $index
        }
    }

    return -1
}

if (-not (Test-Path -LiteralPath $todoPath -PathType Leaf)) {
    throw "TODO document was not found: $todoPath"
}
if (-not (Test-Path -LiteralPath $verificationPath -PathType Leaf)) {
    throw "Verification document was not found: $verificationPath"
}

$todoLines = @(Get-Content -LiteralPath $todoPath)
$verificationLines = @(Get-Content -LiteralPath $verificationPath)
$taskIdPattern = '^\s*-\s*\[ \]\s+([A-Z][A-Z0-9]{0,7}-\d{1,4})\s+\|'
$matrixIdPattern = '^\|\s*([A-Z][A-Z0-9]{0,7}-\d{1,4})\s*\|'
$activeIds = @(Get-TaskIds -Lines $todoLines -Pattern $taskIdPattern)

if ($activeIds.Count -eq 0) {
    throw "No active TODO IDs were found."
}

$duplicateActiveIds = @($activeIds | Group-Object | Where-Object Count -gt 1)
if ($duplicateActiveIds.Count -gt 0) {
    throw ("Duplicate active TODO IDs: " + ($duplicateActiveIds.Name -join ", "))
}

$matrixStart = Find-LineIndex -Lines $verificationLines -Value "## 작업별 체크 매트릭스"
$matrixEnd = Find-LineIndex -Lines $verificationLines -Value "## 완료 검증 기록"
if ($matrixStart -lt 0 -or $matrixEnd -le $matrixStart) {
    throw "The active verification matrix boundaries were not found."
}

$matrixLines = @($verificationLines[($matrixStart + 1)..($matrixEnd - 1)])
$matrixIds = @(Get-TaskIds -Lines $matrixLines -Pattern $matrixIdPattern)
$duplicateMatrixIds = @($matrixIds | Group-Object | Where-Object Count -gt 1)
if ($duplicateMatrixIds.Count -gt 0) {
    throw ("Duplicate verification matrix IDs: " + ($duplicateMatrixIds.Name -join ", "))
}

$missingMatrixIds = @($activeIds | Where-Object { $_ -notin $matrixIds })
$orphanMatrixIds = @($matrixIds | Where-Object { $_ -notin $activeIds })
if ($missingMatrixIds.Count -gt 0) {
    throw ("Active TODO IDs missing from verification matrix: " + ($missingMatrixIds -join ", "))
}
if ($orphanMatrixIds.Count -gt 0) {
    throw ("Verification matrix IDs without an active TODO row: " + ($orphanMatrixIds -join ", "))
}

$decisionHeading = "## 판단·환경 확인이 필요한 보류 항목"
$decisionStart = Find-LineIndex -Lines $todoLines -Value $decisionHeading
if ($decisionStart -lt 0) {
    throw "The TODO decision/environment section was not found."
}

$decisionMarkerPattern = '^\s*<!--\s*decision-task-ids:\s*(.*?)\s*-->\s*$'
$decisionMarkerIndexes = @(
    0..($todoLines.Count - 1) |
        Where-Object { $todoLines[$_] -match $decisionMarkerPattern }
)
if ($decisionMarkerIndexes.Count -ne 1 -or $decisionMarkerIndexes[0] -le $decisionStart) {
    throw "TODO must contain exactly one decision-task-ids marker after the decision/environment section."
}

$decisionMarkerLine = $todoLines[$decisionMarkerIndexes[0]]
$null = $decisionMarkerLine -match $decisionMarkerPattern
$decisionIds = @($Matches[1] -split ',' | ForEach-Object { $_.Trim() } | Where-Object { $_ })
if ($decisionIds.Count -eq 0) {
    throw "TODO decision-task-ids marker is empty."
}

$invalidDecisionIds = @($decisionIds | Where-Object { $_ -notmatch '^[A-Z][A-Z0-9]{0,7}-\d{1,4}$' })
if ($invalidDecisionIds.Count -gt 0) {
    throw ("Invalid decision-task IDs: " + ($invalidDecisionIds -join ", "))
}

$duplicateDecisionIds = @($decisionIds | Group-Object | Where-Object Count -gt 1)
if ($duplicateDecisionIds.Count -gt 0) {
    throw ("Duplicate decision-task IDs: " + ($duplicateDecisionIds.Name -join ", "))
}

$orphanDecisionIds = @($decisionIds | Where-Object { $_ -notin $activeIds })
if ($orphanDecisionIds.Count -gt 0) {
    throw ("Decision/environment IDs without an active TODO row: " + ($orphanDecisionIds -join ", "))
}

Write-Output ("DOCS_VERIFIED active={0} matrix={1}" -f $activeIds.Count, $matrixIds.Count)
