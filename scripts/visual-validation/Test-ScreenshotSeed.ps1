# 파일 전용 합성 fixture. 앱/프로세스/클립보드/화면/전역 입력을 사용하지 않는다.
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'ScreenshotSeed.ps1')
$testRoot = Join-Path ([System.IO.Path]::GetTempPath()) ('e004-png-seed-' + [Guid]::NewGuid().ToString('N'))
$dataRoot = Join-Path $testRoot 'data'
$source = Join-Path $testRoot 'synthetic.png'
$destination = Join-Path $dataRoot 'validation-screenshot.png'
New-Item -ItemType Directory -Path $dataRoot | Out-Null
try {
    # 자체 생성한 1x1 PNG. 개인 이미지/화면을 읽지 않는다.
    $png = [Convert]::FromBase64String('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a6gAAAABJRU5ErkJggg==')
    [System.IO.File]::WriteAllBytes($source, $png)
    if ($null -ne (Copy-ValidationScreenshotSeed -SeedPath '' -DataDir $dataRoot)) { throw 'empty seed must be disabled' }
    $copied = Copy-ValidationScreenshotSeed -SeedPath $source -DataDir $dataRoot
    if ($copied -ne $destination -or (Get-FileHash -LiteralPath $source).Hash -ne (Get-FileHash -LiteralPath $copied).Hash) {
        throw 'isolated seed copy mismatch'
    }
    $blocked = $false
    try { Copy-ValidationScreenshotSeed -SeedPath $source -DataDir $dataRoot | Out-Null }
    catch { $blocked = $true }
    if (-not $blocked) { throw 'existing destination must not be overwritten' }
    Remove-Item -LiteralPath $destination
    $badHeader = [byte[]]$png.Clone()
    $badHeader[16] = 255
    [System.IO.File]::WriteAllBytes($source, $badHeader)
    $blocked = $false
    try { Copy-ValidationScreenshotSeed -SeedPath $source -DataDir $dataRoot | Out-Null }
    catch { $blocked = $true }
    if (-not $blocked -or (Test-Path -LiteralPath $destination)) { throw 'oversized dimensions must fail before copy' }
    [System.IO.File]::WriteAllBytes($source, [byte[]](1, 2, 3))
    $blocked = $false
    try { Copy-ValidationScreenshotSeed -SeedPath $source -DataDir $dataRoot | Out-Null }
    catch { $blocked = $true }
    if (-not $blocked -or (Test-Path -LiteralPath $destination)) { throw 'invalid PNG must not be copied' }
    Write-Output 'SCREENSHOT_SEED_COPY_VERIFIED'
}
finally {
    # 신규 GUID 폴더에 직접 만든 정확한 파일/빈 디렉터리만 정리한다. 재귀 삭제하지 않는다.
    foreach ($fixtureFile in @($destination, $source)) {
        if (Test-Path -LiteralPath $fixtureFile) { Remove-Item -LiteralPath $fixtureFile }
    }
    Remove-Item -LiteralPath $dataRoot
    Remove-Item -LiteralPath $testRoot
}
