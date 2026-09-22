# build/msi.ps1: build the Windows MSI on a Windows machine with WiX 5.
# WiX 4/5 as a dotnet tool runs on Linux/macOS but rejects Directory/@Name
# there (BundleValidator.GetCanonicalRelativePath assumes a "C:\" root), so
# until that is fixed upstream the MSI is built on Windows (DECISIONES #34).
# WiX needs only the .NET 6+ runtime: pass the tool's wix.dll with -Wix, or
# install it with `dotnet tool install --global wix --version 5.0.2`.
#
# Usage:
#   powershell -NoProfile -File build\msi.ps1 -Exe target\x86_64-pc-windows-gnu\release\guardiana.exe `
#       -Version 0.1.0 -Out dist\0.1.0\guardiana-0.1.0-windows-x64.msi [-Idioma es|en|pt] [-Wix C:\path\to\wix.dll]
#
# -Idioma picks the language of the installer: the two screens, the shortcut names, the service
# description and the readme that is installed next to the program. Someone downloading from the
# English page must not be handed a Spanish installer (22 Sep 2026).
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][string]$Out,
    [ValidateSet("es", "en", "pt")][string]$Idioma = "es",
    [string]$Wix = "wix",
    [string]$CertSha1 = "",
    [string]$SignTool = "",
    [string]$TimestampUrl = "http://time.certum.pl"
)
$ErrorActionPreference = "Stop"
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$wxs = Join-Path $here "wix\guardiana.wxs"
# Culture for WiX (its own buttons and ours); the package itself is language neutral.
#
# The three installers MUST share one ProductLanguage. With one LCID per language (10, 1033,
# 1046) Windows Installer treated them as three different products: installing the English one
# over the Spanish left BOTH registered in Programs and Features, both sets of shortcuts and two
# readme files in the folder - measured on the test Windows on 22 Sep 2026. Neutral (0) is also
# the honest value for a program that speaks three languages and picks by the system's own.
$culturas = @{ es = "es-ES"; en = "en-US"; pt = "pt-BR" }
$cultura = $culturas[$Idioma]
$lcid = 0
$readme = Join-Path $here "wix\textos\leeme-$Idioma.txt"
$welcome = Join-Path $here "wix\textos\bienvenida-$Idioma.rtf"
$loc = Join-Path $here "wix\loc-$cultura.wxl"
foreach ($f in @($readme, $welcome, $loc)) { if (-not (Test-Path $f)) { throw "Falta $f" } }
$icon = Join-Path $here "wix\guardiana.ico"
# The exe must carry a version resource. Without one, Windows Installer compares timestamps
# instead of versions and can silently keep the old binary on an upgrade (DECISIONES #93).
if (-not (Test-Path $Exe)) { throw "No existe $Exe" }
$fv = (Get-Item $Exe).VersionInfo.FileVersion
if ([string]::IsNullOrWhiteSpace($fv)) {
    throw "$Exe no tiene FileVersion: compila con un compilador de recursos disponible (zig rc, llvm-rc o windres) para que crates/cli/build.rs incruste la version."
}
Write-Host "guardiana.exe FileVersion = $fv"
$outDir = Split-Path -Parent $Out
if ($outDir -and -not (Test-Path $outDir)) { New-Item -ItemType Directory -Path $outDir | Out-Null }
if ($Wix -like "*.dll") { $cmd = "dotnet"; $pre = @($Wix) } else { $cmd = $Wix; $pre = @() }
# The UI and Util extensions (WixUI_Minimal, WixShellExec) come from NuGet once:
#   dotnet wix.dll extension add -g WixToolset.UI.wixext/5.0.2 WixToolset.Util.wixext/5.0.2
Write-Host "idioma del instalador: $Idioma ($cultura, LCID $lcid)"
& $cmd @pre build -arch x64 -culture $cultura -loc $loc -ext WixToolset.UI.wixext -ext WixToolset.Util.wixext -d "Version=$Version" -d "Lang=$lcid" -d "Exe=$Exe" -d "Readme=$readme" -d "Welcome=$welcome" -d "Icon=$icon" -o $Out $wxs
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
# Code signing of the MSI itself (decision 66): the installer is what the person double-clicks, so
# it must carry the certificate too, not only guardiana.exe. Pass -CertSha1 with the thumbprint of
# the GUARDIANA GROUP certificate that SimplySign publishes in the Windows store; without it the
# MSI is left unsigned and the script says so.
if ($CertSha1) {
    $signtool = if ($SignTool) { $SignTool } else { "signtool.exe" }
    & $signtool sign /sha1 $CertSha1 /fd sha256 /td sha256 /tr $TimestampUrl `
        /d "GUARDIANA" /du "https://guardianagroup.com" $Out
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    & $signtool verify /pa /v $Out
} else {
    Write-Warning "MSI sin firmar: pasa -CertSha1 <huella> cuando exista el certificado (docs/FIRMA-CODIGO.md)."
}
Get-FileHash -Algorithm SHA256 $Out | Format-List
