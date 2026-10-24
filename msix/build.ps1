# Packages target\release\deeraw.exe as deeraw-windows-x64.msix for the Microsoft Store (which signs it).
# Run from the repo root after `cargo build --release`: powershell -File msix\build.ps1
# The identity is the app's in Partner Center (Product identity page).
param(
    [string]$IdentityName = "Briedis.Deeraw",
    [string]$Publisher = "CN=5063D15F-3615-403A-8689-C049FF108DAF",
    [string]$PublisherDisplayName = "Briedis",
    [string]$Out = "deeraw-windows-x64.msix"
)
$ErrorActionPreference = "Stop"

# cargo prints UTF-8; Windows PowerShell would read it as the OEM code page.
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$meta = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
$package = $meta.packages[0]
# MSIX versions have four parts; the Store requires the last to be 0.
$version = "$($package.version -replace '-.*$', '').0"

$sdk = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\10.*\x64\makeappx.exe" |
    Sort-Object { [version]$_.Directory.Parent.Name } | Select-Object -Last 1
if (-not $sdk) { throw "makeappx.exe not found (Windows SDK)" }
$bin = $sdk.Directory.FullName

$dir = Join-Path ([IO.Path]::GetTempPath()) "deeraw-msix"
Remove-Item -Recurse -Force $dir -ErrorAction SilentlyContinue
New-Item -ItemType Directory "$dir\Assets" | Out-Null
Copy-Item target\release\deeraw.exe, target\release\deeraw.pdb, LICENSE.md $dir
Copy-Item msix\Assets\*.png "$dir\Assets"

$manifest = (Get-Content -Raw -Encoding UTF8 msix\AppxManifest.xml).
    Replace('$IdentityName$', $IdentityName).
    Replace('$Publisher$', [Security.SecurityElement]::Escape($Publisher)).
    Replace('$PublisherDisplayName$', [Security.SecurityElement]::Escape($PublisherDisplayName)).
    Replace('$Description$', [Security.SecurityElement]::Escape($package.description)).
    Replace('$Version$', $version)
[IO.File]::WriteAllText("$dir\AppxManifest.xml", $manifest, (New-Object Text.UTF8Encoding $false))

# resources.pri lets Windows pick the logo variant (scale-200, targetsize-24, ...) for each place.
$pri = Join-Path ([IO.Path]::GetTempPath()) "deeraw-priconfig.xml"
& "$bin\makepri.exe" createconfig /cf $pri /dq en-US /o | Out-Null
if ($LASTEXITCODE) { throw "makepri createconfig failed" }
# Keep every logo in the one resources.pri (the default config splits scales off for resource packs).
$config = [xml](Get-Content -Raw $pri)
$packaging = $config.SelectSingleNode("//packaging")
if ($packaging) { $packaging.ParentNode.RemoveChild($packaging) | Out-Null }
$config.Save($pri)
& "$bin\makepri.exe" new /pr $dir /cf $pri /mn "$dir\AppxManifest.xml" /of "$dir\resources.pri" /o | Out-Null
if ($LASTEXITCODE) { throw "makepri new failed" }

& "$bin\makeappx.exe" pack /d $dir /p $Out /o | Out-Null
if ($LASTEXITCODE) { throw "makeappx pack failed" }
Write-Host "Built $Out (version $version, identity $IdentityName, publisher $Publisher)"
