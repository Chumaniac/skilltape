[CmdletBinding()]
param(
    [string]$Version,
    [string]$InstallDir,
    [string]$Target
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($Version)) {
    $Version = $env:SKILLTAPE_VERSION
}
if ([string]::IsNullOrWhiteSpace($Version)) {
    throw "Pass a release version or set SKILLTAPE_VERSION."
}
if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    $InstallDir = $env:SKILLTAPE_INSTALL_DIR
}
if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    $InstallDir = Join-Path $env:LOCALAPPDATA "SkillTape\bin"
}
if ([string]::IsNullOrWhiteSpace($Target)) {
    $Target = $env:SKILLTAPE_TARGET
}
if ([string]::IsNullOrWhiteSpace($Target)) {
    switch ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture) {
        "X64" { $Target = "x86_64-pc-windows-msvc" }
        "Arm64" { $Target = "aarch64-pc-windows-msvc" }
        default { throw "Set SKILLTAPE_TARGET for this Windows architecture." }
    }
}

$releaseBase = $env:SKILLTAPE_RELEASE_BASE_URL
if ([string]::IsNullOrWhiteSpace($releaseBase)) {
    throw "Set SKILLTAPE_RELEASE_BASE_URL to a release URL ending in /releases/download."
}
if (-not $releaseBase.StartsWith("https://", [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "SKILLTAPE_RELEASE_BASE_URL must use HTTPS."
}

$Version = $Version.TrimStart('v')
$asset = "skilltape-v$Version-$Target.zip"
$releaseRoot = "$($releaseBase.TrimEnd('/'))/v$Version"
$archiveUrl = "$releaseRoot/$asset"
$checksumsUrl = "$releaseRoot/checksums.txt"
$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("skilltape-install-" + [guid]::NewGuid().ToString("N"))
$staged = $null

try {
    New-Item -ItemType Directory -Path $tempRoot -Force | Out-Null
    $archive = Join-Path $tempRoot $asset
    $checksums = Join-Path $tempRoot "checksums.txt"
    Invoke-WebRequest -Uri $archiveUrl -OutFile $archive -UseBasicParsing
    Invoke-WebRequest -Uri $checksumsUrl -OutFile $checksums -UseBasicParsing

    $checksumLine = Get-Content $checksums | Where-Object {
        $parts = $_ -split '\s+'
        $parts.Length -ge 2 -and ($parts[1] -eq $asset -or $parts[1] -eq "*$asset")
    } | Select-Object -First 1
    if ([string]::IsNullOrWhiteSpace($checksumLine)) {
        throw "checksums.txt has no entry for $asset."
    }
    $expected = ($checksumLine -split '\s+')[0].ToLowerInvariant()
    if ($expected -notmatch '^[0-9a-f]{64}$') {
        throw "checksums.txt contains an invalid SHA-256 for $asset."
    }
    $actual = (Get-FileHash -Algorithm SHA256 -Path $archive).Hash.ToLowerInvariant()
    if ($actual -ne $expected) {
        throw "Checksum mismatch for $asset."
    }

    $extractRoot = Join-Path $tempRoot "extracted"
    Expand-Archive -LiteralPath $archive -DestinationPath $extractRoot -Force
    $candidate = Get-ChildItem -LiteralPath $extractRoot -Recurse -File -Filter "skilltape.exe" | Select-Object -First 1
    if ($null -eq $candidate) {
        throw "Release archive does not contain skilltape.exe."
    }

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    $staged = Join-Path $InstallDir (".skilltape.tmp." + [guid]::NewGuid().ToString("N") + ".exe")
    Copy-Item -LiteralPath $candidate.FullName -Destination $staged
    $destination = Join-Path $InstallDir "skilltape.exe"
    if (Test-Path -LiteralPath $destination) {
        [System.IO.File]::Replace($staged, $destination, $null)
    }
    else {
        [System.IO.File]::Move($staged, $destination)
    }
    $staged = $null

    Write-Output "Installed skilltape $Version for $Target at $(Join-Path $InstallDir 'skilltape.exe')"
}
finally {
    if ($null -ne $staged -and (Test-Path -LiteralPath $staged)) {
        Remove-Item -LiteralPath $staged -Force -ErrorAction SilentlyContinue
    }
    if (Test-Path -LiteralPath $tempRoot) {
        Remove-Item -LiteralPath $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
