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
$activeIds = @(Get-TaskIds -Lines $todoLines -Pattern '^\- \[ \] ((?:VDE|D|E|G|K)-\d{3}) \|')

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
$matrixIds = @(Get-TaskIds -Lines $matrixLines -Pattern '^\| ((?:VDE|D|E|G|K)-\d{3}) \|')
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

Write-Output ("DOCS_VERIFIED active={0} matrix={1}" -f $activeIds.Count, $matrixIds.Count)
