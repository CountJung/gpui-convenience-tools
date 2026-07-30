[CmdletBinding()]
param(
    [switch]$PrepareDesktopHandoff
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version 2.0

$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$packageName = "gpui-convenience-tools"
$visualTests = @(
    "ad_block_split_fills_default_and_minimum_supported_width",
    "split_panels_keep_settings_pane_usable_at_default_width",
    "sidebar_divider_drag_resizes_navigation_and_content",
    "sidebar_wheel_scroll_reaches_last_navigation_item",
    "rendered_switch_toggles_in_light_dark_and_missing_switch_token_theme",
    "file_sync_unified_page_uses_full_width_and_scrolls_to_last_record",
    "file_sync_run_button_saves_current_inputs_and_queues_selected_job",
    "file_sync_status_bar_stays_visible_at_compact_height_while_running",
    "file_sync_stop_button_requests_cancellation_and_clears_pending_queue",
    "sync_failures_log_one_summary_line_instead_of_one_entry_per_file",
    "background_sync_event_wakes_render_without_additional_user_input"
)

function Invoke-CheckedCommand {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Executable,

        [Parameter(Mandatory = $true)]
        [string[]]$Arguments
    )

    Write-Host ""
    Write-Host ("> {0} {1}" -f $Executable, ($Arguments -join " "))
    & $Executable @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw ("Command failed with exit code {0}: {1} {2}" -f
            $LASTEXITCODE, $Executable, ($Arguments -join " "))
    }
}

Push-Location $repoRoot
try {
    Write-Host "Surface policy: IDE-safe verification only."
    Write-Host "Computer Use is not initialized or retried by this script."

    Invoke-CheckedCommand -Executable "cargo" -Arguments @(
        "check", "-p", $packageName
    )

    Write-Host ""
    Write-Host ("> cargo test -p {0} -- --list" -f $packageName)
    $testListOutput = @(& cargo test -p $packageName -- --list)
    if ($LASTEXITCODE -ne 0) {
        throw "Unable to enumerate the package test list."
    }

    foreach ($testName in $visualTests) {
        $fullTestName = "app::tests::$testName"
        $testMatches = @($testListOutput | Where-Object {
            $_ -match ("^{0}: test$" -f [Regex]::Escape($fullTestName))
        })
        if ($testMatches.Count -ne 1) {
            throw ("Expected exactly one listed GPUI test named {0}; found {1}." -f
                $fullTestName, $testMatches.Count)
        }
    }
    Write-Host ("Validated {0} required GPUI test names." -f $visualTests.Count)

    foreach ($testName in $visualTests) {
        Invoke-CheckedCommand -Executable "cargo" -Arguments @(
            "test", "-p", $packageName, $testName, "--", "--nocapture"
        )
    }

    Invoke-CheckedCommand -Executable "cargo" -Arguments @(
        "test", "--all-targets", "--all-features"
    )

    if (-not $PrepareDesktopHandoff) {
        Write-Host ""
        Write-Host "IDE_VERIFIED"
        Write-Host "DESKTOP_PENDING"
        Write-Host "Real-screen verification: on Windows Claude Code (CLAUDE_LOCAL) run"
        Write-Host "  scripts\Invoke-ClaudeVisualCheck.ps1 -Action Start"
        Write-Host "Otherwise run the VS Code task 'GPUI: Prepare ChatGPT desktop handoff'."
        exit 0
    }

    Invoke-CheckedCommand -Executable "cargo" -Arguments @(
        "build", "-p", $packageName, "--release"
    )

    $releaseBinary = Join-Path $repoRoot "target\release\gpui-convenience-tools.exe"
    if (-not (Test-Path -LiteralPath $releaseBinary -PathType Leaf)) {
        throw "Release binary was not produced: $releaseBinary"
    }

    $commit = (& git rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0) {
        throw "Unable to read the current Git commit."
    }

    $statusLines = @(& git status --porcelain=v1 --untracked-files=all)
    if ($LASTEXITCODE -ne 0) {
        throw "Unable to read the working tree status."
    }

    $binaryHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $releaseBinary).Hash.ToLowerInvariant()
    $artifactDirectory = Join-Path $repoRoot ("target\visual-validation\builds\" + $binaryHash)
    New-Item -ItemType Directory -Path $artifactDirectory -Force | Out-Null

    $artifactBinary = Join-Path $artifactDirectory "gpui-convenience-tools.exe"
    Copy-Item -LiteralPath $releaseBinary -Destination $artifactBinary -Force

    $artifactHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $artifactBinary).Hash.ToLowerInvariant()
    if ($artifactHash -ne $binaryHash) {
        throw "Copied validation binary hash does not match the release binary."
    }

    $manifestDirectory = Join-Path $repoRoot "target\visual-validation"
    $manifestPath = Join-Path $manifestDirectory "handoff.json"
    $artifactItem = Get-Item -LiteralPath $artifactBinary
    $changedPaths = @($statusLines | ForEach-Object {
        if ($_.Length -gt 3) { $_.Substring(3) } else { $_ }
    })

    $manifest = [ordered]@{
        schemaVersion = 1
        state = "DESKTOP_PENDING"
        preparedAtUtc = [DateTime]::UtcNow.ToString("o")
        source = [ordered]@{
            commit = $commit
            workingTreeDirty = ($statusLines.Count -gt 0)
            changedPaths = $changedPaths
        }
        ideVerification = [ordered]@{
            state = "IDE_VERIFIED"
            computerUseAttempted = $false
            visualTests = $visualTests
            fullCommand = "cargo test --all-targets --all-features"
        }
        build = [ordered]@{
            path = $artifactItem.FullName
            sha256 = $artifactHash
            sizeBytes = $artifactItem.Length
            lastWriteTimeUtc = $artifactItem.LastWriteTimeUtc.ToString("o")
        }
        desktop = [ordered]@{
            requiredSurface = "Windows ChatGPT desktop Work or Codex"
            startScript = (Join-Path $repoRoot "scripts\Start-DesktopVisualValidation.ps1")
            stopScript = (Join-Path $repoRoot "scripts\Stop-DesktopVisualValidation.ps1")
            lastSession = (Join-Path $repoRoot "target\visual-validation\last-session.json")
            policy = (Join-Path $repoRoot ".github\copilot-instructions.md")
            state = "DESKTOP_PENDING"
        }
    }

    $manifest | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $manifestPath -Encoding UTF8

    Write-Host ""
    Write-Host "IDE_VERIFIED"
    Write-Host "DESKTOP_PENDING"
    Write-Host ("Manifest: {0}" -f $manifestPath)
    Write-Host ("Binary:   {0}" -f $artifactBinary)
    Write-Host ("SHA256:   {0}" -f $artifactHash)
}
finally {
    Pop-Location
}
