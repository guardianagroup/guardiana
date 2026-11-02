# GUARDIANA, «ligero» (1.0.55): lo que gasta el servicio cuando nadie le pide nada, en un Windows de verdad.
# El motor (`guardiana observe`, el mismo que corre el servicio, con su panel y sus vigilancias) corre dentro de un
# objeto de trabajo, con el DNS del equipo apuntado a él como lo deja el instalador (`dns --apply --yes`). Tras un
# rato para que arranque todo, se cuenta durante $Minutos minutos: el procesador del motor y de todo lo que arranca, los
# procesos que arranca (por nombre), lo que gasta el equipo entero, y lo que tardan en contestar las preguntas de un
# nombre ya guardado, una por segundo, directamente a 127.0.0.1:53 (un parón del resolutor se ve ahí). Al final el DNS
# vuelve a como estaba y el motor se para. Lo usa .github/workflows/ligero.yml, en una máquina que se tira después.
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Etiqueta,
    [int]$Minutos = 10,
    [int]$Calentar = 120,
    [string]$Salida = "$env:RUNNER_TEMP\reposo.json"
)
$ErrorActionPreference = 'Stop'
Add-Type -Path (Join-Path $PSScriptRoot 'Medidor.cs')

$log = Join-Path $env:RUNNER_TEMP "observe-$Etiqueta.log"
$trabajo = [Medidor]::Trabajo()
$motor = [Medidor]::Arranca($trabajo, "cmd.exe /c """"$Exe"" observe > ""$log"" 2>&1""")
Write-Host "motor en marcha (cmd $motor)"

# Que conteste en 127.0.0.1:53 antes de apuntar nada a él.
$listo = $false
for ($i = 0; $i -lt 90 -and -not $listo; $i++) {
    Start-Sleep -Seconds 1
    try { $null = Resolve-DnsName -Name 'www.microsoft.com' -Server 127.0.0.1 -DnsOnly -QuickTimeout -ErrorAction Stop; $listo = $true } catch {}
}
if (-not $listo) { Get-Content -LiteralPath $log -ErrorAction SilentlyContinue | Select-Object -Last 40; throw 'el motor no contesta en 127.0.0.1:53' }

& $Exe dns --apply --yes 2>&1 | ForEach-Object { Write-Host "dns --apply: $_" }
$aplicado = $LASTEXITCODE -eq 0
Write-Host "dns --apply salió con $LASTEXITCODE"
Get-DnsClientServerAddress | Where-Object { $_.ServerAddresses } | ForEach-Object { Write-Host ("  {0} {1} {2}" -f $_.InterfaceAlias, $_.AddressFamily, ($_.ServerAddresses -join ',')) }

Start-Sleep -Seconds $Calentar

function Cpu($nombre) {
    $t = 0.0
    Get-Process -Name $nombre -ErrorAction SilentlyContinue | ForEach-Object { try { $t += $_.TotalProcessorTime.TotalMilliseconds } catch {} }
    return $t
}

# Los procesos que arranquen en la ventana, con su padre, para contar los del motor por nombre.
$arranques = [System.Collections.Concurrent.ConcurrentQueue[object]]::new()
Register-CimIndicationEvent -ClassName Win32_ProcessStartTrace -SourceIdentifier ligero -Action {
    $e = $Event.SourceEventArgs.NewEvent
    $Event.MessageData.Enqueue([pscustomobject]@{ pid = [int]$e.ProcessID; padre = [int]$e.ParentProcessID; nombre = [string]$e.ProcessName })
} -MessageData $arranques | Out-Null

$motores = @(Get-CimInstance Win32_Process -Filter "ParentProcessId = $motor" | Where-Object { $_.Name -eq 'guardiana.exe' } | ForEach-Object { [int]$_.ProcessId })
$t0 = [Medidor]::Lleva($trabajo); $e0 = [Medidor]::EquipoMs(); $w0 = Cpu 'WmiPrvSE'; $d0 = Cpu 'MsMpEng'
$g0 = 0.0; foreach ($p in $motores) { $g0 += (Get-Process -Id $p).TotalProcessorTime.TotalMilliseconds }
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
$perdidas = 0
$id = [uint16]1
$fin = [DateTime]::UtcNow.AddMinutes($Minutos)
while ([DateTime]::UtcNow -lt $fin) {
    $p = Pregunta $id
    $id = [uint16](($id + 1) % 65535 + 1)
    $c = [System.Diagnostics.Stopwatch]::StartNew()
    try {
        [void]$udp.Send($p, $p.Length)
        $remoto = [System.Net.IPEndPoint]::new([System.Net.IPAddress]::Any, 0)
        [void]$udp.Receive([ref]$remoto)
        $tiempos.Add($c.Elapsed.TotalMilliseconds)
    } catch { $perdidas++ }
    $resto = 1000 - [int]$c.Elapsed.TotalMilliseconds
    if ($resto -gt 0) { Start-Sleep -Milliseconds $resto }
}
$segundos = $reloj.Elapsed.TotalSeconds
$t1 = [Medidor]::Lleva($trabajo); $e1 = [Medidor]::EquipoMs(); $w1 = Cpu 'WmiPrvSE'; $d1 = Cpu 'MsMpEng'
$g1 = 0.0; foreach ($p in $motores) { $g1 += (Get-Process -Id $p).TotalProcessorTime.TotalMilliseconds }
Unregister-Event -SourceIdentifier ligero
$udp.Close()

# Los procesos del motor: hijos (y nietos) de guardiana.exe.
$todos = @($arranques.ToArray())
$suyos = [System.Collections.Generic.HashSet[int]]::new()
foreach ($p in $motores) { [void]$suyos.Add($p) }
$cambio = $true
while ($cambio) { $cambio = $false; foreach ($a in $todos) { if ($suyos.Contains($a.padre) -and $suyos.Add($a.pid)) { $cambio = $true } } }
$porNombre = [ordered]@{}
foreach ($a in ($todos | Where-Object { $suyos.Contains($_.padre) })) { $porNombre[$a.nombre] = 1 + [int]$porNombre[$a.nombre] }

& $Exe dns --restore 2>&1 | ForEach-Object { Write-Host "dns --restore: $_" }
Get-Content -LiteralPath $log -ErrorAction SilentlyContinue | Select-Object -First 30 | ForEach-Object { Write-Host "observe: $_" }

$ordenados = @($tiempos | Sort-Object)
function Percentil($p) { if ($ordenados.Count -eq 0) { return $null }; return [math]::Round($ordenados[[int][math]::Min($ordenados.Count - 1, [math]::Floor($ordenados.Count * $p))], 1) }
$min = $segundos / 60.0
$r = [ordered]@{
    etiqueta = $Etiqueta
    dns_aplicado = $aplicado
    minutos = [math]::Round($min, 2)
    motor_cpu_ms_min = [math]::Round(($t1[0] - $t0[0]) / $min, 0)
    guardiana_cpu_ms_min = [math]::Round(($g1 - $g0) / $min, 0)
    procesos_min = [math]::Round(($t1[1] - $t0[1]) / $min, 2)
    procesos_por_nombre = $porNombre
    equipo_cpu_ms_min = [math]::Round(($e1 - $e0) / $min, 0)
    wmiprvse_cpu_ms_min = [math]::Round(($w1 - $w0) / $min, 0)
    msmpeng_cpu_ms_min = [math]::Round(($d1 - $d0) / $min, 0)
    memoria_pico_mb = [math]::Round($t1[3], 0)
    preguntas = $tiempos.Count
    perdidas = $perdidas
    p50_ms = Percentil 0.5
    p99_ms = Percentil 0.99
    max_ms = if ($ordenados.Count) { [math]::Round($ordenados[-1], 1) } else { $null }
    mas_de_20_ms = @($tiempos | Where-Object { $_ -gt 20 }).Count
}
$r | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $Salida -Encoding UTF8
$r | ConvertTo-Json -Depth 4 | Write-Host
$nombres = ($porNombre.GetEnumerator() | ForEach-Object { "$($_.Key) $($_.Value)" }) -join ', '
Write-Host ("::notice title=reposo {0}::{1} min con el DNS {2}: motor y lo que arranca {3} ms de procesador por minuto (guardiana.exe {4}), {5} procesos por minuto ({6}); equipo entero {7} ms/min, WmiPrvSE {8} ms/min, Defender {9} ms/min; preguntas a 127.0.0.1: mediana {10} ms, p99 {11} ms, máximo {12} ms, {13} de más de 20 ms, {14} sin respuesta de {15}" -f $Etiqueta, $r.minutos, $(if ($aplicado) { 'apuntado a GUARDIANA' } else { 'SIN apuntar' }), $r.motor_cpu_ms_min, $r.guardiana_cpu_ms_min, $r.procesos_min, $nombres, $r.equipo_cpu_ms_min, $r.wmiprvse_cpu_ms_min, $r.msmpeng_cpu_ms_min, $r.p50_ms, $r.p99_ms, $r.max_ms, $r.mas_de_20_ms, $r.perdidas, ($tiempos.Count + $perdidas))
if ($env:GITHUB_STEP_SUMMARY) {
    @("### Servicio en reposo: $Etiqueta", '', '```json', ($r | ConvertTo-Json -Depth 4), '```') -join "`n" | Add-Content -LiteralPath $env:GITHUB_STEP_SUMMARY -Encoding UTF8
}
if (-not $aplicado) { exit 1 }
