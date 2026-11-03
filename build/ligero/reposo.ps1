# GUARDIANA, «ligero» (1.0.55): lo que gasta el servicio cuando nadie le pide nada, en un Windows de verdad.
# El servicio de verdad (`guardiana service install`: el administrador de servicios lo arranca como SYSTEM, sin
# consola, con su panel y sus vigilancias), con el DNS del equipo apuntado a él como lo deja el instalador
# (`dns --apply --yes`), dentro de un objeto de trabajo de Windows para contar también lo que arranca. Tras un rato
# para que arranque todo, se cuenta durante $Minutos minutos: el procesador del servicio y de todo lo que arranca, los
# procesos que arranca (por nombre), lo que gasta el equipo entero, y lo que tardan en contestar las preguntas de un
# nombre ya guardado, una por segundo, directamente a 127.0.0.1:53 (un parón del resolutor se ve ahí). Al final el DNS
# vuelve a como estaba y el servicio se quita. Lo usa .github/workflows/ligero.yml, en una máquina que se tira después.
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Etiqueta,
    [int]$Minutos = 10,
    [int]$Calentar = 120,
    [string]$Salida = "$env:RUNNER_TEMP\reposo.json"
)
$ErrorActionPreference = 'Stop'
Add-Type -Path (Join-Path $PSScriptRoot 'Medidor.cs')

# Cada proceso que arranque alguien queda en el registro de seguridad de Windows (suceso 4688) con su orden completa:
# así se sabe, sin perder ninguno, qué parte del programa arrancó cada uno. Es la máquina de la prueba.
& auditpol.exe /set /subcategory:"{0cce922b-69ae-11d9-bed3-505054503030}" /success:enable | Out-Null
& reg.exe add 'HKLM\Software\Microsoft\Windows\CurrentVersion\Policies\System\Audit' /v ProcessCreationIncludeCmdLine_Enabled /t REG_DWORD /d 1 /f | Out-Null
$horaArranque = Get-Date

& $Exe service install 2>&1 | ForEach-Object { Write-Host "service install: $_" }
$servicio = $null
for ($i = 0; $i -lt 30 -and -not $servicio; $i++) {
    Start-Sleep -Seconds 1
    $s = Get-CimInstance Win32_Service -Filter "Name='guardiana'"
    if ($s -and $s.ProcessId) { $servicio = [int]$s.ProcessId }
}
if (-not $servicio) { throw 'el servicio no arrancó' }
Write-Host "servicio en marcha (proceso $servicio)"
$trabajo = [Medidor]::Trabajo()
$enTrabajo = $false
try { [Medidor]::Depurar(); [Medidor]::Mete($trabajo, $servicio); $enTrabajo = $true } catch { Write-Host "::warning title=reposo $Etiqueta::no se pudo meter el servicio en un trabajo ($_): sin el procesador de lo que arranca" }

# Que conteste en 127.0.0.1:53 antes de apuntar nada a él.
$listo = $false
for ($i = 0; $i -lt 90 -and -not $listo; $i++) {
    Start-Sleep -Seconds 1
    try { $null = Resolve-DnsName -Name 'www.microsoft.com' -Server 127.0.0.1 -DnsOnly -QuickTimeout -ErrorAction Stop; $listo = $true } catch {}
}
if (-not $listo) { Get-Content -LiteralPath (Join-Path $env:ProgramData 'Guardiana\ultimo-error.txt') -ErrorAction SilentlyContinue; throw 'el servicio no contesta en 127.0.0.1:53' }

& $Exe dns --apply --yes 2>&1 | ForEach-Object { Write-Host "dns --apply: $_" }
$aplicado = $LASTEXITCODE -eq 0
Write-Host "dns --apply salió con $LASTEXITCODE"
Get-DnsClientServerAddress | Where-Object { $_.ServerAddresses } | ForEach-Object { Write-Host ("  {0} {1} {2}" -f $_.InterfaceAlias, $_.AddressFamily, ($_.ServerAddresses -join ',')) }
# Lo que dice el programa (`guardiana dns`: «GUARDIANA es el primero» sale de guardian_is_primary, la lectura de cada
# minuto) frente a lo que dice el PowerShell de siempre, con el DNS apuntado.
& $Exe dns 2>&1 | ForEach-Object { Write-Host "dns: $_" }

Start-Sleep -Seconds $Calentar

function Cpu($nombre) {
    $t = 0.0
    Get-Process -Name $nombre -ErrorAction SilentlyContinue | ForEach-Object { try { $t += $_.TotalProcessorTime.TotalMilliseconds } catch {} }
    return $t
}

$horaVentana = Get-Date
$t0 = if ($enTrabajo) { [Medidor]::Lleva($trabajo) } else { @(0, 0, 0, 0, 0) }
$e0 = [Medidor]::EquipoMs(); $w0 = Cpu 'WmiPrvSE'; $d0 = Cpu 'MsMpEng'
$g0 = (Get-Process -Id $servicio).TotalProcessorTime.TotalMilliseconds
$reloj = [System.Diagnostics.Stopwatch]::StartNew()

# Una pregunta por segundo al resolutor, por UDP y sin pasar por el sistema.
$udp = [System.Net.Sockets.UdpClient]::new()
$udp.Client.ReceiveTimeout = 3000
$udp.Connect('127.0.0.1', 53)
function Pregunta([uint16]$id) {
    $q = [System.Collections.Generic.List[byte]]::new()
    $q.AddRange([byte[]]@([byte]($id -shr 8), [byte]($id -band 255), 1, 0, 0, 1, 0, 0, 0, 0, 0, 0))
    foreach ($parte in 'www.microsoft.com'.Split('.')) { $q.Add([byte]$parte.Length); $q.AddRange([Text.Encoding]::ASCII.GetBytes($parte)) }
    $q.AddRange([byte[]]@(0, 0, 1, 0, 1))
    return $q.ToArray()
}
$tiempos = [System.Collections.Generic.List[double]]::new()
$lentas = [System.Collections.Generic.List[string]]::new()
$perdidas = 0
$id = [uint16]1
$fin = [DateTime]::UtcNow.AddMinutes($Minutos)
while ([DateTime]::UtcNow -lt $fin) {
    $p = Pregunta $id
    $id = [uint16](($id % 65000) + 1)
    $c = [System.Diagnostics.Stopwatch]::StartNew()
    try {
        [void]$udp.Send($p, $p.Length)
        $remoto = [System.Net.IPEndPoint]::new([System.Net.IPAddress]::Any, 0)
        [void]$udp.Receive([ref]$remoto)
        $ms = $c.Elapsed.TotalMilliseconds
        $tiempos.Add($ms)
        if ($ms -gt 20) { $lentas.Add(('{0:N1} ms a los {1:N0} s' -f $ms, $reloj.Elapsed.TotalSeconds)) }
    } catch { $perdidas++ }
    $resto = 1000 - [int]$c.Elapsed.TotalMilliseconds
    if ($resto -gt 0) { Start-Sleep -Milliseconds $resto }
}
$segundos = $reloj.Elapsed.TotalSeconds
$horaFin = Get-Date
$t1 = if ($enTrabajo) { [Medidor]::Lleva($trabajo) } else { @(0, 0, 0, 0, 0) }
$e1 = [Medidor]::EquipoMs(); $w1 = Cpu 'WmiPrvSE'; $d1 = Cpu 'MsMpEng'
$g1 = (Get-Process -Id $servicio).TotalProcessorTime.TotalMilliseconds
$udp.Close()

# Qué parte del programa pidió cada PowerShell, por el principio de su orden.
function QuienPidio([string]$l) {
    if (-not $l) { return 'powershell (sin orden)' }
    foreach ($p in @(
            @('GUARDIANA_FIRMAS', 'firmas (en tanda)'), @('Get-AuthenticodeSignature', 'firmas'),
            @('$v4 = @(', 'dns: ¿solo GUARDIANA?'),
            @('$out = @()', 'dns: copia (snapshot)'), @('Get-NetAdapter -InterfaceIndex', 'dns: DHCP'),
            @('Set-DnsClientServerAddress', 'dns: cambiar o devolver'), @('ExpandProperty ServerAddresses', 'dns: resolutores del sistema'),
            @('Get-NetRoute', 'puerta de enlace'), @('Win32_ComputerSystem', 'carpeta de datos: cuenta'),
            @('ReadToEnd', 'mirada lenta (vigilancia.ps1 o defensa.ps1)'), @('TimeZoneInfo', 'zona horaria'))) {
        if ($l.Contains($p[0])) { return $p[1] }
    }
    return 'otro: ' + $l.Substring(0, [math]::Min(90, $l.Length))
}
# Del registro de seguridad: cada proceso que arrancó el servicio, con su orden.
$porQuien = [ordered]@{}
$porQuienArranque = [ordered]@{}
$sucesos = @()
try { $sucesos = @(Get-WinEvent -FilterHashtable @{ LogName = 'Security'; Id = 4688; StartTime = $horaArranque } -ErrorAction Stop) } catch { Write-Host "::warning title=reposo $Etiqueta::sin el registro de seguridad: $_" }
foreach ($ev in $sucesos) {
    $padre = [int64]$ev.Properties[7].Value
    if ($padre -ne $servicio) { continue }
    # (no `$exe`: PowerShell no distingue mayúsculas en los nombres, y `$Exe` es el programa de GUARDIANA)
    $programa = Split-Path -Leaf ([string]$ev.Properties[5].Value)
    $q = if ($programa -eq 'powershell.exe') { QuienPidio ([string]$ev.Properties[8].Value) } else { $programa }
    if ($ev.TimeCreated -ge $horaVentana -and $ev.TimeCreated -le $horaFin) { $porQuien[$q] = 1 + [int]$porQuien[$q] }
    elseif ($ev.TimeCreated -lt $horaVentana) { $porQuienArranque[$q] = 1 + [int]$porQuienArranque[$q] }
}

& $Exe dns 2>&1 | ForEach-Object { Write-Host "dns: $_" }
& $Exe dns --restore 2>&1 | ForEach-Object { Write-Host "dns --restore: $_" }
& $Exe service uninstall 2>&1 | ForEach-Object { Write-Host "service uninstall: $_" }
Get-DnsClientServerAddress | Where-Object { $_.ServerAddresses } | ForEach-Object { Write-Host ("  {0} {1} {2}" -f $_.InterfaceAlias, $_.AddressFamily, ($_.ServerAddresses -join ',')) }
$lentas | ForEach-Object { Write-Host "lenta: $_" }

$ordenados = @($tiempos | Sort-Object)
function Percentil($p) { if ($ordenados.Count -eq 0) { return $null }; return [math]::Round($ordenados[[int][math]::Min($ordenados.Count - 1, [math]::Floor($ordenados.Count * $p))], 1) }
$min = $segundos / 60.0
$r = [ordered]@{
    etiqueta = $Etiqueta
    dns_aplicado = $aplicado
    en_trabajo = $enTrabajo
    minutos = [math]::Round($min, 2)
    servicio_y_lo_que_arranca_cpu_ms_min = [math]::Round(($t1[0] - $t0[0]) / $min, 0)
    guardiana_cpu_ms_min = [math]::Round(($g1 - $g0) / $min, 0)
    procesos_min = [math]::Round(($t1[1] - $t0[1]) / $min, 2)
    por_quien = $porQuien
    por_quien_al_arrancar = $porQuienArranque
    segundos_al_arrancar = [math]::Round(($horaVentana - $horaArranque).TotalSeconds, 0)
    equipo_cpu_ms_min = [math]::Round(($e1 - $e0) / $min, 0)
    wmiprvse_cpu_ms_min = [math]::Round(($w1 - $w0) / $min, 0)
    msmpeng_cpu_ms_min = [math]::Round(($d1 - $d0) / $min, 0)
    memoria_pico_mb = [math]::Round($t1[3], 0)
    preguntas = $tiempos.Count
    perdidas = $perdidas
    p50_ms = Percentil 0.5
    p99_ms = Percentil 0.99
    max_ms = if ($ordenados.Count) { [math]::Round($ordenados[-1], 1) } else { $null }
    mas_de_20_ms = $lentas.Count
}
$r | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $Salida -Encoding UTF8
$r | ConvertTo-Json -Depth 4 | Write-Host
$quien = ($porQuien.GetEnumerator() | ForEach-Object { "$($_.Key) $($_.Value)" }) -join '; '
$quienArranque = ($porQuienArranque.GetEnumerator() | ForEach-Object { "$($_.Key) $($_.Value)" }) -join '; '
Write-Host ("::notice title=procesos {0}::en la ventana, lo que arrancó el servicio y quién lo pidió: {1}" -f $Etiqueta, $quien)
Write-Host ("::notice title=procesos al arrancar {0}::en los {1} s desde que arranca hasta la ventana (con el dns --apply): {2}" -f $Etiqueta, $r.segundos_al_arrancar, $quienArranque)
Write-Host ("::notice title=reposo {0}::{1} min con el DNS {2}: servicio y lo que arranca {3} ms de procesador por minuto (guardiana.exe {4}), {5} procesos por minuto (con sus conhost); equipo entero {6} ms/min, WmiPrvSE {7} ms/min, Defender {8} ms/min; preguntas a 127.0.0.1: mediana {9} ms, p99 {10} ms, máximo {11} ms, {12} de más de 20 ms, {13} sin respuesta de {14}" -f $Etiqueta, $r.minutos, $(if ($aplicado) { 'apuntado a GUARDIANA' } else { 'SIN apuntar' }), $r.servicio_y_lo_que_arranca_cpu_ms_min, $r.guardiana_cpu_ms_min, $r.procesos_min, $r.equipo_cpu_ms_min, $r.wmiprvse_cpu_ms_min, $r.msmpeng_cpu_ms_min, $r.p50_ms, $r.p99_ms, $r.max_ms, $r.mas_de_20_ms, $r.perdidas, ($tiempos.Count + $perdidas))
if ($env:GITHUB_STEP_SUMMARY) {
    @("### Servicio en reposo: $Etiqueta", '', '```json', ($r | ConvertTo-Json -Depth 4), '```') -join "`n" | Add-Content -LiteralPath $env:GITHUB_STEP_SUMMARY -Encoding UTF8
}
if (-not $aplicado) { exit 1 }
exit 0
