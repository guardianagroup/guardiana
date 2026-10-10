# The GUARDIANA ZERO installer, end to end on a clean Windows (zero.yml, after the browser's own
# test): what a person gets after double-clicking it. Usage:
#   powershell -NoProfile -File zero/prueba/instalador.ps1 -Msi dist/guardiana-zero-<v>-windows-x64.msi `
#       -Exe zero/navegador/target/release/guardiana-zero.exe -Version <v> [-Otros dist/...-en.msi,dist/...-pt.msi]
# It checks, in this order: the package asks for no administrator rights; installing leaves the
# program (byte for byte the one GitHub built), the readme and the two shortcuts; the installed
# program opens; installing again while it is open closes it (it saves) and replaces it in place;
# uninstalling removes program and shortcuts and leaves the person's data. Every check prints
# «OK» or «FALLO», and the run fails on any «FALLO».
param(
    [Parameter(Mandatory = $true)][string]$Msi,
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Version,
    [string[]]$Otros = @()
)
$ErrorActionPreference = "Continue"
$fallos = 0
$bien = 0
function Comprueba([bool]$ok, [string]$que) {
    if ($ok) { $script:bien++; Write-Host "OK    $que" } else { $script:fallos++; Write-Host "FALLO $que"; Write-Host "::error title=instalador::$que" }
}
function Msiexec([string[]]$args, [string]$log) {
    $p = Start-Process -FilePath msiexec.exe -ArgumentList ($args + @('/qn', '/norestart', '/l*v', $log)) -Wait -PassThru
    return $p.ExitCode
}
function Lee-Lnk([string]$ruta) {
    $sh = New-Object -ComObject WScript.Shell
    return $sh.CreateShortcut($ruta).TargetPath
}

$carpeta = Join-Path $env:LOCALAPPDATA "Programs\GUARDIANA ZERO"
$instalado = Join-Path $carpeta "GUARDIANA ZERO.exe"
$datos = Join-Path $env:LOCALAPPDATA "GUARDIANA ZERO\datos"
$inicio = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\GUARDIANA ZERO.lnk"
$escritorio = Join-Path ([Environment]::GetFolderPath('Desktop')) "GUARDIANA ZERO.lnk"
$logs = Join-Path $env:RUNNER_TEMP "instalador"
if (-not $env:RUNNER_TEMP) { $logs = Join-Path $env:TEMP "instalador-zero" }
New-Item -ItemType Directory -Force -Path $logs | Out-Null
$huella = (Get-FileHash -Algorithm SHA256 $Exe).Hash

# 1. The package itself: per user, no elevation (summary information, word count bit 8), language
#    neutral, and the product name a person reads in «Apps».
$wi = New-Object -ComObject WindowsInstaller.Installer
$db = $wi.GetType().InvokeMember('OpenDatabase', 'InvokeMethod', $null, $wi, @((Resolve-Path $Msi).Path, 0))
$si = $db.GetType().InvokeMember('SummaryInformation', 'GetProperty', $null, $db, @(0))
$wc = [int]$si.GetType().InvokeMember('Property', 'GetProperty', $null, $si, @(15))
Comprueba (($wc -band 8) -eq 8) "el paquete declara que no hace falta administrador (word count $wc)"
function Propiedad([string]$nombre) {
    $v = $db.GetType().InvokeMember('OpenView', 'InvokeMethod', $null, $db, @("SELECT Value FROM Property WHERE Property='$nombre'"))
    $v.GetType().InvokeMember('Execute', 'InvokeMethod', $null, $v, $null) | Out-Null
    $r = $v.GetType().InvokeMember('Fetch', 'InvokeMethod', $null, $v, $null)
    if ($null -eq $r) { return $null }
    return $r.GetType().InvokeMember('StringData', 'GetProperty', $null, $r, @(1))
}
$allusers = Propiedad 'ALLUSERS'
Comprueba (($null -eq $allusers) -or ($allusers -eq '') -or ($allusers -eq '2')) "ALLUSERS no fuerza la instalación para todos ($allusers)"
Comprueba ((Propiedad 'ProductName') -eq 'GUARDIANA ZERO') "el producto se llama GUARDIANA ZERO"
Comprueba ((Propiedad 'ProductVersion') -eq $Version) "la versión del paquete es $Version"
Comprueba ((Propiedad 'ProductLanguage') -eq '0') "el paquete es neutro de idioma"
Comprueba ((Propiedad 'ARPNOMODIFY') -eq '1') "en «Aplicaciones» no hay «Modificar»"
$codigo = Propiedad 'ProductCode'

# 2. A person's data from before (an earlier version, or the loose .exe) must survive.
New-Item -ItemType Directory -Force -Path $datos | Out-Null
$marca = Join-Path $datos "marca-de-prueba.txt"
Set-Content -Path $marca -Value "antes de instalar"

# 3. Install.
$r = Msiexec @('/i', (Resolve-Path $Msi).Path) (Join-Path $logs "instalar.log")
Comprueba ($r -eq 0) "msiexec /i termina con 0 ($r)"
Comprueba (Test-Path $instalado) "deja GUARDIANA ZERO.exe en $carpeta"
if (Test-Path $instalado) {
    Comprueba ((Get-FileHash -Algorithm SHA256 $instalado).Hash -eq $huella) "y es, byte a byte, el que compiló GitHub"
}
Comprueba (Test-Path (Join-Path $carpeta "LEEME.txt")) "deja LEEME.txt al lado"
Comprueba ((Test-Path $inicio) -and ((Lee-Lnk $inicio) -eq $instalado)) "acceso en el menú Inicio, al programa instalado"
Comprueba ((Test-Path $escritorio) -and ((Lee-Lnk $escritorio) -eq $instalado)) "acceso en el escritorio, al programa instalado"
$arp = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$codigo"
Comprueba (Test-Path $arp) "aparece en «Aplicaciones» del usuario (HKCU), no de la máquina"
if (Test-Path $arp) {
    $p = Get-ItemProperty $arp
    Comprueba ($p.DisplayName -eq 'GUARDIANA ZERO') "con el nombre GUARDIANA ZERO ($($p.DisplayName))"
    Comprueba ($p.Publisher -eq 'GUARDIANA GROUP') "y el editor GUARDIANA GROUP ($($p.Publisher))"
    Comprueba ($p.DisplayVersion -eq $Version) "y la versión $Version ($($p.DisplayVersion))"
}
Comprueba (-not (Test-Path "HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$codigo")) "nada en la parte de la máquina (HKLM)"
Comprueba ((Get-Content $marca) -eq 'antes de instalar') "los datos de antes siguen ahí"

# 4. The installed program opens (its own log says so: «arranque: GUARDIANA ZERO <v>», and
#    en-marcha.txt carries its version and PID).
$enMarcha = Join-Path $datos "en-marcha.txt"
Remove-Item -Force -ErrorAction SilentlyContinue $enMarcha
$proc = Start-Process -FilePath $instalado -PassThru
$abierto = $false
for ($i = 0; $i -lt 60; $i++) {
    Start-Sleep -Milliseconds 500
    if ((Test-Path $enMarcha) -and ((Get-Content $enMarcha -ErrorAction SilentlyContinue) -match "^$([regex]::Escape($Version)) $($proc.Id)$")) { $abierto = $true; break }
}
Comprueba $abierto "el programa instalado abre (en-marcha.txt: $(Get-Content $enMarcha -ErrorAction SilentlyContinue))"

# 5. Install again while it is open: it must be asked to close (it saves) and be replaced in
#    place, with no «file in use» and no reboot.
$r = Msiexec @('/i', (Resolve-Path $Msi).Path) (Join-Path $logs "instalar-encima.log")
Comprueba ($r -eq 0) "instalar encima con el programa abierto termina con 0 ($r)"
Start-Sleep -Seconds 2
Comprueba ($proc.HasExited) "el programa abierto se cerró solo para dejar paso"
Comprueba ((Test-Path $instalado) -and ((Get-FileHash -Algorithm SHA256 $instalado).Hash -eq $huella)) "y el programa sigue en su sitio, entero"
Comprueba ((Test-Path $inicio) -and (Test-Path $escritorio)) "con sus dos accesos"
$reg = Get-Content (Join-Path $datos "registro.txt") -ErrorAction SilentlyContinue
Comprueba (($reg | Where-Object { $_ -match 'arranque: GUARDIANA ZERO ' + [regex]::Escape($Version) }).Count -ge 1) "y su registro anota el arranque"

# 6. The other languages install and uninstall too, with their own readme.
foreach ($otro in $Otros) {
    $nombre = Split-Path -Leaf $otro
    $leeme = if ($nombre -match '-pt\.msi$') { 'LEIAME.txt' } elseif ($nombre -match '-en\.msi$') { 'README.txt' } else { 'LEEME.txt' }
    $r = Msiexec @('/i', (Resolve-Path $otro).Path) (Join-Path $logs "instalar-$nombre.log")
    Comprueba (($r -eq 0) -and (Test-Path (Join-Path $carpeta $leeme))) "$nombre instala encima y deja $leeme ($r)"
}

# 7. Uninstall: program and shortcuts go, the person's data stays.
$r = Msiexec @('/x', $codigo) (Join-Path $logs "desinstalar.log")
Comprueba ($r -eq 0) "msiexec /x termina con 0 ($r)"
Comprueba (-not (Test-Path $instalado)) "quita el programa"
Comprueba (-not (Test-Path $carpeta)) "y su carpeta"
Comprueba (-not (Test-Path $inicio)) "y el acceso del menú Inicio"
Comprueba (-not (Test-Path $escritorio)) "y el del escritorio"
Comprueba (-not (Test-Path $arp)) "y la entrada de «Aplicaciones»"
Comprueba ((Test-Path $marca) -and ((Get-Content $marca) -eq 'antes de instalar')) "pero los datos de la persona se quedan"

Write-Host "::notice title=instalador::$bien bien, $fallos fallos"
if ($fallos -gt 0) { exit 1 }
