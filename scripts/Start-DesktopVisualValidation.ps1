[CmdletBinding()]
param(
    [string]$ManifestPath
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version 2.0

$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
if ([string]::IsNullOrWhiteSpace($ManifestPath)) {
    $ManifestPath = Join-Path $repoRoot "target\visual-validation\handoff.json"
}

$resolvedManifest = (Resolve-Path -LiteralPath $ManifestPath).Path
$manifest = Get-Content -Raw -LiteralPath $resolvedManifest | ConvertFrom-Json
if ($manifest.state -ne "DESKTOP_PENDING") {
    throw "The handoff manifest is not in DESKTOP_PENDING state."
}

$binaryPath = [System.IO.Path]::GetFullPath([string]$manifest.build.path)
if (-not (Test-Path -LiteralPath $binaryPath -PathType Leaf)) {
    throw "Validation binary does not exist: $binaryPath"
}

$actualHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $binaryPath).Hash.ToLowerInvariant()
$expectedHash = ([string]$manifest.build.sha256).ToLowerInvariant()
if ($actualHash -ne $expectedHash) {
    throw "Validation binary hash does not match the handoff manifest."
}

$lastSessionPath = Join-Path $repoRoot "target\visual-validation\last-session.json"
if (Test-Path -LiteralPath $lastSessionPath) {
    $lastSession = Get-Content -Raw -LiteralPath $lastSessionPath | ConvertFrom-Json
    if ($lastSession.state -eq "DESKTOP_READY") {
        throw ("A desktop validation session is already recorded. Run " +
            "scripts\Stop-DesktopVisualValidation.ps1 before starting another session.")
    }
    if ($lastSession.state -ne "CLEANED") {
        throw "The previous desktop validation session has an unknown state."
    }
}

$sessionId = [Guid]::NewGuid().ToString("N")
$sessionRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("gpui-visual-validation-" + $sessionId)
$appDataPath = Join-Path $sessionRoot "appdata"
$sourcePath = Join-Path $sessionRoot "source"
$targetPath = Join-Path $sessionRoot "target"
$pendingPublishPath = $lastSessionPath + ".pending-" + $sessionId
$process = $null

try {
    New-Item -ItemType Directory -Path $appDataPath, $sourcePath, $targetPath -Force | Out-Null

    $standardOutputPath = Join-Path $sessionRoot "process.stdout.log"
    $standardErrorPath = Join-Path $sessionRoot "process.stderr.log"
    $previousAppData = $env:APPDATA
    try {
        $env:APPDATA = $appDataPath
        $process = Start-Process `
            -FilePath $binaryPath `
            -WorkingDirectory $repoRoot `
            -RedirectStandardOutput $standardOutputPath `
            -RedirectStandardError $standardErrorPath `
            -PassThru
    }
    finally {
        if ($null -eq $previousAppData) {
            Remove-Item Env:\APPDATA -ErrorAction SilentlyContinue
        }
        else {
            $env:APPDATA = $previousAppData
        }
    }

    if ($null -eq $process) {
        throw "The validation process did not start."
    }

    $session = [ordered]@{
        state = "DESKTOP_READY"
        processId = $process.Id
        processStartTimeUtc = $process.StartTime.ToUniversalTime().ToString("o")
        binaryPath = $binaryPath
        binarySha256 = $actualHash
        manifestPath = $resolvedManifest
        sessionRoot = $sessionRoot
        appData = $appDataPath
        source = $sourcePath
        target = $targetPath
        standardOutput = $standardOutputPath
        standardError = $standardErrorPath
        nextStep = "Use Computer Use from Windows ChatGPT desktop Work or Codex."
        sessionPath = (Join-Path $sessionRoot "session.json")
        lastSessionPath = $lastSessionPath
    }

    $sessionJson = $session | ConvertTo-Json -Depth 4
    $sessionJson | Set-Content -LiteralPath $session.sessionPath -Encoding UTF8
    $sessionJson | Set-Content -LiteralPath $pendingPublishPath -Encoding UTF8
    Move-Item -LiteralPath $pendingPublishPath -Destination $lastSessionPath -Force
    Write-Output $sessionJson
}
catch {
    if ($null -ne $process -and -not $process.HasExited) {
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
        $process.WaitForExit()
    }
    if (Test-Path -LiteralPath $pendingPublishPath) {
        Remove-Item -LiteralPath $pendingPublishPath -Force
    }
    if (Test-Path -LiteralPath $sessionRoot) {
        Remove-Item -LiteralPath $sessionRoot -Recurse -Force
    }
    throw
}
