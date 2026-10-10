# build/msi-zero.ps1: build the GUARDIANA ZERO installer (MSI) on Windows with WiX 5.
# The same tool and the same shape as build/msi.ps1 (GUARDIANA's), for the browser: a per-user
# package (no administrator password) with the welcome screen, the shortcut names and the readme
# in one language. Someone downloading from the English page must not be handed a Spanish
# installer, so the three are built, one per language.
#
# Usage:
#   powershell -NoProfile -File build\msi-zero.ps1 -Exe dist\guardiana-zero-1.0.7-windows-x64.exe `
#       -Version 1.0.7 -Out dist\guardiana-zero-1.0.7-windows-x64.msi [-Idioma es|en|pt] [-Wix C:\path\to\wix.dll]
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][string]$Out,
    [ValidateSet("es", "en", "pt")][string]$Idioma = "es",
    [string]$Wix = "wix"
)
$ErrorActionPreference = "Stop"
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$wxs = Join-Path $here "wix\zero.wxs"
$culturas = @{ es = "es-ES"; en = "en-US"; pt = "pt-BR" }
$cultura = $culturas[$Idioma]
$readme = Join-Path $here "wix\textos\zero-leeme-$Idioma.txt"
$welcome = Join-Path $here "wix\textos\zero-bienvenida-$Idioma.rtf"
$loc = Join-Path $here "wix\loc-zero-$cultura.wxl"
$icon = Join-Path (Split-Path -Parent $here) "zero\navegador\recursos\guardiana-zero.ico"
foreach ($f in @($readme, $welcome, $loc, $icon, $wxs)) { if (-not (Test-Path $f)) { throw "Falta $f" } }
# The exe must carry a version resource: without one, Windows Installer compares timestamps
# instead of versions and can silently keep the old binary on an upgrade (DECISIONES #93).
if (-not (Test-Path $Exe)) { throw "No existe $Exe" }
$fv = (Get-Item $Exe).VersionInfo.FileVersion
if ([string]::IsNullOrWhiteSpace($fv)) { throw "$Exe no tiene FileVersion" }
if (-not $fv.StartsWith($Version)) { throw "$Exe dice FileVersion $fv y el instalador es $Version" }
Write-Host "GUARDIANA ZERO.exe FileVersion = $fv"
$outDir = Split-Path -Parent $Out
if ($outDir -and -not (Test-Path $outDir)) { New-Item -ItemType Directory -Path $outDir | Out-Null }
if ($Wix -like "*.dll") { $cmd = "dotnet"; $pre = @($Wix) } else { $cmd = $Wix; $pre = @() }
Write-Host "idioma del instalador: $Idioma ($cultura)"
& $cmd @pre build -arch x64 -culture $cultura -loc $loc -ext WixToolset.UI.wixext -ext WixToolset.Util.wixext -d "Version=$Version" -d "Exe=$Exe" -d "Readme=$readme" -d "Welcome=$welcome" -d "Icon=$icon" -o $Out $wxs
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Get-FileHash -Algorithm SHA256 $Out | Format-List
