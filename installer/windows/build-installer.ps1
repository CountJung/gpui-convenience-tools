param(
    [string]$Configuration = "release"
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $root

$exePath = Join-Path $root "target\$Configuration\gpui-convenience-tools.exe"
if (-not (Test-Path $exePath)) {
    Write-Host "Building gpui-convenience-tools binary..."
    if ($Configuration -eq "release") {
        cargo build -p gpui-convenience-tools --release
    }
    else {
        cargo build -p gpui-convenience-tools
    }
}

$wxsPath = Join-Path $root "installer\windows\gpui-convenience-tools.wxs"
$wixObj = Join-Path $root "installer\windows\gpui-convenience-tools.wixobj"
$msiPath = Join-Path $root "installer\windows\gpui-convenience-tools.msi"

candle.exe $wxsPath -out $wixObj
light.exe $wixObj -ext WixUIExtension -out $msiPath

Write-Host "Installer generated: $msiPath"



