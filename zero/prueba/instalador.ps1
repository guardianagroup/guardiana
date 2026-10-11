# The GUARDIANA ZERO installer, end to end on a clean Windows (zero.yml, after the browser's own
# test): what a person gets after double-clicking it. Usage:
#   powershell -NoProfile -File zero/prueba/instalador.ps1 -Msi dist/guardiana-zero-<v>-windows-x64.msi `
#       -Viejo msi/prueba-viejo.msi -Exe zero/navegador/target/release/guardiana-zero.exe -Version <v> `
#       [-Otros "dist/...-en.msi;dist/...-pt.msi"]
# It checks, in this order: the package asks for no administrator rights; an older version
# (-Viejo, the same program packaged as 1.0.0) installs and opens; the new installer, run while the
# older one is OPEN, closes it the way its own X does (it saves and exits by itself, never killed)
# and replaces it in place, with one entry in «Apps»; the new program is byte for byte the one
# GitHub built, with its readme and its two shortcuts, and opens; the other two languages install
# over it; uninstalling removes program and shortcuts and leaves the person's data. Every check
# prints «OK» or «FALLO», and the run fails on any «FALLO». Every wait has an end: a run that
# hung once on a clean runner (10 Oct 2026, 50 minutes, no log) must fail with its reason instead.
param(
    [Parameter(Mandatory = $true)][string]$Msi,
    [Parameter(Mandatory = $true)][string]$Viejo,
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Version,
    # One string, paths separated by «;»: «powershell -File» does not take arrays, and a list that
    # arrives as one path would have msiexec show its help window, waiting for a click that never comes.
    [string]$Otros = ''
)
$ErrorActionPreference = "Continue"
$fallos = 0
$bien = 0
# GitHub shows ten annotations of a kind per step: every «FALLO» goes into one at the end, and
# what was seen on the way (where Windows put things, what msiexec logged) into another.
$todosFallos = New-Object System.Collections.Generic.List[string]
$visto = New-Object System.Collections.Generic.List[string]
function Comprueba([bool]$ok, [string]$que) {
    if ($ok) { $script:bien++; Write-Host "OK    $que" } else { $script:fallos++; Write-Host "FALLO $que"; $script:todosFallos.Add($que) }
}
function Anota([string]$que) { Write-Host "VISTO $que"; $script:visto.Add($que) }
# The end of an msiexec log, for the annotation when something fails: the run page shows it,
# the log itself only a signed-in browser can open.
function Cola([string]$log, [string]$patron = 'error|Return value 3|CloseApplication|in use|RestartManager|Product:') {
    $l = @(Get-Content $log -ErrorAction SilentlyContinue | Where-Object { $_ -match $patron })
    return (($l | Select-Object -Last 12 | ForEach-Object { $_.Trim() }) -join ' / ')
}
# Each argument quoted: runner paths have no spaces today, a person's may. Never «Start-Process
# -Wait»: it waits for every process msiexec leaves behind, and has no end.
function Msiexec([string[]]$argumentos, [string]$log) {
    $todos = @($argumentos | ForEach-Object { if ($_ -match '^/') { $_ } else { '"' + $_ + '"' } }) + @('/qn', '/norestart', '/l*v', ('"' + $log + '"'))
    $p = Start-Process -FilePath msiexec.exe -ArgumentList $todos -PassThru
    $null = $p.Handle  # without it, ExitCode reads empty once the process is gone
    if (-not $p.WaitForExit(240000)) {
        Comprueba $false "msiexec $($argumentos -join ' ') no terminó en 4 minutos: $(Ventanas)"
        Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
        return -1
    }
    if ($p.ExitCode -ne 0) { Anota "msiexec $($argumentos[0]) $($p.ExitCode): $(Cola $log)" }
    return $p.ExitCode
}
# What is open on screen that nobody can click on a runner: a message box from ZERO or msiexec.
function Ventanas() {
    $v = @(Get-Process -ErrorAction SilentlyContinue | Where-Object { ($_.ProcessName -in @('GUARDIANA ZERO', 'msiexec')) -and $_.MainWindowTitle } | ForEach-Object { "$($_.ProcessName) $($_.Id): «$($_.MainWindowTitle)»" })
    if ($v.Count -eq 0) { return 'ninguna ventana a la vista' }
    return ($v -join '; ')
}
# Opens the installed program and waits for its own mark (en-marcha.txt: «<version> <pid>»).
function Abre([string]$version) {
    Remove-Item -Force -ErrorAction SilentlyContinue $enMarcha
    $p = Start-Process -FilePath $instalado -PassThru
    $null = $p.Handle
    for ($i = 0; $i -lt 60; $i++) {
        Start-Sleep -Milliseconds 500
        if ((Test-Path $enMarcha) -and ((Get-Content $enMarcha -ErrorAction SilentlyContinue) -match "^$([regex]::Escape($version)) $($p.Id)$")) { return $p }
    }
    Anota "no abrió en 30 s: $(Ventanas)"
    return $null
}
# Where Windows Installer registered the product, read from its own keys, not guessed: the
# products of this family in «Apps» (both hives; each language is its own product code), and for
# whom (a per-user product is under HKCU\Software\Microsoft\Installer\Products, a per-machine one
# under HKLM\SOFTWARE\Classes\Installer\Products, both by the «packed» product code).
$colmenas = @('HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall', 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall', 'HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall')
function Productos() {
    $r = @()
    foreach ($h in $colmenas) {
        if (Test-Path $h) { $r += @(Get-ChildItem $h | Where-Object { (Get-ItemProperty $_.PSPath).DisplayName -eq 'GUARDIANA ZERO' } | ForEach-Object { $_.PSChildName }) }
    }
    return , @($r | Select-Object -Unique)
}
function Instalado() { $l = Productos; if ($l.Count -gt 0) { return $l[0] }; return $null }
function Empaqueta([string]$g) {
    $h = $g.Trim('{', '}').Replace('-', '').ToUpper()
    if ($h.Length -ne 32) { return '' }
    $p = ''
    foreach ($tramo in @(@(0, 8), @(8, 4), @(12, 4))) {
        $t = $h.Substring($tramo[0], $tramo[1]).ToCharArray(); [array]::Reverse($t); $p += -join $t
    }
    for ($i = 16; $i -lt 32; $i += 2) { $p += "$($h[$i + 1])$($h[$i])" }
    return $p
}
function ParaQuien([string]$c) {
    $e = Empaqueta $c
    if (-not $e) { return '?' }
    if (Test-Path "HKCU:\Software\Microsoft\Installer\Products\$e") { return 'usuario' }
    if (Test-Path "HKLM:\SOFTWARE\Classes\Installer\Products\$e") { return 'equipo' }
    return '?'
}
# The «Apps» entry, wherever Windows keeps it (the hive is noted, not assumed).
function Entrada([string]$c) {
    foreach ($h in $colmenas) {
        if ($c -and (Test-Path "$h\$c")) { return "$h\$c" }
    }
    return $null
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
Comprueba (($null -eq $allusers) -or ($allusers -eq '')) "ALLUSERS vacío: solo para este usuario ($allusers)"
Comprueba ((Propiedad 'ProductName') -eq 'GUARDIANA ZERO') "el producto se llama GUARDIANA ZERO"
Comprueba ((Propiedad 'ProductVersion') -eq $Version) "la versión del paquete es $Version"
Comprueba ((Propiedad 'ProductLanguage') -eq '0') "el paquete es neutro de idioma"
Comprueba ((Propiedad 'ARPNOMODIFY') -eq '1') "en «Aplicaciones» no hay «Modificar»"
$codigo = Propiedad 'ProductCode'
$familia = Propiedad 'UpgradeCode'
Comprueba ($familia -eq '{7C2F0A4E-9B63-4D7A-8E21-5F3C6D1B9A08}') "la familia (UpgradeCode) es la de siempre ($familia)"
$enMarcha = Join-Path $datos "en-marcha.txt"

# 2. A person's data from before (an earlier version, or the loose .exe) must survive.
New-Item -ItemType Directory -Force -Path $datos | Out-Null
$marca = Join-Path $datos "marca-de-prueba.txt"
Set-Content -Path $marca -Value "antes de instalar"

# 3. An older version first, open, as a person who updates would have it.
$r = Msiexec @('/i', (Resolve-Path $Viejo).Path) (Join-Path $logs "instalar-viejo.log")
Comprueba ($r -eq 0) "la versión vieja (1.0.0) se instala ($r)"
$viejo = Abre $Version  # the same program inside, packaged as 1.0.0
Comprueba ($null -ne $viejo) "la versión vieja abre"
$codigoViejo = Instalado
Comprueba ($null -ne $codigoViejo) "Windows Installer la tiene instalada ($codigoViejo)"
Comprueba ((ParaQuien $codigoViejo) -eq 'usuario') "solo para este usuario ($(ParaQuien $codigoViejo))"
Anota "la vieja: entrada de «Aplicaciones» en $(Entrada $codigoViejo)"

# 4. The new installer while the old one is open: it must be asked to close (WM_CLOSE: it saves
#    and exits by itself) and be replaced in place, with no «file in use» and no reboot.
$r = Msiexec @('/i', (Resolve-Path $Msi).Path) (Join-Path $logs "instalar.log")
Comprueba ($r -eq 0) "el instalador nuevo, con la vieja abierta, termina con 0 ($r)"
Anota "cierre de la vieja, según msiexec: $(Cola (Join-Path $logs 'instalar.log') 'CloseApplication|RestartManager|RmShutdown|in use|FilesInUse|WixCloseApplications|Doing action: (RemoveExistingProducts|InstallValidate)')"
if ($null -ne $viejo) {
    $cerro = $viejo.WaitForExit(15000)
    Comprueba $cerro "la versión vieja se cerró para dejar paso ($(Ventanas))"
    if ($cerro) {
        Comprueba ($viejo.ExitCode -eq 0) "y se cerró ella sola, sin que nadie la matara (código $($viejo.ExitCode))"
        Comprueba (-not ((Get-Content $enMarcha -ErrorAction SilentlyContinue) -match " $($viejo.Id)$")) "y quitó su marca de en marcha al salir, como con su X"
    } else {
        Stop-Process -Id $viejo.Id -Force -ErrorAction SilentlyContinue
    }
}
$productos = Productos
Comprueba (($productos.Count -eq 1) -and ($productos[0] -eq $codigo)) "una sola GUARDIANA ZERO instalada, la nueva ($($productos -join ', '))"
Comprueba ($codigoViejo -ne $codigo) "la vieja era otro producto de la misma familia ($codigoViejo)"
Comprueba ((ParaQuien $codigo) -eq 'usuario') "la nueva también, solo para este usuario ($(ParaQuien $codigo))"
Comprueba (Test-Path $instalado) "deja GUARDIANA ZERO.exe en $carpeta"
if (Test-Path $instalado) {
    Comprueba ((Get-FileHash -Algorithm SHA256 $instalado).Hash -eq $huella) "y es, byte a byte, el que compiló GitHub"
}
Comprueba (Test-Path (Join-Path $carpeta "LEEME.txt")) "deja LEEME.txt al lado"
Comprueba ((Test-Path $inicio) -and ((Lee-Lnk $inicio) -eq $instalado)) "acceso en el menú Inicio, al programa instalado"
Comprueba ((Test-Path $escritorio) -and ((Lee-Lnk $escritorio) -eq $instalado)) "acceso en el escritorio, al programa instalado"
$arp = Entrada $codigo
Anota "la nueva: entrada de «Aplicaciones» en $arp"
Comprueba ($null -ne $arp) "aparece en «Aplicaciones»"
if ($arp) {
    $p = Get-ItemProperty $arp
    Comprueba ($p.DisplayName -eq 'GUARDIANA ZERO') "con el nombre GUARDIANA ZERO ($($p.DisplayName))"
    Comprueba ($p.Publisher -eq 'GUARDIANA GROUP') "y el editor GUARDIANA GROUP ($($p.Publisher))"
    Comprueba ($p.DisplayVersion -eq $Version) "y la versión $Version ($($p.DisplayVersion))"
}
Comprueba ((Get-Content $marca) -eq 'antes de instalar') "los datos de antes siguen ahí"

# 5. The new program opens (en-marcha.txt carries its version and PID), and its log says so.
$nuevo = Abre $Version
Comprueba ($null -ne $nuevo) "el programa nuevo abre (en-marcha.txt: $(Get-Content $enMarcha -ErrorAction SilentlyContinue))"
$reg = Get-Content (Join-Path $datos "registro.txt") -ErrorAction SilentlyContinue
Comprueba (($reg | Where-Object { $_ -match 'arranque: GUARDIANA ZERO ' + [regex]::Escape($Version) }).Count -ge 1) "y su registro anota el arranque de la $Version"
if ($null -ne $nuevo) {
    $nuevo.CloseMainWindow() | Out-Null
    Comprueba ($nuevo.WaitForExit(15000)) "y se cierra con su X"
}

# 6. The other languages install and uninstall too, with their own readme.
foreach ($otro in @($Otros -split ';' | Where-Object { $_ })) {
    $nombre = Split-Path -Leaf $otro
    if (-not (Test-Path $otro)) { Comprueba $false "no existe $otro"; continue }
    $leeme = if ($nombre -match '-pt\.msi$') { 'LEIAME.txt' } elseif ($nombre -match '-en\.msi$') { 'README.txt' } else { 'LEEME.txt' }
    $r = Msiexec @('/i', (Resolve-Path $otro).Path) (Join-Path $logs "instalar-$nombre.log")
    Comprueba (($r -eq 0) -and (Test-Path (Join-Path $carpeta $leeme))) "$nombre instala encima y deja $leeme ($r)"
    Comprueba ((Productos).Count -eq 1) "y sigue habiendo una sola GUARDIANA ZERO instalada ($((Productos) -join ', '))"
}

# 7. Uninstall: program and shortcuts go, the person's data stays.
# The product installed now: after the other languages, it is the last one's, not the first's.
$codigo = Instalado
$arp = Entrada $codigo
if (-not $codigo) { Comprueba $false "no hay nada que desinstalar"; $codigo = '{00000000-0000-0000-0000-000000000000}' }
$r = Msiexec @('/x', $codigo) (Join-Path $logs "desinstalar.log")
Comprueba ($r -eq 0) "msiexec /x termina con 0 ($r)"
Comprueba (-not (Test-Path $instalado)) "quita el programa"
Comprueba (-not (Test-Path $carpeta)) "y su carpeta"
Comprueba (-not (Test-Path $inicio)) "y el acceso del menú Inicio"
Comprueba (-not (Test-Path $escritorio)) "y el del escritorio"
Comprueba ((-not $arp) -or (-not (Test-Path $arp))) "y la entrada de «Aplicaciones»"
Comprueba ((Productos).Count -eq 0) "y Windows Installer ya no la tiene"
Comprueba ((Test-Path $marca) -and ((Get-Content $marca) -eq 'antes de instalar')) "pero los datos de la persona se quedan"

# Nothing of ours may be left running: a runner waits for every process a step leaves behind.
$quedan = @(Get-Process -Name 'GUARDIANA ZERO' -ErrorAction SilentlyContinue)
Comprueba ($quedan.Count -eq 0) "no queda ningún GUARDIANA ZERO abierto ($(Ventanas))"
$quedan | Stop-Process -Force -ErrorAction SilentlyContinue

function Una([string]$t) { $t = $t -replace "`r?`n", ' '; if ($t.Length -gt 3800) { $t = $t.Substring(0, 3800) + '...' }; return $t }
Write-Host "::notice title=instalador visto::$(Una ($visto -join ' | '))"
if ($fallos -gt 0) { Write-Host "::error title=instalador $fallos fallos::$(Una ($todosFallos -join ' | '))" }
Write-Host "::notice title=instalador::$bien bien, $fallos fallos"
if ($fallos -gt 0) { exit 1 }
