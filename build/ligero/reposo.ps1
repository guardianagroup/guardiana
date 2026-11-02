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

# Los procesos que arranque el servicio, desde que arranca: sus hijos directos, por nombre, con la hora.
$arranques = [System.Collections.Concurrent.ConcurrentQueue[object]]::new()
$desde = [System.Diagnostics.Stopwatch]::StartNew()
Register-CimIndicationEvent -ClassName Win32_ProcessStartTrace -SourceIdentifier ligero -Action {
    $e = $Event.SourceEventArgs.NewEvent
    $Event.MessageData[0].Enqueue([pscustomobject]@{ pid = [int]$e.ProcessID; padre = [int]$e.ParentProcessID; nombre = [string]$e.ProcessName; s = $Event.MessageData[1].Elapsed.TotalSeconds })
} -MessageData @($arranques, $desde) | Out-Null

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

$inicioVentana = $desde.Elapsed.TotalSeconds
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
$t1 = if ($enTrabajo) { [Medidor]::Lleva($trabajo) } else { @(0, 0, 0, 0, 0) }
$e1 = [Medidor]::EquipoMs(); $w1 = Cpu 'WmiPrvSE'; $d1 = Cpu 'MsMpEng'
$g1 = (Get-Process -Id $servicio).TotalProcessorTime.TotalMilliseconds
Unregister-Event -SourceIdentifier ligero
$udp.Close()

$porNombre = [ordered]@{}
$alArrancar = [ordered]@{}
foreach ($a in @($arranques.ToArray() | Where-Object { $_.padre -eq $servicio })) {
    if ($a.s -ge $inicioVentana) { $porNombre[$a.nombre] = 1 + [int]$porNombre[$a.nombre] }
    else { $alArrancar[$a.nombre] = 1 + [int]$alArrancar[$a.nombre] }
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
    hijos_por_nombre = $porNombre
    hijos_al_arrancar = $alArrancar
    segundos_al_arrancar = [math]::Round($inicioVentana, 0)
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
$nombres = ($porNombre.GetEnumerator() | ForEach-Object { "$($_.Key) $($_.Value)" }) -join ', '
$nombresArranque = ($alArrancar.GetEnumerator() | ForEach-Object { "$($_.Key) $($_.Value)" }) -join ', '
Write-Host ("::notice title=arranque {0}::en los {1} s desde que arranca el servicio (con el dns --apply): {2}" -f $Etiqueta, $r.segundos_al_arrancar, $nombresArranque)
Write-Host ("::notice title=reposo {0}::{1} min con el DNS {2}: servicio y lo que arranca {3} ms de procesador por minuto (guardiana.exe {4}), {5} procesos por minuto (hijos directos en la ventana: {6}); equipo entero {7} ms/min, WmiPrvSE {8} ms/min, Defender {9} ms/min; preguntas a 127.0.0.1: mediana {10} ms, p99 {11} ms, máximo {12} ms, {13} de más de 20 ms, {14} sin respuesta de {15}" -f $Etiqueta, $r.minutos, $(if ($aplicado) { 'apuntado a GUARDIANA' } else { 'SIN apuntar' }), $r.servicio_y_lo_que_arranca_cpu_ms_min, $r.guardiana_cpu_ms_min, $r.procesos_min, $nombres, $r.equipo_cpu_ms_min, $r.wmiprvse_cpu_ms_min, $r.msmpeng_cpu_ms_min, $r.p50_ms, $r.p99_ms, $r.max_ms, $r.mas_de_20_ms, $r.perdidas, ($tiempos.Count + $perdidas))
if ($env:GITHUB_STEP_SUMMARY) {
    @("### Servicio en reposo: $Etiqueta", '', '```json', ($r | ConvertTo-Json -Depth 4), '```') -join "`n" | Add-Content -LiteralPath $env:GITHUB_STEP_SUMMARY -Encoding UTF8
}
if (-not $aplicado) { exit 1 }
