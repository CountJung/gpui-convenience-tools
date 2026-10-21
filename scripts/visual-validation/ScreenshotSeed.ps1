# 합성 PNG를 현재 검증 세션의 격리 data_dir에만 복사한다. 앱/화면/클립보드에는 접근하지 않는다.
function Copy-ValidationScreenshotSeed {
    param([string]$SeedPath, [string]$DataDir)
    if ([string]::IsNullOrWhiteSpace($SeedPath)) { return $null }
    $source = [System.IO.Path]::GetFullPath($SeedPath)
    if ([System.IO.Path]::GetExtension($source) -ine '.png') { throw '합성 시드는 PNG 파일이어야 한다.' }
    $root = (Resolve-Path -LiteralPath $DataDir -ErrorAction Stop).ProviderPath
    if (-not (Test-Path -LiteralPath $root -PathType Container)) { throw '격리 데이터 폴더가 필요하다.' }
    $destination = Join-Path $root 'validation-screenshot.png'
    $inputFile = $null
    $outputFile = $null
    $created = $false
    $complete = $false
    try {
        $inputFile = [System.IO.File]::Open($source, 'Open', 'Read', 'Read')
        if ($inputFile.Length -lt 33 -or $inputFile.Length -gt 16MB) { throw 'PNG 시드 크기 한도: 33B~16MiB.' }
        $header = New-Object byte[] 24
        if ($inputFile.Read($header, 0, 24) -ne 24) { throw 'PNG 헤더 읽기 실패.' }
        if ([BitConverter]::ToString($header, 0, 16) -ne '89-50-4E-47-0D-0A-1A-0A-00-00-00-0D-49-48-44-52') {
            throw 'PNG 서명/IHDR이 잘못되었다.'
        }
        $pngWidth = [uint64]$header[16] * 16777216 + [uint64]$header[17] * 65536 + [uint64]$header[18] * 256 + $header[19]
        $pngHeight = [uint64]$header[20] * 16777216 + [uint64]$header[21] * 65536 + [uint64]$header[22] * 256 + $header[23]
        if ($pngWidth -lt 1 -or $pngHeight -lt 1 -or $pngWidth -gt 4096 -or $pngHeight -gt 4096 -or $pngWidth * $pngHeight -gt 8MB) {
            throw 'PNG 시드 한도: 4096px/변, 총 8M픽셀.'
        }
        $inputFile.Position = 0
        # 신규 세션 파일만 만들며 기존 파일을 덮어쓰지 않는다. 입력은 write 공유를 허용하지 않는다.
        $outputFile = [System.IO.File]::Open($destination, 'CreateNew', 'Write', 'None')
        $created = $true
        $inputFile.CopyTo($outputFile)
        $outputFile.Flush()
        $complete = $true
        return $destination
    }
    finally {
        if ($null -ne $outputFile) { $outputFile.Dispose() }
        if ($null -ne $inputFile) { $inputFile.Dispose() }
        if ($created -and -not $complete) { Remove-Item -LiteralPath $destination -ErrorAction SilentlyContinue }
    }
}
