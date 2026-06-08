param(
    [ValidateSet("layout", "pack", "sign")]
    [string] $Command = "layout",
    [string] $Configuration = "Release"
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$AppDir = Resolve-Path (Join-Path $ScriptDir "..")
$RepoDir = Resolve-Path (Join-Path $AppDir "..")
$BuildDir = Join-Path $AppDir "windows\build"
$LayoutDir = Join-Path $BuildDir "HDMTester"
$ManifestPath = Join-Path $AppDir "windows\Package.appxmanifest"
$AssetsDir = Join-Path $AppDir "windows\Assets"
$MsixPath = Join-Path $BuildDir "HDMTester.msix"

Push-Location $RepoDir
try {
    cargo build -p hdm-am-app --release --bin hdm-app
}
finally {
    Pop-Location
}

if (Test-Path $LayoutDir) {
    Remove-Item -Recurse -Force $LayoutDir
}
New-Item -ItemType Directory -Force $LayoutDir | Out-Null

Copy-Item (Join-Path $RepoDir "target\release\hdm-app.exe") (Join-Path $LayoutDir "hdm-app.exe")
Copy-Item $ManifestPath (Join-Path $LayoutDir "AppxManifest.xml")
Copy-Item -Recurse $AssetsDir (Join-Path $LayoutDir "Assets")

Write-Host "Layout: $LayoutDir"

if ($Command -eq "layout") {
    exit 0
}

$MakeAppx = Get-Command makeappx.exe -ErrorAction SilentlyContinue
if (-not $MakeAppx) {
    throw "makeappx.exe is required. Install the Windows SDK and run from a Developer PowerShell."
}

New-Item -ItemType Directory -Force $BuildDir | Out-Null
& $MakeAppx.Source pack /d $LayoutDir /p $MsixPath /o
Write-Host "MSIX: $MsixPath"

if ($Command -eq "pack") {
    exit 0
}

$SignTool = Get-Command signtool.exe -ErrorAction SilentlyContinue
if (-not $SignTool) {
    throw "signtool.exe is required. Install the Windows SDK and run from a Developer PowerShell."
}

if (-not $env:WINDOWS_SIGN_CERT_THUMBPRINT) {
    throw "WINDOWS_SIGN_CERT_THUMBPRINT is required for signing."
}

& $SignTool.Source sign `
    /fd SHA256 `
    /td SHA256 `
    /tr "http://timestamp.digicert.com" `
    /sha1 $env:WINDOWS_SIGN_CERT_THUMBPRINT `
    $MsixPath

Write-Host "Signed MSIX: $MsixPath"
