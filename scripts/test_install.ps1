param([string]$InstallerPath = (Join-Path $PSScriptRoot "install.ps1"))
$ErrorActionPreference = "Stop"
$fixtureRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("skilltape-installer-fixture-" + [guid]::NewGuid().ToString("N"))
$previousBase = $env:SKILLTAPE_RELEASE_BASE_URL
$previousApi = $env:SKILLTAPE_RELEASE_API_BASE_URL
$previousToken = $env:SKILLTAPE_RELEASE_TOKEN
$global:SkillTapeInstallerFixture = @{}

function Assert-Fixture([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}
# PowerShell on Unix hides dot-prefixed staged files unless Force is explicit.
function Get-Item {
    [CmdletBinding()]
    param($LiteralPath)
    Microsoft.PowerShell.Management\Get-Item -LiteralPath $LiteralPath -Force
}
function Invoke-WebRequest {
    param($Uri, $OutFile, [switch]$UseBasicParsing, $Headers)
    if ($Uri.EndsWith("checksums.txt")) { Copy-Item -LiteralPath $global:SkillTapeInstallerFixture.checksumFixture -Destination $OutFile }
    else { Copy-Item -LiteralPath $global:SkillTapeInstallerFixture.archiveFixture -Destination $OutFile }
}
function Move-Item {
    param($LiteralPath, $Destination)
    if ($Destination -eq $global:SkillTapeInstallerFixture.failureDestination -and ($global:SkillTapeInstallerFixture.persistentFailure -or $global:SkillTapeInstallerFixture.failureCount -eq 0)) {
        $global:SkillTapeInstallerFixture.failureCount++
        throw "Synthetic final-move failure."
    }
    Microsoft.PowerShell.Management\Move-Item -LiteralPath $LiteralPath -Destination $Destination
}
function Remove-Item {
    [CmdletBinding()]
    param($LiteralPath, [switch]$Recurse, [switch]$Force)
    if ($LiteralPath -eq $global:SkillTapeInstallerFixture.cleanupFailureDestination) {
        throw "Synthetic lock-cleanup failure."
    }
    Microsoft.PowerShell.Management\Remove-Item -LiteralPath $LiteralPath -Recurse:$Recurse -Force:$Force -ErrorAction $ErrorActionPreference
}
function Invoke-Fixture([string]$Name, [bool]$Existing, [bool]$Persistent, [bool]$Fail) {
    $root = Join-Path $fixtureRoot $Name
    $bin = Join-Path $root "bin"
    New-Item -ItemType Directory -Path $bin -Force | Out-Null
    if ($Existing) {
        New-Item -ItemType Directory -Path (Join-Path $root "console/assets") -Force | Out-Null
        Set-Content (Join-Path $bin "skilltape.exe") "old cli"
        Set-Content (Join-Path $bin "skilltape-console-api.exe") "old api"
        Set-Content (Join-Path $root "console/index.html") "old UI"
        Set-Content (Join-Path $root "console/assets/old.js") "old asset"
    }
    Set-Content (Join-Path $root "receipt.json") "preserved receipt"
    $global:SkillTapeInstallerFixture.failureDestination = if ($Fail) { Join-Path $root "console" } else { "" }
    $global:SkillTapeInstallerFixture.persistentFailure = $Persistent
    $global:SkillTapeInstallerFixture.failureCount = 0
    $global:SkillTapeInstallerFixture.cleanupFailureDestination = if ($Name -eq "cleanup") { Join-Path $root ".skilltape-install.lock" } else { "" }
    $failed = $false
    try { & $InstallerPath -Version "0.1.0" -InstallDir $bin -Target "x86_64-pc-windows-msvc" | Out-Null }
    catch { $failed = $true; if (-not $Fail) { throw } }
    Assert-Fixture ($failed -eq $Fail) "Installer outcome did not match the injected failure."
    Assert-Fixture ((Get-Content (Join-Path $root "receipt.json") -Raw).Trim() -eq "preserved receipt") "Receipt changed."
    if (-not $Fail) {
        Assert-Fixture ((Get-Content (Join-Path $bin "skilltape.exe") -Raw).Trim() -eq "new cli") "New CLI missing."
        Assert-Fixture ((Get-Content (Join-Path $bin "skilltape-console-api.exe") -Raw).Trim() -eq "new api") "New API missing."
        Assert-Fixture ((Get-Content (Join-Path $root "console/index.html") -Raw).Trim() -eq "new UI") "New UI missing."
    }
    else {
        Assert-Fixture ($global:SkillTapeInstallerFixture.failureCount -ge 1) "Failure injection was not reached."
        if ($Existing) {
            Assert-Fixture ((Get-Content (Join-Path $bin "skilltape.exe") -Raw).Trim() -eq "old cli") "Old CLI was not restored."
            Assert-Fixture ((Get-Content (Join-Path $bin "skilltape-console-api.exe") -Raw).Trim() -eq "old api") "Old API was not restored."
            $uiRoot = Join-Path $root "console"
            if ($Persistent) {
                $backups = @(Get-ChildItem -LiteralPath $root -Directory -Force -Filter ".skilltape-rollback-*")
                Assert-Fixture ($backups.Count -eq 1) "Recovery backup was not retained."
                $uiRoot = Join-Path $backups[0].FullName "previous-console"
            }
            Assert-Fixture ((Get-Content (Join-Path $uiRoot "index.html") -Raw).Trim() -eq "old UI") "Old UI was lost."
            Assert-Fixture ((Get-Content (Join-Path $uiRoot "assets/old.js") -Raw).Trim() -eq "old asset") "Old asset was lost."
        }
        else {
            Assert-Fixture (-not (Test-Path (Join-Path $bin "skilltape.exe"))) "Partial fresh CLI remained."
            Assert-Fixture (-not (Test-Path (Join-Path $bin "skilltape-console-api.exe"))) "Partial fresh API remained."
            Assert-Fixture (-not (Test-Path (Join-Path $root "console"))) "Partial fresh UI remained."
        }
    }
    Assert-Fixture ((Test-Path (Join-Path $root ".skilltape-install.lock")) -eq ($Persistent -or $Name -eq "cleanup")) "Unexpected lock retention."
}
try {
    $env:SKILLTAPE_RELEASE_BASE_URL = "https://fixture.invalid/releases/download"
    $env:SKILLTAPE_RELEASE_API_BASE_URL = $null
    $env:SKILLTAPE_RELEASE_TOKEN = $null
    $package = Join-Path $fixtureRoot "package"
    New-Item -ItemType Directory -Path (Join-Path $package "console/assets") -Force | Out-Null
    Set-Content (Join-Path $package "skilltape.exe") "new cli"
    Set-Content (Join-Path $package "skilltape-console-api.exe") "new api"
    Set-Content (Join-Path $package "console/index.html") "new UI"
    Set-Content (Join-Path $package "console/assets/app.js") "new asset"
    $global:SkillTapeInstallerFixture.archiveFixture = Join-Path $fixtureRoot "release.zip"
    Compress-Archive -Path $package -DestinationPath $global:SkillTapeInstallerFixture.archiveFixture
    $global:SkillTapeInstallerFixture.checksumFixture = Join-Path $fixtureRoot "checksums.txt"
    Set-Content $global:SkillTapeInstallerFixture.checksumFixture ((Get-FileHash $global:SkillTapeInstallerFixture.archiveFixture -Algorithm SHA256).Hash + "  skilltape-v0.1.0-x86_64-pc-windows-msvc.zip")
    Invoke-Fixture "success" $true $false $false
    Invoke-Fixture "rollback" $true $false $true
    Invoke-Fixture "fresh" $false $false $true
    Invoke-Fixture "retained" $true $true $true
    Invoke-Fixture "cleanup" $true $false $false
    Write-Output "PowerShell installer fixtures passed"
}
finally {
    $env:SKILLTAPE_RELEASE_BASE_URL = $previousBase
    $env:SKILLTAPE_RELEASE_API_BASE_URL = $previousApi
    $env:SKILLTAPE_RELEASE_TOKEN = $previousToken
    Remove-Variable SkillTapeInstallerFixture -Scope Global
    Remove-Item -LiteralPath $fixtureRoot -Recurse -Force -ErrorAction SilentlyContinue
}
