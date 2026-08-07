[CmdletBinding()]
param(
    [string]$Version,
    [string]$InstallDir,
    [string]$Target,
    [string]$ReleaseToken
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
$releaseApiBase = $env:SKILLTAPE_RELEASE_API_BASE_URL
if (-not [string]::IsNullOrWhiteSpace($releaseApiBase) -and
    -not $releaseApiBase.StartsWith("https://", [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "SKILLTAPE_RELEASE_API_BASE_URL must use HTTPS."
}
if ([string]::IsNullOrWhiteSpace($ReleaseToken)) {
    $ReleaseToken = $env:SKILLTAPE_RELEASE_TOKEN
}

$downloadHeaders = @{}
if (-not [string]::IsNullOrWhiteSpace($ReleaseToken)) {
    $downloadHeaders["Authorization"] = "Bearer $ReleaseToken"
}

$Version = $Version.TrimStart('v')
$asset = "skilltape-v$Version-$Target.zip"
$releaseRoot = "$($releaseBase.TrimEnd('/'))/v$Version"
$archiveUrl = "$releaseRoot/$asset"
$checksumsUrl = "$releaseRoot/checksums.txt"
if (-not [string]::IsNullOrWhiteSpace($releaseApiBase)) {
    $apiHeaders = @{
        "Accept" = "application/vnd.github+json"
        "User-Agent" = "skilltape-installer"
        "X-GitHub-Api-Version" = "2022-11-28"
    }
    if (-not [string]::IsNullOrWhiteSpace($ReleaseToken)) {
        $apiHeaders["Authorization"] = "Bearer $ReleaseToken"
    }
    $releaseMetadataUrl = "$($releaseApiBase.TrimEnd('/'))/releases/tags/v$Version"
    $releaseMetadata = Invoke-RestMethod -Uri $releaseMetadataUrl -Headers $apiHeaders -Method Get
    $archiveAsset = @($releaseMetadata.assets) |
        Where-Object { $_.name -eq $asset } |
        Select-Object -First 1
    $checksumsAsset = @($releaseMetadata.assets) |
        Where-Object { $_.name -eq "checksums.txt" } |
        Select-Object -First 1
    if ($null -eq $archiveAsset -or $null -eq $checksumsAsset) {
        throw "GitHub release v$Version does not contain the required Windows assets."
    }
    $archiveUrl = $archiveAsset.url
    $checksumsUrl = $checksumsAsset.url
    $downloadHeaders["Accept"] = "application/octet-stream"
    $downloadHeaders["User-Agent"] = "skilltape-installer"
    $downloadHeaders["X-GitHub-Api-Version"] = "2022-11-28"
}
$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("skilltape-install-" + [guid]::NewGuid().ToString("N"))
$stagedCli = $null
$stagedApi = $null
$stagedConsole = $null

function Assert-RegularFile([string]$Path, [string]$Label) {
    $item = Get-Item -LiteralPath $Path -ErrorAction Stop
    if ($item.PSIsContainer -or (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0)) {
        throw "$Label is not a regular file: $Path"
    }
}

function Assert-RegularDirectory([string]$Path, [string]$Label) {
    $item = Get-Item -LiteralPath $Path -ErrorAction Stop
    if (-not $item.PSIsContainer -or (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0)) {
        throw "$Label is not a regular directory: $Path"
    }
}

function Replace-File([string]$Staged, [string]$Destination) {
    if (Test-Path -LiteralPath $Destination) {
        [System.IO.File]::Replace($Staged, $Destination, $null)
    }
    else {
        [System.IO.File]::Move($Staged, $Destination)
    }
}

try {
    New-Item -ItemType Directory -Path $tempRoot -Force | Out-Null
    $archive = Join-Path $tempRoot $asset
    $checksums = Join-Path $tempRoot "checksums.txt"
    $archiveRequest = @{
        Uri = $archiveUrl
        OutFile = $archive
        UseBasicParsing = $true
    }
    $checksumsRequest = @{
        Uri = $checksumsUrl
        OutFile = $checksums
        UseBasicParsing = $true
    }
    if ($downloadHeaders.Count -gt 0) {
        $archiveRequest.Headers = $downloadHeaders
        $checksumsRequest.Headers = $downloadHeaders
    }
    Invoke-WebRequest @archiveRequest
    Invoke-WebRequest @checksumsRequest

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
    Assert-RegularFile $candidate.FullName "skilltape binary"
    $candidateApi = Get-ChildItem -LiteralPath $extractRoot -Recurse -File -Filter "skilltape-console-api.exe" |
        Where-Object { ($_.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -eq 0 } |
        Select-Object -First 1
    if ($null -eq $candidateApi) {
        throw "Release archive does not contain skilltape-console-api.exe."
    }
    Assert-RegularFile $candidateApi.FullName "Console API binary"
    $consoleIndex = Get-ChildItem -LiteralPath $extractRoot -Recurse -File -Filter "index.html" |
        Where-Object {
            $_.FullName -match '[\\/]console[\\/]index\.html$' -and
            ($_.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -eq 0
        } |
        Select-Object -First 1
    if ($null -eq $consoleIndex) {
        throw "Release archive does not contain console/index.html."
    }
    $consoleSource = $consoleIndex.Directory.FullName
    Assert-RegularDirectory $consoleSource "Console UI directory"
    $unsafeUiEntry = Get-ChildItem -LiteralPath $consoleSource -Recurse -Force |
        Where-Object { ($_.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0 } |
        Select-Object -First 1
    if ($null -ne $unsafeUiEntry) {
        throw "Release archive contains a symlink in the Console UI."
    }
    $consoleAssets = Join-Path $consoleSource "assets"
    Assert-RegularDirectory $consoleAssets "Console UI assets directory"
    $assetFile = Get-ChildItem -LiteralPath $consoleAssets -Recurse -File |
        Where-Object { ($_.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -eq 0 } |
        Select-Object -First 1
    if ($null -eq $assetFile) {
        throw "Release archive does not contain regular Console UI assets."
    }

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Assert-RegularDirectory $InstallDir "install directory"
    $installParent = Split-Path -Parent $InstallDir
    New-Item -ItemType Directory -Path $installParent -Force | Out-Null
    $stagedCli = Join-Path $InstallDir (".skilltape.tmp." + [guid]::NewGuid().ToString("N") + ".exe")
    $stagedApi = Join-Path $InstallDir (".skilltape-console-api.tmp." + [guid]::NewGuid().ToString("N") + ".exe")
    $stagedConsole = Join-Path $installParent (".skilltape-console.tmp." + [guid]::NewGuid().ToString("N"))
    Copy-Item -LiteralPath $candidate.FullName -Destination $stagedCli
    Copy-Item -LiteralPath $candidateApi.FullName -Destination $stagedApi
    New-Item -ItemType Directory -Path $stagedConsole -Force | Out-Null
    Get-ChildItem -LiteralPath $consoleSource -Force | Copy-Item -Destination $stagedConsole -Recurse -Force
    Assert-RegularFile $stagedCli "staged skilltape binary"
    Assert-RegularFile $stagedApi "staged Console API binary"
    Assert-RegularFile (Join-Path $stagedConsole "index.html") "staged Console UI index"

    $destination = Join-Path $InstallDir "skilltape.exe"
    Replace-File $stagedCli $destination
    $stagedCli = $null
    $apiDestination = Join-Path $InstallDir "skilltape-console-api.exe"
    Replace-File $stagedApi $apiDestination
    $stagedApi = $null
    $consoleDestination = Join-Path $installParent "console"
    $previousConsole = Join-Path $tempRoot "previous-console"
    if (Test-Path -LiteralPath $consoleDestination) {
        Move-Item -LiteralPath $consoleDestination -Destination $previousConsole
    }
    try {
        Move-Item -LiteralPath $stagedConsole -Destination $consoleDestination
        $stagedConsole = $null
    }
    catch {
        if (Test-Path -LiteralPath $previousConsole) {
            Move-Item -LiteralPath $previousConsole -Destination $consoleDestination
        }
        throw
    }

    Write-Output "Installed skilltape $Version for $Target at $(Join-Path $InstallDir 'skilltape.exe')"
}
finally {
    if ($null -ne $stagedCli -and (Test-Path -LiteralPath $stagedCli)) {
        Remove-Item -LiteralPath $stagedCli -Force -ErrorAction SilentlyContinue
    }
    if ($null -ne $stagedApi -and (Test-Path -LiteralPath $stagedApi)) {
        Remove-Item -LiteralPath $stagedApi -Force -ErrorAction SilentlyContinue
    }
    if ($null -ne $stagedConsole -and (Test-Path -LiteralPath $stagedConsole)) {
        Remove-Item -LiteralPath $stagedConsole -Recurse -Force -ErrorAction SilentlyContinue
    }
    if (Test-Path -LiteralPath $tempRoot) {
        Remove-Item -LiteralPath $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
