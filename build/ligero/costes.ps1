# GUARDIANA, «ligero» (1.0.55). El dueño, 2 nov 2026, 16:53: «se me traba», sobre todo en los equipos sencillos.
# Lo que cuesta, en un Windows de verdad (el que GitHub presta para la prueba), cada orden que el servicio lanza solo:
# una vez para calentar y luego $Veces veces, contando dentro de un objeto de trabajo el procesador, los procesos, la
# memoria y el disco de la orden y de todo lo que arranca, y aparte lo que gastó el equipo entero mientras tanto (el
# trabajo que una orden deja a WmiPrvSE o a Defender), menos lo que el equipo gasta sin hacer nada.
# Solo lectura, salvo una clave de prueba en el registro y una carpeta temporal, que se quitan al terminar.
# Escribe la tabla en el resumen de la ejecución y en un JSON. Lo usa .github/workflows/ligero.yml.
param(
    [int]$Veces = 5,
    [string]$Salida = "$env:RUNNER_TEMP\costes.json"
)
$ErrorActionPreference = 'Stop'
$raiz = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Add-Type -Path (Join-Path $PSScriptRoot 'Medidor.cs')

# Los guiones tal como van dentro del programa: leídos de su fuente, no copiados.
$fuente = Get-Content -Raw -LiteralPath (Join-Path $raiz 'crates\service\src\sysdns\windows.rs')
function Guion($nombre) {
    $m = [regex]::Match($fuente, "const $nombre`: &str = r#""(.*?)""#;", 'Singleline')
    if (-not $m.Success) { throw "no encuentro $nombre en windows.rs" }
    return $m.Groups[1].Value
}
$primario = Guion 'PRIMARY_SCRIPT'
$foto = Guion 'SNAPSHOT_SCRIPT'
$vigilancia = Get-Content -Raw -LiteralPath (Join-Path $raiz 'crates\apps\src\vigilancia.ps1')
$defensa = Get-Content -Raw -LiteralPath (Join-Path $raiz 'crates\apps\src\vigia\defensa.ps1')

# Las interfaces conectadas del equipo de prueba, para la orden de cada minuto que lee el DHCP.
$indices = @(Get-NetIPInterface -AddressFamily IPv4 | Where-Object { $_.ConnectionState -eq 'Connected' -and $_.InterfaceAlias -notlike 'Loopback*' } | ForEach-Object { $_.InterfaceIndex })
$dhcp = ($indices | ForEach-Object { "`$g = (Get-NetAdapter -InterfaceIndex $_ -ErrorAction SilentlyContinue).InterfaceGuid; if (`$g) { '$_|' + [string](Get-ItemProperty -Path ""HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\`$g"" -Name DhcpNameServer -ErrorAction SilentlyContinue).DhcpNameServer }" }) -join '; '

# Las órdenes de crates/service/src/sysdns/mod.rs y crates/core/src/paths.rs, como las escribe el programa.
$resolutores = "Get-NetIPInterface -AddressFamily IPv4 | Where-Object { `$_.ConnectionState -eq 'Connected' -and `$_.InterfaceAlias -notlike 'Loopback*' } | ForEach-Object { Get-DnsClientServerAddress -InterfaceIndex `$_.InterfaceIndex -AddressFamily IPv4 } | Select-Object -ExpandProperty ServerAddresses"
$puerta = "Get-NetRoute -AddressFamily IPv4 -DestinationPrefix '0.0.0.0/0' -PolicyStore ActiveStore -ErrorAction SilentlyContinue | ForEach-Object { `$m = `$_.RouteMetric + (Get-NetIPInterface -InterfaceIndex `$_.InterfaceIndex -AddressFamily IPv4 -ErrorAction SilentlyContinue).InterfaceMetric; [pscustomobject]@{ M = `$m; H = `$_.NextHop } } | Sort-Object M | Select-Object -ExpandProperty H"
$cuenta = "`$u = (Get-CimInstance Win32_ComputerSystem).UserName; if (`$u) { (New-Object System.Security.Principal.NTAccount(`$u)).Translate([System.Security.Principal.SecurityIdentifier]).Value }"

$ps = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
function Ps($guion) {
    # -EncodedCommand solo aquí, para que el guion llegue entero sin pelearse con las comillas; cuesta lo mismo.
    $b = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($guion))
    return """$ps"" -NoProfile -NonInteractive -EncodedCommand $b"
}
# Los dos guiones largos, como los pasa el programa (un bloque leído entero), desde un archivo: por la línea de órdenes
# no caben con holgura.
function PsArchivo($id, $guion) {
    $f = Join-Path $env:RUNNER_TEMP "ligero-$id.ps1"
    Set-Content -LiteralPath $f -Value $guion -Encoding UTF8
    return """$ps"" -NoProfile -NonInteractive -Command ""& ([scriptblock]::Create([IO.File]::ReadAllText('$f')))"""
}

# La clave del ancla de la licencia, con valores como los que escribe el programa, para medir «reg query».
$clave = 'HKLM:\SOFTWARE\Guardiana'
$habia = Test-Path $clave
if (-not $habia) {
    New-Item -Path $clave -Force | Out-Null
    New-ItemProperty -Path $clave -Name prueba -Value '1793750400000' -PropertyType String -Force | Out-Null
    New-ItemProperty -Path $clave -Name visto -Value '1793750400000' -PropertyType String -Force | Out-Null
}
$carpeta = Join-Path $env:RUNNER_TEMP 'ligero-icacls'
New-Item -ItemType Directory -Force -Path $carpeta | Out-Null

$BAJA = 0x4000 # BELOW_NORMAL_PRIORITY_CLASS, la de las dos miradas lentas
$ordenes = @(
    @{ id = 'cmd'; que = 'Un proceso vacío (cmd /c exit), el suelo'; cada = '-'; linea = "cmd.exe /c exit 0"; prio = 0 },
    @{ id = 'ps_vacio'; que = 'PowerShell sin hacer nada (exit)'; cada = '-'; linea = (Ps 'exit 0'); prio = 0 },
    @{ id = 'reg'; que = 'Ancla de la licencia: reg query (license::status)'; cada = 'cada 30 s con consultas, y 2 por minuto'; linea = "reg.exe query HKLM\SOFTWARE\Guardiana"; prio = 0 },
    @{ id = 'primario'; que = 'DNS: guardian_is_primary (PRIMARY_SCRIPT)'; cada = 'cada minuto'; linea = (Ps $primario); prio = 0 },
    @{ id = 'dhcp'; que = 'DNS: upstreams_for (DhcpNameServer)'; cada = 'cada minuto'; linea = (Ps $dhcp); prio = 0 },
    @{ id = 'foto'; que = 'DNS: snapshot (SNAPSHOT_SCRIPT)'; cada = 'al aplicar o reapuntar'; linea = (Ps $foto); prio = 0 },
    @{ id = 'resolutores'; que = 'DNS: current_resolvers'; cada = 'al empezar cada vuelta'; linea = (Ps $resolutores); prio = 0 },
    @{ id = 'puerta'; que = 'Modo Hogar: default_gateway'; cada = 'cada 5 min con Modo Hogar'; linea = (Ps $puerta); prio = 0 },
    @{ id = 'cuenta'; que = 'Carpeta de datos: la cuenta de la consola (harden_data_dir)'; cada = 'cada 10 min'; linea = (Ps $cuenta); prio = 0 },
    @{ id = 'icacls'; que = 'Carpeta de datos: un icacls (van 3)'; cada = 'cada 10 min'; linea = "icacls.exe ""$carpeta"" /inheritance:r /grant:r *S-1-5-18:(OI)(CI)F *S-1-5-32-544:(OI)(CI)F"; prio = 0 },
    @{ id = 'vigilancia'; que = 'Vigilancia, mirada lenta (vigilancia.ps1)'; cada = 'cada 5 min'; linea = (PsArchivo 'vigilancia' $vigilancia); prio = $BAJA },
    @{ id = 'defensa'; que = 'Vigía, Defender (defensa.ps1)'; cada = 'cada 10 min'; linea = (PsArchivo 'defensa' $defensa); prio = $BAJA }
)

# Lo que el equipo gasta sin hacer nada, en ms de procesador por segundo de reloj.
$e0 = [Medidor]::EquipoMs(); Start-Sleep -Seconds 10; $ruido = ([Medidor]::EquipoMs() - $e0) / 10.0

function Mediana($v) { $s = @($v | Sort-Object); return $s[[int][math]::Floor(($s.Count - 1) / 2)] }
$filas = @()
foreach ($o in $ordenes) {
    [void][Medidor]::Mide($o.linea, [uint32]$o.prio)
    $r = @()
    for ($i = 0; $i -lt $Veces; $i++) { $r += , ([Medidor]::Mide($o.linea, [uint32]$o.prio)); Start-Sleep -Milliseconds 300 }
    $fila = [ordered]@{
        id = $o.id; que = $o.que; cada = $o.cada
        reloj_ms = [math]::Round((Mediana ($r | ForEach-Object { $_[0] })), 0)
        cpu_ms = [math]::Round((Mediana ($r | ForEach-Object { $_[1] })), 0)
        cpu_min_ms = [math]::Round((($r | ForEach-Object { $_[1] }) | Measure-Object -Minimum).Minimum, 0)
        cpu_max_ms = [math]::Round((($r | ForEach-Object { $_[1] }) | Measure-Object -Maximum).Maximum, 0)
        procesos = [int](Mediana ($r | ForEach-Object { $_[2] }))
        leido_mb = [math]::Round((Mediana ($r | ForEach-Object { $_[3] })) / 1MB, 1)
        memoria_mb = [math]::Round((Mediana ($r | ForEach-Object { $_[4] })), 0)
        equipo_ms = [math]::Round((Mediana ($r | ForEach-Object { $_[5] - $ruido * $_[0] / 1000.0 })), 0)
        salida = [int](Mediana ($r | ForEach-Object { $_[6] }))
    }
    $filas += , $fila
    Write-Host ("{0,-12} reloj {1,6} ms  cpu {2,6} ms ({3}-{4})  equipo {5,6} ms  procesos {6}  memoria {7} MB  leído {8} MB  salida {9}" -f $fila.id, $fila.reloj_ms, $fila.cpu_ms, $fila.cpu_min_ms, $fila.cpu_max_ms, $fila.equipo_ms, $fila.procesos, $fila.memoria_mb, $fila.leido_mb, $fila.salida)
}

if (-not $habia) { Remove-Item -Path $clave -Recurse -Force }
Remove-Item -LiteralPath $carpeta -Recurse -Force -ErrorAction SilentlyContinue

$cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
$so = Get-CimInstance Win32_OperatingSystem
$equipo = "{0} ({1} núcleos lógicos), {2} GB, {3} {4}" -f $cpu.Name.Trim(), $cpu.NumberOfLogicalProcessors, [math]::Round($so.TotalVisibleMemorySize / 1MB, 1), $so.Caption, $so.Version
$res = [ordered]@{ equipo = $equipo; ruido_ms_por_s = [math]::Round($ruido, 1); veces = $Veces; filas = $filas }
$res | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $Salida -Encoding UTF8

$md = @("### Lo que cuesta cada orden (mediana de $Veces, tras una para calentar)", '', "Equipo: $equipo. Sin hacer nada gasta $([math]::Round($ruido,1)) ms de procesador por segundo, ya restado en «equipo».", '',
    '| Orden | Cada cuánto | Reloj (ms) | Procesador de la orden (ms, mín–máx) | Procesador del equipo entero (ms) | Procesos | Memoria pico (MB) |', '|---|---|---:|---:|---:|---:|---:|')
foreach ($f in $filas) { $md += "| $($f.que) | $($f.cada) | $($f.reloj_ms) | $($f.cpu_ms) ($($f.cpu_min_ms)–$($f.cpu_max_ms)) | $($f.equipo_ms) | $($f.procesos) | $($f.memoria_mb) |" }
if ($env:GITHUB_STEP_SUMMARY) { $md -join "`n" | Add-Content -LiteralPath $env:GITHUB_STEP_SUMMARY -Encoding UTF8 }
foreach ($f in $filas) { Write-Host ("::notice title=coste {0}::reloj {1} ms, procesador {2} ms ({3}-{4}), equipo {5} ms, {6} procesos, {7} MB" -f $f.id, $f.reloj_ms, $f.cpu_ms, $f.cpu_min_ms, $f.cpu_max_ms, $f.equipo_ms, $f.procesos, $f.memoria_mb) }
Write-Host "::notice title=equipo::$equipo; ruido $([math]::Round($ruido,1)) ms/s"
