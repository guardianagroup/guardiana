# GUARDIANA · el vigilante del DNS (30 Oct 2026).
#
# El dueño: «que cuando la persona salga, no le pase como a mi amigo, que tenga que volver a reconfigurar el DNS… Tiene que
# ser todo automático y que al cliente siempre tenga el internet». Mientras GUARDIANA está instalada, el DNS del equipo apunta a
# ella (127.0.0.1 y ::1). Si el servicio se para bien, devuelve él mismo el DNS de antes. Pero si lo matan, se cuelga, el
# antivirus lo pone en cuarentena o alguien borra la carpeta del programa, nadie contestaba en 127.0.0.1 y el equipo se quedaba
# sin internet hasta que alguien cambiaba el DNS a mano (lo que le pasó al amigo del dueño en Chequia con la 1.0.46).
#
# Este guion no depende del programa: lo arranca el Programador de tareas de Windows como SYSTEM al encender el equipo y cada
# minuto (tarea «\GUARDIANA\Vigilante», vigilante.xml). En cada pasada mira durante ~55 s, cada 5 s:
#   1. ¿alguna conexión del equipo tiene 127.0.0.1 o ::1 como DNS? Si no, no hay nada que vigilar.
#   2. ¿contesta alguien de verdad en 127.0.0.1:53? Una pregunta DNS por UDP de un nombre de prueba que solo GUARDIANA conoce
#      (*.prueba.guardiana.hogar). Cualquier respuesta vale: el resolutor está vivo aunque no haya internet.
#   3. Tres fallos seguidos (unos 15 s): devuelve el DNS de antes de GUARDIANA, exactamente como lo dejó la copia
#      (dns-anterior.json, junto al extracto); sin copia, «Automático (DHCP)», que es lo que tiene casi todo el mundo.
# Cuando el servicio vuelve a contestar, él mismo vuelve a apuntar el equipo a GUARDIANA (cada minuto, solo si su resolutor
# responde: engine.rs). El vigilante nunca apunta el DNS a ningún sitio: solo lo devuelve.
#
# Si GUARDIANA ya no está (desinstalada a medias) y el DNS es el normal, el vigilante se quita a sí mismo.
$ErrorActionPreference = 'SilentlyContinue'
$dir = Join-Path $env:ProgramData 'Guardiana'
$registro = Join-Path $dir 'vigilante.log'

function Anota([string]$texto) {
    try {
        if ((Test-Path $registro) -and (Get-Item $registro).Length -gt 262144) { Move-Item $registro ($registro + '.1') -Force }
        Add-Content -Path $registro -Value ('{0:yyyy-MM-dd HH:mm:ss} {1}' -f (Get-Date), $texto) -Encoding UTF8
    } catch {}
}

# Las conexiones cuyo DNS es GUARDIANA (127.0.0.1 o ::1)
function ApuntanAGuardiana {
    $r = @()
    foreach ($d in @(Get-DnsClientServerAddress)) {
        $s = @($d.ServerAddresses)
        if ($s -contains '127.0.0.1' -or $s -contains '::1') { $r += [int]$d.InterfaceIndex }
    }
    return @($r | Sort-Object -Unique)
}

# Una pregunta DNS de verdad a 127.0.0.1:53 por UDP, con 2 s de espera
function Contesta {
    $u = $null
    try {
        $u = New-Object System.Net.Sockets.UdpClient
        $u.Client.ReceiveTimeout = 2000
        $id = Get-Random -Minimum 1 -Maximum 65535
        $nombre = 'v' + [DateTime]::UtcNow.Ticks + '.prueba.guardiana.hogar'
        $q = New-Object 'System.Collections.Generic.List[byte]'
        $q.AddRange([byte[]]@([byte]($id -shr 8), [byte]($id -band 255), 1, 0, 0, 1, 0, 0, 0, 0, 0, 0))
        foreach ($p in $nombre.Split('.')) { $b = [Text.Encoding]::ASCII.GetBytes($p); $q.Add([byte]$b.Length); $q.AddRange($b) }
        $q.AddRange([byte[]]@(0, 0, 1, 0, 1))
        $a = $q.ToArray()
        [void]$u.Send($a, $a.Length, '127.0.0.1', 53)
        $ep = New-Object System.Net.IPEndPoint([System.Net.IPAddress]::Any, 0)
        $r = $u.Receive([ref]$ep)
        return ($r.Length -ge 12 -and $r[0] -eq $a[0] -and $r[1] -eq $a[1])
    } catch {
        return $false
    } finally {
        if ($u) { try { $u.Close() } catch {} }
    }
}

# Devuelve el DNS de antes en esas conexiones
function Devuelve([int[]]$indices) {
    $copia = $null
    $f = Join-Path $dir 'dns-anterior.json'
    if (Test-Path $f) { try { $copia = Get-Content $f -Raw -Encoding UTF8 | ConvertFrom-Json } catch { $copia = $null } }
    foreach ($id in $indices) {
        $manual = @()
        $c = $null
        if ($copia -and $copia.interfaces) { $c = @($copia.interfaces | Where-Object { [string]$_.id -eq [string]$id })[0] }
        if ($c) {
            if ($c.extra) {
                try { $x = $c.extra | ConvertFrom-Json; $manual = @(@($x.manual4) + @($x.manual6) | Where-Object { $_ }) } catch { $manual = @() }
            } elseif (-not $c.automatic) {
                $manual = @(@($c.servers) | Where-Object { $_ -and $_ -ne '127.0.0.1' -and $_ -ne '::1' })
            }
        }
        Set-DnsClientServerAddress -InterfaceIndex $id -ResetServerAddresses
        if ($manual.Count -gt 0) { Set-DnsClientServerAddress -InterfaceIndex $id -ServerAddresses $manual }
        $d6 = Get-DnsClientServerAddress -InterfaceIndex $id -AddressFamily IPv6
        if ($d6 -and (@($d6.ServerAddresses) -contains '::1')) { & netsh.exe interface ipv6 set dnsservers name=$id source=dhcp | Out-Null }
        Anota ("devuelto el DNS de la conexión $id" + $(if ($manual.Count -gt 0) { ' (los de antes: ' + ($manual -join ', ') + ')' } else { ' (automático)' }))
    }
    Clear-DnsClientCache
}

# GUARDIANA ya no está y el DNS es el normal: el vigilante sobra
if (-not (Get-Service -Name 'guardiana' -ErrorAction SilentlyContinue) -and (ApuntanAGuardiana).Count -eq 0) {
    Anota 'GUARDIANA no está instalada y el DNS es el normal: el vigilante se quita'
    & schtasks.exe /Delete /TN '\GUARDIANA\Vigilante' /F | Out-Null
    Remove-Item -LiteralPath $PSCommandPath -Force
    exit 0
}

$fallos = 0
$hasta = (Get-Date).AddSeconds(55)
while ((Get-Date) -lt $hasta) {
    $ind = ApuntanAGuardiana
    if ($ind.Count -eq 0) { $fallos = 0 }
    elseif (Contesta) { $fallos = 0 }
    else {
        $fallos++
        if ($fallos -ge 3) {
            Anota ('GUARDIANA no contesta en 127.0.0.1 (' + $fallos + ' veces seguidas): se devuelve el DNS para que el equipo no se quede sin internet')
            Devuelve $ind
            $fallos = 0
        }
    }
    Start-Sleep -Seconds 3
}
exit 0
