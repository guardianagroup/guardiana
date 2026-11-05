# build/vpn/estricto_de_verdad.ps1: el modo estricto de la VPN con el servicio de verdad y los servidores de verdad
# (docs/VPN.md, «El modo estricto» y «Las salidas»), en un Windows de GitHub, como administrador. Lo lanza el trabajo
# «estricto_de_verdad» de .github/workflows/vpn.yml, que solo corre a mano:
#
#   pwsh -File build/vpn/estricto_de_verdad.ps1 -Exe target\debug\guardiana.exe [-Trabajo <carpeta>]
#
# (con la wireguard.dll firmada por WireGuard LLC junto a guardiana.exe). Hasta ahora el modo estricto solo se había
# probado con un servidor que no contesta (el trabajo «estricto») y la conexión de verdad solo en el modo normal (el
# trabajo «de_verdad»). Aquí, todo junto, por la API del panel (127.0.0.1:7443, con la ficha X-Guardiana-Token), como lo
# hace la página:
#
#  0. Antes, desde fuera del túnel: la dirección pública del equipo, su tarjeta de red, y de cada salida (hel, nbg, mia)
#     su dirección y si su alta contesta (`GET https://<salida>/vpn/alta`). Una salida cuya alta no contesta se omite en
#     el paso 2 («salida no disponible, se omite»), sin fallo.
#  1. El modo estricto encendido y Helsinki: el túnel arriba, el bloqueo en Windows, HTTPS a internet, y la dirección
#     pública que ve un eco es la de la salida (62.238.17.121) y no la del equipo; por la tarjeta, nada sale (WSAEACCES).
#     Y el bloqueo visto por fuera del programa, sitio por sitio: en `netsh wfp show state` y en lo que Windows guarda
#     para el próximo arranque (la política de BFE en el registro). `netsh wfp show filters` no lista los 4 persistentes
#     «todo lo demás» (gemelos exactos, sin condiciones y con peso efectivo 0, de los de arranque): se dice, y no cuenta.
#  2. Núremberg y Miami (si su alta contestó), cada una con su dirección, con el bloqueo puesto durante todo el cambio;
#     y vuelta a Helsinki.
#  3. El proceso del servicio, muerto (taskkill /F) con el túnel puesto: mientras está muerto (unos 12 s), nada sale
#     (WSAEACCES), ni hacia el HTTPS del servidor desde otro programa, y el bloqueo sigue. Luego, solo el bloqueo
#     persistente, como tras arrancar el equipo (los de arranque dejan de valer cuando el servicio de filtrado carga los
#     persistentes, y GUARDIANA aún no está): la prueba quita los de arranque y nada sale tampoco. Arrancado otra vez, el
#     servicio vuelve a poner el bloqueo entero, conecta solo y la dirección vuelve a ser la del servidor.
#  4. «Desconectar» (`POST /api/vpn/desconecta`): la red de siempre (la dirección del equipo) y nada de GUARDIANA en el
#     Filtering Platform (`netsh wfp show filters` y `show state`: ni filtros persistentes, ni de arranque, ni dinámicos,
#     ni su proveedor ni su subcapa) ni guardado para el próximo arranque.
#  5. Otra vez conectada, el servicio parado (el bloqueo se queda: nada sale) y la orden de emergencia, la de «Apagar la
#     VPN de GUARDIANA» del menú Inicio (`guardiana vpn apaga --menu`, con su Intro al final): la red vuelve y no queda
#     nada; y el servicio, arrancado otra vez, no vuelve a poner el bloqueo.
#
# Cada comprobación sale como «BIEN …» o «MAL …», con los segundos desde el principio; al final, la lista de las que
# fallaron (también en el resumen de la ejecución) y el código 1 si alguna falló.
#
# El equipo de pruebas habla con GitHub por la red: con el túnel puesto, por el túnel; sin túnel y con el bloqueo, no
# habla. Esos ratos son cortos (el más largo, el servicio muerto, unos 20 s), y un vigilante aparte
# (build/vpn/vigilante_vpn.ps1), con un plazo por fase, apaga la VPN, quita el bloqueo y para el servicio si una fase se
# pasa de su plazo. Al terminar, pase lo que pase, la VPN se apaga y el servicio se quita.
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [string]$Trabajo = (Join-Path ([System.IO.Path]::GetTempPath()) 'estricto-de-verdad')
)

$ErrorActionPreference = 'Continue'
$PSNativeCommandUseErrorActionPreference = $false
$ProgressPreference = 'SilentlyContinue'

$g = (Resolve-Path -LiteralPath $Exe).Path
$datos = Join-Path $env:ProgramData 'Guardiana'
$curl = Join-Path $env:SystemRoot 'System32\curl.exe'
New-Item -ItemType Directory -Force -Path $Trabajo | Out-Null
$plazoArchivo = Join-Path $Trabajo 'plazo.txt'
$finArchivo = Join-Path $Trabajo 'fin.txt'
$vigiaRegistro = Join-Path $Trabajo 'vigilante.log'
Remove-Item -LiteralPath $plazoArchivo, $finArchivo, $vigiaRegistro -Force -ErrorAction SilentlyContinue

# Las claves fijas del modo estricto en Windows (crates/tunel/src/estricto.rs, `claves`): el proveedor, la subcapa y el
# principio de las de sus filtros persistentes y de arranque.
$PROVEEDOR = '36694406-4925-4a21-9e16-0031b0e144b5'
$SUBCAPA = '50573eae-5843-4fc2-aefc-12aeda4a7908'
$PERSISTENTES = '329dccb1-2e23-4244-bf84-61d4a4b7'
$ARRANQUE = '53fb6f77-c067-462b-9aca-6a6337d8'
# Las salidas (crates/tunel/src/salidas.rs, `SALIDAS`); Helsinki, con la dirección que se conoce de ella.
$HELSINKI_IP = '62.238.17.121'
$SALIDAS = @(
    [pscustomobject]@{ Id = 'hel'; Ciudad = 'Helsinki'; Nombre = 'vpn.guardianagroup.com'; Ip = ''; Disponible = $false; Motivo = '' }
    [pscustomobject]@{ Id = 'nbg'; Ciudad = 'Núremberg'; Nombre = 'vpn-nbg.guardianagroup.com'; Ip = ''; Disponible = $false; Motivo = '' }
    [pscustomobject]@{ Id = 'mia'; Ciudad = 'Miami'; Nombre = 'vpn-mia.guardianagroup.com'; Ip = ''; Disponible = $false; Motivo = '' }
)
$hel = $SALIDAS[0]
# Servicios que contestan, por HTTPS, con la dirección que ven; vale el primero que contesta. El último, sin nombre que
# resolver.
$ECOS = @('https://api.ipify.org', 'https://checkip.amazonaws.com', 'https://1.1.1.1/cdn-cgi/trace')

$inicio = Get-Date
$script:fase = '0. Antes'
$script:equipo = ''
$script:tarjeta = $null
$script:panelFallo = ''
$script:nWfp = 0
$hechas = [System.Collections.Generic.List[object]]::new()
$omitidas = [System.Collections.Generic.List[string]]::new()

# ---- El registro: cada línea con los segundos desde el principio (si el equipo pierde a GitHub un rato, el registro
# sigue diciendo dónde estaba cada paso). Write-Host: lo escribe el anfitrión y no entra en lo que devuelven las
# funciones.

function Linea([string]$t) {
    Write-Host ('[{0,6:N1} s] {1}' -f ((Get-Date) - $inicio).TotalSeconds, $t)
}

# Una comprobación: BIEN solo si `$ok` es $true (un valor que no sea un booleano es un fallo de la prueba, y se ve).
function Bien($ok, [string]$que, [string]$detalle = '') {
    $si = ($ok -is [bool]) -and $ok
    $hechas.Add([pscustomobject]@{ Fase = $script:fase; Bien = $si; Que = $que })
    Linea ('{0} {1}' -f $(if ($si) { 'BIEN' } else { 'MAL ' }), $que)
    if ($detalle) { Linea "     $detalle" }
}

function Omite([string]$que) {
    $omitidas.Add($que)
    Linea "OMITIDA $que"
}

function Json($o) {
    if ($null -eq $o) { return 'null' }
    try { return ($o | ConvertTo-Json -Compress -Depth 8) } catch { return "$o" }
}

function Recorta([string]$t, [int]$n = 200) {
    $u = (($t -split '\s+') | Where-Object { $_ }) -join ' '
    if ($u.Length -gt $n) { return $u.Substring(0, $n) + '…' }
    return $u
}

# Hasta cuándo puede durar lo que viene, para el vigilante: escrito aparte y puesto en su sitio de una vez, para que
# nunca lea un número a medias.
function Plazo([int]$segundos) {
    $aparte = "$plazoArchivo.tmp"
    Set-Content -LiteralPath $aparte -Value ('{0}|{1}' -f [DateTime]::UtcNow.AddSeconds($segundos).Ticks, $script:fase) -Encoding utf8
    # si el vigilante lo está leyendo justo entonces, Windows no deja cambiarlo: otra vez, un momento después
    for ($i = 0; $i -lt 20; $i++) {
        try {
            Move-Item -LiteralPath $aparte -Destination $plazoArchivo -Force -ErrorAction Stop
            return
        } catch {
            Start-Sleep -Milliseconds 50
        }
    }
    Linea "     (no se pudo poner el plazo del vigilante: $plazoArchivo)"
}

function Fase([string]$nombre, [int]$segundos) {
    $script:fase = $nombre
    Write-Host ''
    Linea "=== $nombre ==="
    Plazo $segundos
}

function Espera([double]$segundos, [scriptblock]$si, [int]$cadaMs = 500) {
    $hasta = (Get-Date).AddSeconds($segundos)
    do {
        if ((& $si) -eq $true) { return $true }
        Start-Sleep -Milliseconds $cadaMs
    } while ((Get-Date) -lt $hasta)
    return $false
}

# ---- El panel del servicio, como lo llama la página.

function Llave {
    try { return (Get-Content -LiteralPath (Join-Path $datos 'panel.token') -Raw -ErrorAction Stop).Trim() } catch { return '' }
}

# Lo que contesta el panel, o $null (y por qué, en $script:panelFallo).
function Panel([string]$metodo, [string]$ruta, $cuerpo = $null, [int]$segundos = 30) {
    $p = @{
        Method = $metodo
        Uri = "http://127.0.0.1:7443$ruta"
        Headers = @{ 'X-Guardiana-Token' = (Llave); 'X-Guardiana-Lang' = 'es' }
        TimeoutSec = $segundos
        NoProxy = $true
    }
    if ($null -ne $cuerpo) {
        $p.Body = ($cuerpo | ConvertTo-Json -Compress)
        $p.ContentType = 'application/json'
    }
    try {
        return Invoke-RestMethod @p
    } catch {
        $script:panelFallo = $_.Exception.Message
        return $null
    }
}

# ---- Lo que dicen guardiana.exe y Windows.

# `guardiana vpn estado --json`: lo guardado y lo que Windows tiene del modo estricto.
function Estado {
    try { return ((& $g vpn estado --json 2>$null) -join '' | ConvertFrom-Json -ErrorAction Stop) } catch { return $null }
}

# Nada del modo estricto en Windows, y la VPN apagada en lo guardado.
function Limpio($e) {
    return ($null -ne $e -and $e.persistentes -eq 0 -and $e.arranque -eq 0 -and -not $e.objetos -and -not $e.encendida)
}

function Servicio {
    $s = Get-Service -Name guardiana -ErrorAction SilentlyContinue
    if ($s) { return "$($s.Status)" }
    return 'no está'
}

function ProcesoDelServicio {
    $s = Get-CimInstance Win32_Service -Filter "Name='guardiana'" -ErrorAction SilentlyContinue
    if ($s) { return [int]$s.ProcessId }
    return 0
}

# El estado del adaptador del túnel («GUARDIANA»), o '' si no está.
function Adaptador {
    $a = Get-NetAdapter -IncludeHidden -ErrorAction SilentlyContinue | Where-Object { $_.Name -eq 'GUARDIANA' } | Select-Object -First 1
    if ($a) { return "$($a.Status)" }
    return ''
}

function Rutas {
    $r = Get-NetRoute -AddressFamily IPv4 -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue | Sort-Object RouteMetric
    return (($r | ForEach-Object { '{0} vía {1} (métrica {2})' -f $_.InterfaceAlias, $_.NextHop, $_.RouteMetric }) -join '; ')
}

# La tarjeta de red por la que sale el equipo sin VPN: su nombre, su puerta de enlace y su dirección.
function Tarjeta {
    $r = Get-NetRoute -AddressFamily IPv4 -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue |
        Where-Object { $_.NextHop -ne '0.0.0.0' } | Sort-Object RouteMetric | Select-Object -First 1
    if (-not $r) { return $null }
    $ip = Get-NetIPAddress -AddressFamily IPv4 -InterfaceIndex $r.InterfaceIndex -ErrorAction SilentlyContinue | Select-Object -First 1
    return [pscustomobject]@{ Alias = $r.InterfaceAlias; Puerta = $r.NextHop; Ip = $(if ($ip) { $ip.IPAddress } else { '' }) }
}

# ---- La red, vista desde este equipo.

# Una conexión TCP a `$ip`:`$puerto` (desde la dirección `$desde`, si se da: así sale por esa tarjeta y no por el túnel):
# «conecta», «cortada» (Windows la rechazó antes de que saliera: WSAEACCES, 10013, el filtro), «sin respuesta» o el error.
function Tcp([string]$ip, [int]$puerto, [string]$desde = '', [int]$ms = 3000) {
    $s = [System.Net.Sockets.Socket]::new([System.Net.Sockets.AddressFamily]::InterNetwork, [System.Net.Sockets.SocketType]::Stream, [System.Net.Sockets.ProtocolType]::Tcp)
    try {
        if ($desde) { $s.Bind([System.Net.IPEndPoint]::new([System.Net.IPAddress]::Parse($desde), 0)) }
        $t = $s.ConnectAsync([System.Net.IPAddress]::Parse($ip), $puerto)
        if ($t.Wait($ms) -and $s.Connected) { return 'conecta' }
        return 'sin respuesta'
    } catch {
        $e = $_.Exception
        while ($e.InnerException) { $e = $e.InnerException }
        if ($e -is [System.Net.Sockets.SocketException]) {
            if ($e.ErrorCode -eq 10013) { return 'cortada' }
            return ('error {0} ({1})' -f $e.ErrorCode, $e.SocketErrorCode)
        }
        return $e.Message
    } finally {
        $s.Dispose()
    }
}

# Una petición HTTPS con el curl de Windows y el DNS del sistema: su código (0 si no hubo respuesta), la dirección a la
# que fue, el cuerpo y, sin respuesta, lo que dijo curl.
function Https([string]$url, [int]$max = 12, [string[]]$mas = @()) {
    $a = @('-sS', '--ssl-no-revoke', '--connect-timeout', '6', '--max-time', "$max", '--stderr', '-', '-w', '\n@@%{http_code}@@%{remote_ip}') + $mas + @($url)
    $salida = (& $curl @a) -join "`n"
    $m = [regex]::Match($salida, '@@(\d{3})@@([0-9A-Fa-f.:]*)\s*$')
    if (-not $m.Success) { return [pscustomobject]@{ Codigo = 0; Ip = ''; Cuerpo = ''; Fallo = (Recorta $salida) } }
    $cuerpo = $salida.Substring(0, $m.Index).Trim()
    $codigo = [int]$m.Groups[1].Value
    return [pscustomobject]@{ Codigo = $codigo; Ip = $m.Groups[2].Value; Cuerpo = $cuerpo; Fallo = $(if ($codigo -eq 0) { Recorta $cuerpo } else { '' }) }
}

function IpDe([string]$texto) {
    $m = [regex]::Match($texto, '(?m)^(?:ip=)?(\d{1,3}(?:\.\d{1,3}){3})\s*$')
    if ($m.Success) { return $m.Groups[1].Value }
    return ''
}

# La dirección pública con la que sale este equipo, según el primer eco que contesta.
function Eco {
    $fallos = @()
    foreach ($u in $ECOS) {
        $r = Https $u
        $ip = if ($r.Codigo -eq 200) { IpDe $r.Cuerpo } else { '' }
        $quien = ([uri]$u).Host
        if ($ip) { return [pscustomobject]@{ Ip = $ip; De = $quien; Fallos = ($fallos -join '; ') } }
        $fallos += ('{0}: {1}' -f $quien, $(if ($r.Codigo) { "HTTP $($r.Codigo) «$(Recorta $r.Cuerpo 60)»" } else { $r.Fallo }))
    }
    return [pscustomobject]@{ Ip = ''; De = ''; Fallos = ($fallos -join '; ') }
}

function EcoConReintentos([int]$veces = 3) {
    $e = $null
    for ($i = 0; $i -lt $veces; $i++) {
        if ($i -gt 0) { Start-Sleep -Seconds 2 }
        $e = Eco
        if ($e.Ip) { return $e }
    }
    return $e
}

# El eco de un servidor de GUARDIANA (`GET https://<nombre>/vpn/alta` → {"v":1,"ip":"…"}): su código y la dirección que
# ve llegar.
function EcoDe([string]$nombre, [int]$max = 10) {
    $r = Https "https://$nombre/vpn/alta" $max @('-H', 'Accept: application/json')
    $ip = ''
    if ($r.Codigo -eq 200) {
        try { $ip = "$(($r.Cuerpo | ConvertFrom-Json -ErrorAction Stop).ip)" } catch { $ip = '' }
    }
    return [pscustomobject]@{ Codigo = $r.Codigo; Ip = $ip; Texto = $(if ($r.Codigo) { Recorta $r.Cuerpo 120 } else { $r.Fallo }) }
}

function Resuelve([string]$nombre) {
    try {
        $a = [System.Net.Dns]::GetHostAddresses($nombre) | Where-Object { $_.AddressFamily -eq [System.Net.Sockets.AddressFamily]::InterNetwork } | Select-Object -First 1
        if ($a) { return $a.IPAddressToString }
    } catch { }
    return ''
}

# Los sitios (los cuatro últimos dígitos de la clave, en hexadecimal) de las claves del modo estricto con el principio
# `$prefijo` que aparecen en `$texto`, sin repetir y en orden. Siempre una lista, también vacía (la coma: si no,
# PowerShell no devuelve nada y `@($null).Count` es 1).
function Sitios([string]$texto, [string]$prefijo) {
    return , @([regex]::Matches($texto, "(?i)$prefijo([0-9a-f]{4})") | ForEach-Object { [Convert]::ToInt32($_.Groups[1].Value, 16) } | Sort-Object -Unique)
}

# Los sitios de 0 a `$n` - 1 que no están en `$hay`, como texto («» si no falta ninguno).
function Faltan($hay, [int]$n) {
    if ($n -le 0) { return '' }
    return ((0..($n - 1) | Where-Object { @($hay) -notcontains $_ }) -join ',')
}

# Lo que el Filtering Platform de Windows tiene de GUARDIANA, leído por fuera del programa: `netsh wfp show filters`
# (cada filtro, con sus banderas) y `netsh wfp show state` (también los proveedores y las subcapas). De la VPN es todo
# filtro con las claves o el proveedor o la subcapa del modo estricto, o con un nombre «GUARDIANA VPN…». Con -Detalle,
# también `show filters verbose=on`, para comparar.
function Wfp([string]$cuando, [switch]$Detalle) {
    $script:nWfp++
    $f = Join-Path $Trabajo ('filtros-{0}.xml' -f $script:nWfp)
    $s = Join-Path $Trabajo ('estado-{0}.xml' -f $script:nWfp)
    $t0 = Get-Date
    $null = & netsh wfp show filters "file=$f" 2>&1
    $null = & netsh wfp show state "file=$s" 2>&1
    $r = [pscustomobject]@{
        Leido = $false; Vpn = 0; Persistentes = 0; Arranque = 0; Dinamicos = 0; OtrosFijos = 0
        Proveedor = $false; Subcapa = $false; EstadoLeido = $false; Nombres = ''; Fallo = ''
        SitiosP = @(); SitiosA = @(); EstadoP = @(); EstadoA = @(); VerboseP = $null; VerboseA = $null
    }
    try {
        $x = [System.Xml.XmlDocument]::new()
        $x.Load($f)
        $porNombre = [ordered]@{}
        $claves = [System.Collections.Generic.List[string]]::new()
        # por el nombre local de cada elemento: igual si el archivo trae un espacio de nombres
        foreach ($i in $x.SelectNodes("//*[local-name()='item'][*[local-name()='filterKey']]")) {
            $clave = "$($i.SelectSingleNode("*[local-name()='filterKey']").InnerText)"
            $nombre = "$($i.SelectSingleNode("*[local-name()='displayData']/*[local-name()='name']").InnerText)"
            $banderas = (@($i.SelectNodes("*[local-name()='flags']/*[local-name()='item']") | ForEach-Object { $_.InnerText }) -join ',')
            $prov = "$($i.SelectSingleNode("*[local-name()='providerKey']").InnerText)"
            $sub = "$($i.SelectSingleNode("*[local-name()='subLayerKey']").InnerText)"
            $estricto = ($clave -like "*$PERSISTENTES*") -or ($clave -like "*$ARRANQUE*") -or ($prov -like "*$PROVEEDOR*") -or ($sub -like "*$SUBCAPA*")
            $vpn = $estricto -or ($nombre -like 'GUARDIANA VPN*')
            if (-not ($vpn -or ($nombre -like 'GUARDIANA*'))) { continue }
            # (no $arranque: PowerShell no distingue mayúsculas, y taparía la constante $ARRANQUE)
            $esPersistente = $banderas -like '*FWPM_FILTER_FLAG_PERSISTENT*'
            $esDeArranque = $banderas -like '*FWPM_FILTER_FLAG_BOOTTIME*'
            if ($vpn) {
                $r.Vpn++
                $tipo = if ($esPersistente) { 'P' } elseif ($esDeArranque) { 'A' } else { 'D' }
                if ($tipo -eq 'P') { $r.Persistentes++ } elseif ($tipo -eq 'A') { $r.Arranque++ } else { $r.Dinamicos++ }
                if (-not $porNombre.Contains($nombre)) { $porNombre[$nombre] = @{ P = 0; A = 0; D = 0 } }
                $porNombre[$nombre][$tipo]++
                $claves.Add($clave)
            } elseif ($esPersistente -or $esDeArranque) {
                $r.OtrosFijos++
            }
        }
        $todas = $claves -join ' '
        $r.SitiosP = Sitios $todas $PERSISTENTES
        $r.SitiosA = Sitios $todas $ARRANQUE
        $r.Nombres = (@($porNombre.Keys | ForEach-Object { $n = $porNombre[$_]; '{0} [{1} P, {2} A, {3} D]' -f $_, $n.P, $n.A, $n.D }) -join '; ')
        $r.Leido = $true
    } catch {
        $r.Fallo = "show filters: $($_.Exception.Message)"
    }
    try {
        $texto = [System.IO.File]::ReadAllText($s)
        $r.Proveedor = $texto -match $PROVEEDOR
        $r.Subcapa = $texto -match $SUBCAPA
        $r.EstadoP = Sitios $texto $PERSISTENTES
        $r.EstadoA = Sitios $texto $ARRANQUE
        $r.EstadoLeido = $texto.Length -gt 0
    } catch {
        $r.Fallo = ("$($r.Fallo) show state: $($_.Exception.Message)").Trim()
    }
    if ($Detalle) {
        $fv = Join-Path $Trabajo ('filtros-{0}-verbose.xml' -f $script:nWfp)
        $null = & netsh wfp show filters verbose=on "file=$fv" 2>&1
        if (Test-Path -LiteralPath $fv) {
            $tv = [System.IO.File]::ReadAllText($fv)
            $r.VerboseP = Sitios $tv $PERSISTENTES
            $r.VerboseA = Sitios $tv $ARRANQUE
        }
        # Para ver lo que netsh enseña y lo que no: los «todo lo demás» (los únicos sin condiciones) de cada volcado
        foreach ($caso in @(@{ Que = 'show state'; Archivo = $s }, @{ Que = 'show filters'; Archivo = $f })) {
            try {
                # show state trae varios elementos raíz seguidos: van dentro de uno
                $contenido = [System.IO.File]::ReadAllText($caso.Archivo) -replace '<\?xml[^>]*\?>', ''
                $d = [System.Xml.XmlDocument]::new()
                $d.LoadXml("<raiz>$contenido</raiz>")
                $vistos = foreach ($i in $d.SelectNodes("//*[local-name()='item'][*[local-name()='filterKey']]")) {
                    $nombre = "$($i.SelectSingleNode("*[local-name()='displayData']/*[local-name()='name']").InnerText)"
                    if ($nombre -ne 'GUARDIANA VPN: todo lo demás') { continue }
                    $clave = "$($i.SelectSingleNode("*[local-name()='filterKey']").InnerText)"
                    $capa = "$($i.SelectSingleNode("*[local-name()='layerKey']").InnerText)" -replace '^FWPM_LAYER_ALE_AUTH_', ''
                    $id = "$($i.SelectSingleNode("*[local-name()='filterId']").InnerText)"
                    $peso = "$($i.SelectSingleNode("*[local-name()='effectiveWeight']/*[local-name()='uint64']").InnerText)"
                    $m = [regex]::Match($clave, '([0-9a-fA-F]{4})\}?$')
                    $quien = if ($clave -match $PERSISTENTES) { 'P' } elseif ($clave -match $ARRANQUE) { 'A' } else { '?' }
                    '{0}{1} {2} id {3} peso {4}' -f $quien, $(if ($m.Success) { [Convert]::ToInt32($m.Groups[1].Value, 16) } else { '?' }), $capa, $id, $peso
                }
                Linea "     «todo lo demás» en $($caso.Que): $(@($vistos) -join '; ')"
            } catch {
                Linea "     «todo lo demás» en $($caso.Que): $($_.Exception.Message)"
            }
        }
    }
    $segundos = ((Get-Date) - $t0).TotalSeconds
    $prov = if ($r.Proveedor) { 'sí' } else { 'no' }
    $sub = if ($r.Subcapa) { 'sí' } else { 'no' }
    $fallo = if ($r.Fallo) { "; no se pudo leer: $($r.Fallo)" } else { '' }
    Linea ('     netsh wfp, {0} ({1:N1} s): {2} filtros de la VPN ({3} persistentes, {4} de arranque, {5} dinámicos); el proveedor del modo estricto: {6}; su subcapa: {7}; otros filtros fijos de GUARDIANA: {8}{9}' -f $cuando, $segundos, $r.Vpn, $r.Persistentes, $r.Arranque, $r.Dinamicos, $prov, $sub, $r.OtrosFijos, $fallo)
    if ($r.Nombres) { Linea "     por nombre: $($r.Nombres)" }
    Linea ('     claves del modo estricto: en show filters {0} persistentes y {1} de arranque; en show state {2} y {3}{4}' -f @($r.SitiosP).Count, @($r.SitiosA).Count, @($r.EstadoP).Count, @($r.EstadoA).Count, $(if ($null -ne $r.VerboseP) { "; en show filters verbose=on $(@($r.VerboseP).Count) y $(@($r.VerboseA).Count)" } elseif ($Detalle) { '; show filters verbose=on no dejó archivo' } else { '' }))
    return $r
}

# Lo que Windows guarda para el próximo arranque del equipo (la política persistente y la de arranque del servicio de
# filtrado, en el registro: HKLM\SYSTEM\CurrentControlSet\Services\BFE\Parameters\Policy), buscado por las claves del
# modo estricto en los nombres de sus claves y de sus valores.
function PoliticaGuardada {
    $raiz = 'HKLM:\SYSTEM\CurrentControlSet\Services\BFE\Parameters\Policy'
    $r = [pscustomobject]@{ Leido = $false; P = @(); A = @(); Proveedor = $false; Subcapa = $false; Donde = ''; Fallo = '' }
    try {
        $todas = @(Get-Item -LiteralPath $raiz -ErrorAction Stop) + @(Get-ChildItem -LiteralPath $raiz -Recurse -ErrorAction SilentlyContinue)
        $nombres = [System.Collections.Generic.List[string]]::new()
        $donde = [ordered]@{}
        foreach ($k in $todas) {
            foreach ($n in @($k.PSChildName) + @($k.GetValueNames())) {
                $t = "$n"
                if ($t -match "(?i)$PERSISTENTES|$ARRANQUE|$PROVEEDOR|$SUBCAPA") {
                    $nombres.Add($t)
                    $donde[($k.Name -replace '^HKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Services\\BFE\\Parameters\\Policy\\?', '')] = 1
                }
            }
        }
        $junto = $nombres -join ' '
        $r.P = Sitios $junto $PERSISTENTES
        $r.A = Sitios $junto $ARRANQUE
        $r.Proveedor = $junto -match $PROVEEDOR
        $r.Subcapa = $junto -match $SUBCAPA
        $r.Donde = (@($donde.Keys) -join '; ')
        $r.Leido = $true
    } catch {
        $r.Fallo = $_.Exception.Message
    }
    if ($r.Leido) {
        Linea ('     lo guardado para el próximo arranque (registro de BFE): {0} filtros persistentes y {1} de arranque del modo estricto, proveedor {2}, subcapa {3}{4}' -f @($r.P).Count, @($r.A).Count, $(if ($r.Proveedor) { 'sí' } else { 'no' }), $(if ($r.Subcapa) { 'sí' } else { 'no' }), $(if ($r.Donde) { " (en: $($r.Donde))" } else { '' }))
    } else {
        Linea "     lo guardado para el próximo arranque (registro de BFE): no se pudo leer ($($r.Fallo))"
    }
    return $r
}

# ---- La VPN, por el panel.

# Espera a que el panel diga «conectada», como mucho `$plazo` segundos. Tras `POST /api/vpn/conecta` el panel dice
# «conectando» antes de contestar, así que «conectada» es ya la conexión nueva; su servidor es el nombre del extremo que
# dio el alta, y en qué salida está lo comprueba ConTunel. Con -VigilaBloqueo, mira también en cada vuelta que el
# bloqueo siga en Windows.
function EsperaConectada($salida, [int]$plazo, [switch]$VigilaBloqueo) {
    $t0 = Get-Date
    $fases = [System.Collections.Generic.List[string]]::new()
    $sinBloqueo = 0
    $v = $null
    do {
        $v = Panel 'GET' '/api/vpn'
        if ($v -and ($fases.Count -eq 0 -or $fases[$fases.Count - 1] -ne "$($v.fase)")) { $fases.Add("$($v.fase)") }
        if ($VigilaBloqueo) {
            $e = Estado
            if ($null -ne $e -and $e.persistentes -eq 0) { $sinBloqueo++ }
        }
        if ($v -and $v.fase -eq 'conectada') { break }
        Start-Sleep -Milliseconds 400
    } while (((Get-Date) - $t0).TotalSeconds -lt $plazo)
    $bien = ($null -ne $v) -and ($v.fase -eq 'conectada')
    return [pscustomobject]@{ Vista = $v; Segundos = [math]::Round(((Get-Date) - $t0).TotalSeconds, 1); Fases = ($fases -join ' → '); SinBloqueo = $sinBloqueo; Bien = $bien }
}

# `POST /api/vpn/conecta` {"salida": id} y la espera hasta «conectada»: lo que dice entonces `GET /api/vpn`, o $null.
function Conecta($salida, [int]$plazo = 60, [switch]$VigilaBloqueo) {
    $r = Panel 'POST' '/api/vpn/conecta' @{ salida = $salida.Id }
    $pedido = 'POST /api/vpn/conecta {"salida":"' + $salida.Id + '"}'
    $dijo = if ($null -eq $r) { "sin respuesta: $script:panelFallo" } else { "contestó: $(Json $r)" }
    Bien (($null -ne $r) -and ($r.ok -eq $true)) $pedido $dijo
    if ($null -eq $r -or $r.ok -ne $true) { return $null }
    $c = EsperaConectada $salida $plazo -VigilaBloqueo:$VigilaBloqueo
    Bien $c.Bien ('conectada, pedida {0} ({1}), en {2} s; el panel dice el servidor {3}' -f $salida.Ciudad, $salida.Nombre, $c.Segundos, $c.Vista.servidor) "fases: $($c.Fases); GET /api/vpn: $(Json $c.Vista)"
    if ($VigilaBloqueo) {
        Bien ($c.SinBloqueo -eq 0) 'el bloqueo, en Windows durante todo el cambio' "vistazos sin bloqueo: $($c.SinBloqueo)"
    }
    if ($c.Bien) { return $c.Vista }
    return $null
}

# La dirección pública es la de `$salida`, y no la del equipo.
function LaDireccion($salida) {
    $eco = EcoConReintentos
    $detalle = 'eco: {0}; la de {1}: {2}; la del equipo: {3}' -f $(if ($eco.De) { $eco.De } else { 'ninguno' }), $salida.Ciudad, $salida.Ip, $script:equipo
    if ($eco.Fallos) { $detalle += "; fallos: $($eco.Fallos)" }
    Bien (($eco.Ip -ne '') -and ($eco.Ip -eq $salida.Ip) -and ($eco.Ip -ne $script:equipo)) ('la dirección pública que ve un eco es la de {0}: {1}' -f $salida.Ciudad, $(if ($eco.Ip) { $eco.Ip } else { 'ninguna' })) $detalle
    return $eco
}

# Las comprobaciones con el túnel a `$salida` puesto; `$v`, lo que dijo `GET /api/vpn` al conectar.
function ConTunel($salida, $v, [switch]$Propia) {
    # el panel lee la conexión sin esperar por ella (`try_lock`): si en ese momento estaba ocupada, otra vez
    for ($i = 0; $i -lt 3 -and $v.dns_tunel -ne $true; $i++) {
        Start-Sleep -Milliseconds 400
        $otra = Panel 'GET' '/api/vpn'
        if ($otra) { $v = $otra }
    }
    Bien (($v.estricto -eq $true) -and ($v.bloqueado -eq $true)) 'el panel dice: modo estricto y bloqueo puestos'
    Bien (("$($v.salida.id)" -eq $salida.Id) -and ("$($v.salida.ip)" -eq $salida.Ip)) ('el panel dice la salida: {0}, {1}, {2}' -f $v.salida.id, $v.salida.ciudad, $v.salida.ip)
    Bien (($v.dns_tunel -eq $true) -and ($v.postcuantica -eq $true)) 'el panel dice: los nombres por el túnel y el alta postcuántica'
    $saludo = if ($null -ne $v.saludo) { ([DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() - [long]$v.saludo) / 1000 } else { $null }
    Bien ($null -ne $saludo) ('el túnel, arriba: el servidor saludó {0}' -f $(if ($null -ne $saludo) { 'hace {0:N1} s' -f $saludo } else { 'nunca' })) ('{0} bytes enviados y {1} recibidos' -f $v.subidos, $v.bajados)
    $e = Estado
    Bien (($null -ne $e) -and $e.estricto -and $e.encendida -and ($e.servidor -eq $salida.Nombre) -and ($e.persistentes -gt 0) -and ($e.arranque -gt 0) -and $e.objetos) ('el bloqueo en Windows: {0} filtros persistentes y {1} de arranque' -f $e.persistentes, $e.arranque) "guardiana vpn estado --json: $(Json $e)"
    $ad = Adaptador
    Bien ($ad -eq 'Up') ('el adaptador GUARDIANA, arriba ({0})' -f $(if ($ad) { $ad } else { 'no está' })) "rutas por defecto: $(Rutas)"
    $h = Https 'https://api.github.com/zen'
    Bien ($h.Codigo -gt 0) 'una conexión HTTPS a internet funciona (api.github.com, con el DNS del sistema)' $(if ($h.Codigo) { "HTTP $($h.Codigo), desde $($h.Ip)" } else { $h.Fallo })
    $null = LaDireccion $salida
    $traza = Https 'https://1.1.1.1/cdn-cgi/trace' 8
    $loc = [regex]::Match($traza.Cuerpo, '(?m)^loc=(\w+)').Groups[1].Value
    if ($loc) { Linea "     Cloudflare sitúa la dirección en: $loc" }
    # el eco de GUARDIANA en otra salida ve llegar la dirección de esta (la propia no contesta a lo que llega por su
    # túnel: sus reglas solo dejan el resolutor)
    $otras = @($SALIDAS | Where-Object { $_.Id -ne $salida.Id -and $_.Disponible })
    foreach ($o in $otras) {
        $x = EcoDe $o.Nombre
        Bien (($x.Ip -ne '') -and ($x.Ip -eq $salida.Ip)) ('el eco de GUARDIANA en {0} ve llegar la dirección de {1}: {2}' -f $o.Ciudad, $salida.Ciudad, $(if ($x.Ip) { $x.Ip } else { 'ninguna' })) ('GET https://{0}/vpn/alta: {1}' -f $o.Nombre, $(if ($x.Codigo) { "HTTP $($x.Codigo) $($x.Texto)" } else { $x.Texto }))
    }
    if ($otras.Count -eq 0) { Linea '     (ninguna otra salida contesta: el eco de GUARDIANA no se puede preguntar por el túnel)' }
    if ($Propia) {
        $x = EcoDe $salida.Nombre 6
        Linea ('     el eco de su propia salida, por el túnel: {0}' -f $(if ($x.Codigo) { "HTTP $($x.Codigo) $($x.Texto)" } else { "$($x.Texto) (lo esperado: sus reglas solo dejan el resolutor)" }))
    }
    if ($script:tarjeta -and $script:tarjeta.Ip) {
        $t = Tcp '1.1.1.1' 443 $script:tarjeta.Ip
        Bien ($t -eq 'cortada') ('por la tarjeta ({0}), fuera del túnel, nada sale: TCP a 1.1.1.1:443 {1}' -f $script:tarjeta.Ip, $t)
    }
    $k = Panel 'GET' '/api/vpn/comprueba' -segundos 60
    if ($k -and $k.dns -eq 'desconocido') {
        Start-Sleep -Seconds 1
        $k = Panel 'GET' '/api/vpn/comprueba' -segundos 60
    }
    if ($null -eq $k) {
        Bien $false 'GET /api/vpn/comprueba contesta' $script:panelFallo
    } else {
        Bien (($k.dns -eq 'tunel') -and ($k.ipv6 -eq 'bloqueado')) ('lo que una web verá (GET /api/vpn/comprueba): DNS «{0}», IPv6 «{1}»' -f $k.dns, $k.ipv6) (Json $k)
        if ($k.ip_vista) {
            Bien ("$($k.ip_vista)" -eq $salida.Ip) "y la dirección que da el panel, la de $($salida.Ciudad): $($k.ip_vista)"
        } else {
            Linea '     el panel no da la dirección (ninguna otra salida contesta a su eco): no se comprueba'
        }
    }
    $despues = Panel 'GET' '/api/vpn'
    if ($despues) { Linea ('     por el túnel, desde que se abrió: {0} bytes enviados y {1} recibidos' -f $despues.subidos, $despues.bajados) }
}

# `guardiana vpn apaga --menu`, como lo lanza la entrada «Apagar la VPN de GUARDIANA» del menú Inicio, con la Intro que
# espera su ventana al final; como mucho 60 s.
function Apaga {
    $psi = [System.Diagnostics.ProcessStartInfo]::new($g)
    foreach ($a in @('vpn', 'apaga', '--menu')) { $psi.ArgumentList.Add($a) }
    $psi.UseShellExecute = $false
    $psi.RedirectStandardInput = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.StandardOutputEncoding = [System.Text.Encoding]::UTF8
    $psi.StandardErrorEncoding = [System.Text.Encoding]::UTF8
    $p = [System.Diagnostics.Process]::Start($psi)
    try {
        $p.StandardInput.WriteLine('')
        $p.StandardInput.Close()
    } catch { }
    $o = $p.StandardOutput.ReadToEndAsync()
    $er = $p.StandardError.ReadToEndAsync()
    $termino = $p.WaitForExit(60000)
    if (-not $termino) { try { $p.Kill() } catch { } }
    $codigo = if ($termino) { $p.ExitCode } else { -1 }
    return [pscustomobject]@{ Termino = $termino; Codigo = $codigo; Salida = (Recorta ($o.Result + ' ' + $er.Result) 300) }
}

# Lo que Windows tiene tras apagar, leído por fuera del programa: nada de la VPN de GUARDIANA en el Filtering Platform
# (ni filtros, ni el proveedor ni la subcapa del modo estricto) y nada guardado para el próximo arranque del equipo.
function NadaEnWindows([string]$cuando) {
    $w = Wfp $cuando
    Bien ($w.Leido -and ($w.Vpn -eq 0) -and ($w.OtrosFijos -eq 0)) "netsh wfp show filters, $($cuando): ningún filtro de la VPN de GUARDIANA (ni persistente, ni de arranque, ni dinámico) ni otro fijo de GUARDIANA"
    Bien ($w.EstadoLeido -and -not $w.Proveedor -and -not $w.Subcapa -and (@($w.EstadoP).Count -eq 0) -and (@($w.EstadoA).Count -eq 0)) "netsh wfp show state, $($cuando): ni el proveedor, ni la subcapa, ni ninguna clave de filtro del modo estricto"
    $pg = PoliticaGuardada
    if ($pg.Leido) {
        Bien ((@($pg.P).Count -eq 0) -and (@($pg.A).Count -eq 0) -and -not $pg.Proveedor -and -not $pg.Subcapa) "nada del modo estricto guardado para el próximo arranque del equipo, $($cuando)"
    }
}

# Con el bloqueo puesto: lo que `guardiana vpn estado` dice que hay (`$e`), visto por fuera del programa, sitio por
# sitio de cada conjunto (su orden en crates/tunel/src/corte.rs, función estricto). Así las comprobaciones de «no queda
# nada» saben ver lo que buscan.
function ElBloqueoPorFuera($e) {
    $w = Wfp 'con el bloqueo puesto' -Detalle
    $np = [int]$e.persistentes
    $na = [int]$e.arranque
    $faltanEP = Faltan $w.EstadoP $np
    $faltanEA = Faltan $w.EstadoA $na
    Bien ($w.EstadoLeido -and ($np -gt 0) -and ($na -gt 0) -and -not $faltanEP -and -not $faltanEA -and $w.Proveedor -and $w.Subcapa) ('netsh wfp show state ve el bloqueo entero que dice el programa: {0} persistentes y {1} de arranque por su clave, el proveedor y la subcapa del modo estricto' -f $np, $na) $(if ($faltanEP -or $faltanEA) { "faltan los persistentes de los sitios [$faltanEP] y los de arranque de los sitios [$faltanEA]" } else { '' })
    # show filters no lista todos: lo que no lista, dicho (lo que cuenta es lo que hay, y eso lo dicen show state y el
    # registro)
    $faltanP = Faltan $w.SitiosP $np
    $faltanA = Faltan $w.SitiosA $na
    Linea ('     show filters lista {0} de {1} persistentes y {2} de {3} de arranque{4}' -f @($w.SitiosP).Count, $np, @($w.SitiosA).Count, $na, $(if ($faltanP -or $faltanA) { "; no lista los persistentes de los sitios [$faltanP] ni los de arranque de los sitios [$faltanA]" } else { '' }))
    $pg = PoliticaGuardada
    if ($pg.Leido) {
        $faltanGP = Faltan $pg.P $np
        $faltanGA = Faltan $pg.A $na
        Bien (-not $faltanGP -and -not $faltanGA -and $pg.Proveedor -and $pg.Subcapa) ('lo guardado para el próximo arranque del equipo: el bloqueo entero ({0} persistentes y {1} de arranque), su proveedor y su subcapa' -f @($pg.P).Count, @($pg.A).Count) $(if ($faltanGP -or $faltanGA) { "faltan los persistentes de los sitios [$faltanGP] y los de arranque de los sitios [$faltanGA]" } else { '' })
    }
}

# Las tres llamadas del Filtering Platform que usa QuitaLosDeArranque. Compilarlas tarda unos segundos: se hace antes
# de cortar nada.
function PreparaWfp {
    if ('Guardiana.PruebaWfp' -as [type]) { return }
    try {
        Add-Type -Namespace Guardiana -Name PruebaWfp -MemberDefinition @'
[DllImport("fwpuclnt.dll")] public static extern uint FwpmEngineOpen0(System.IntPtr serverName, uint authnService, System.IntPtr authIdentity, System.IntPtr session, out System.IntPtr engineHandle);
[DllImport("fwpuclnt.dll")] public static extern uint FwpmFilterDeleteByKey0(System.IntPtr engineHandle, ref System.Guid key);
[DllImport("fwpuclnt.dll")] public static extern uint FwpmEngineClose0(System.IntPtr engineHandle);
'@
    } catch {
        Linea "     (no se pudieron preparar las llamadas al Filtering Platform: $($_.Exception.Message))"
    }
}

# Quita de Windows los filtros de arranque del modo estricto, por sus claves fijas (solo la prueba, con el servicio
# muerto): así queda lo que está en vigor tras arrancar el equipo, cuando el servicio de filtrado ya cargó los
# persistentes y los de arranque dejan de valer, y GUARDIANA aún no ha arrancado. El servicio los vuelve a poner al
# arrancar. Cuántos quitó, y lo que Windows dijo de otros.
function QuitaLosDeArranque {
    PreparaWfp
    if (-not ('Guardiana.PruebaWfp' -as [type])) { return [pscustomobject]@{ Quitados = 0; Fallo = 'sin las llamadas al Filtering Platform' } }
    $motor = [IntPtr]::Zero
    # RPC_C_AUTHN_WINNT, como el programa (crates/apps/src/corte_vpn.rs)
    $r = [Guardiana.PruebaWfp]::FwpmEngineOpen0([IntPtr]::Zero, 10, [IntPtr]::Zero, [IntPtr]::Zero, [ref]$motor)
    if ($r -ne 0) { return [pscustomobject]@{ Quitados = 0; Fallo = ('FwpmEngineOpen0 = 0x{0:x8}' -f $r) } }
    $quitados = 0
    $otros = [System.Collections.Generic.List[string]]::new()
    try {
        for ($i = 0; $i -lt 64; $i++) {
            $clave = [Guid]::Parse(('{0}{1:x4}' -f $ARRANQUE, $i))
            $r = [Guardiana.PruebaWfp]::FwpmFilterDeleteByKey0($motor, [ref]$clave)
            # FWP_E_FILTER_NOT_FOUND (0x80320003): ese sitio no lo usa el conjunto
            if ($r -eq 0) { $quitados++ } elseif ($r -ne [uint32]2150760451) { $otros.Add(('{0}: 0x{1:x8}' -f $i, $r)) }
        }
    } finally {
        $null = [Guardiana.PruebaWfp]::FwpmEngineClose0($motor)
    }
    return [pscustomobject]@{ Quitados = $quitados; Fallo = ($otros -join '; ') }
}

# ---- La prueba.

$vigilante = $null
try {
    Linea "GUARDIANA: $g"
    Linea "Windows $([Environment]::OSVersion.Version), PowerShell $($PSVersionTable.PSVersion)"

    # 0. Antes, desde fuera del túnel
    $script:tarjeta = Tarjeta
    if ($script:tarjeta) {
        Linea ('la tarjeta: {0}, dirección {1}, puerta de enlace {2}' -f $script:tarjeta.Alias, $script:tarjeta.Ip, $script:tarjeta.Puerta)
    } else {
        Linea 'la tarjeta: no se encontró la ruta por defecto'
    }
    Linea "rutas por defecto: $(Rutas)"
    $eq = EcoConReintentos
    $script:equipo = $eq.Ip
    Bien ($eq.Ip -ne '') "la dirección pública del equipo: $($eq.Ip)" "eco: $($eq.De)$(if ($eq.Fallos) { "; fallos: $($eq.Fallos)" })"
    Bien ((Tcp '1.1.1.1' 443) -eq 'conecta') 'antes, la red sale (TCP a 1.1.1.1:443)'
    foreach ($s in $SALIDAS) {
        $s.Ip = Resuelve $s.Nombre
        $x = EcoDe $s.Nombre
        # lo que el programa toma por «su alta contesta» (crates/tunel/src/salidas.rs, `es_del_alta`)
        $s.Disponible = ($s.Ip -ne '') -and (@(200, 400, 405, 429) -contains $x.Codigo)
        $s.Motivo = if ($s.Ip -eq '') { 'su nombre no resuelve' } elseif ($x.Codigo) { "su alta contesta HTTP $($x.Codigo)" } else { "su alta no contesta: $($x.Texto)" }
        Linea ('{0} ({1}): {2}; {3}{4}' -f $s.Ciudad, $s.Nombre, $(if ($s.Ip) { $s.Ip } else { 'sin dirección' }), $s.Motivo, $(if ($x.Codigo -eq 200) { ", su eco ve $($x.Ip)" } else { '' }))
        if ($x.Ip) {
            Bien ($x.Ip -eq $script:equipo) "el eco de GUARDIANA en $($s.Ciudad), desde fuera, ve la dirección del equipo: $($x.Ip)"
        }
    }
    Bien $hel.Disponible "Helsinki ($($hel.Nombre)) contesta desde fuera" $hel.Motivo
    if ($hel.Ip -and $hel.Ip -ne $HELSINKI_IP) { Linea "     ojo: $($hel.Nombre) resuelve a $($hel.Ip), no a $HELSINKI_IP" }

    # El servicio de verdad, como lo deja el instalador (sin VPN y con el modo estricto por defecto)
    $instala = (& $g service install 2>&1) -join ' '
    Bien (Espera 60 { (Servicio) -eq 'Running' }) 'el servicio, instalado y en marcha' (Recorta $instala)
    $script:v0 = $null
    Bien (Espera 60 { $script:v0 = Panel 'GET' '/api/vpn'; $null -ne $script:v0 }) 'su panel contesta (GET /api/vpn)' (Json $script:v0)
    $listo = ($null -ne $script:v0) -and ($script:v0.disponible -eq $true)
    Bien ($listo -and ($script:v0.fase -eq 'apagada') -and ($script:v0.bloqueado -eq $false)) 'la VPN, disponible y apagada, sin bloqueo'
    $sal = Panel 'GET' '/api/vpn/salidas' -segundos 60
    Linea "     GET /api/vpn/salidas (medido por el servicio, sin VPN): $(Json $sal)"
    PreparaWfp

    if (-not $listo) {
        Bien $false 'la VPN no está disponible en el panel: no se sigue' $script:panelFallo
    } else {
        # El vigilante, aparte, con un plazo que cada fase vuelve a poner
        Plazo 300
        $vigilanteGuion = Join-Path $PSScriptRoot 'vigilante_vpn.ps1'
        $argumentos = @('-NoProfile', '-NonInteractive', '-File', "`"$vigilanteGuion`"", '-Exe', "`"$g`"", '-Plazo', "`"$plazoArchivo`"", '-Fin', "`"$finArchivo`"", '-Registro', "`"$vigiaRegistro`"")
        $vigilante = Start-Process -FilePath (Get-Process -Id $PID).Path -ArgumentList $argumentos -WindowStyle Hidden -PassThru
        Bien (($null -ne $vigilante) -and (Espera 10 { Test-Path -LiteralPath $vigiaRegistro })) "el vigilante, en marcha (proceso $(if ($vigilante) { $vigilante.Id }))"

        # 1. El modo estricto y Helsinki
        Fase '1. El modo estricto y Helsinki' 180
        $r = Panel 'POST' '/api/vpn/estricto' @{ activo = $true }
        Bien (($null -ne $r) -and ($r.ok -eq $true) -and ($r.estricto -eq $true)) 'el modo estricto, encendido (POST /api/vpn/estricto {"activo":true})' "contestó: $(Json $r)"
        $v = Conecta $hel 60
        $conectada = $null -ne $v
        if ($conectada) {
            ConTunel $hel $v -Propia
            ElBloqueoPorFuera (Estado)
        }

        # 2. Las otras salidas, solo si su alta contestó al principio, y vuelta a Helsinki
        Fase '2. Núremberg y Miami, y vuelta a Helsinki' 150
        $cambio = $false
        foreach ($s in @($SALIDAS | Where-Object { $_.Id -ne 'hel' })) {
            if (-not $s.Disponible) {
                Omite "$($s.Ciudad) ($($s.Nombre)): salida no disponible, se omite ($($s.Motivo))"
                continue
            }
            if (-not $conectada) {
                Omite "$($s.Ciudad): sin túnel a Helsinki, el cambio de salida no se prueba"
                continue
            }
            Plazo 150
            Linea "--- $($s.Ciudad)"
            $cambio = $true
            $v = Conecta $s 75 -VigilaBloqueo
            if ($v) { ConTunel $s $v }
        }
        if ($cambio) {
            Plazo 150
            Linea '--- vuelta a Helsinki'
            $v = Conecta $hel 75 -VigilaBloqueo
            $conectada = $null -ne $v
            if ($conectada) { $null = LaDireccion $hel }
        }

        # 3. El servicio muerto con el túnel puesto
        Fase '3. El servicio muerto con el túnel puesto' 150
        if (-not $conectada) {
            Omite 'el servicio muerto con el túnel puesto: no hay túnel a Helsinki'
        } else {
            # Windows lo arrancaría solo a los 5 s (sus acciones de recuperación: 5 s, 15 s y 60 s). Para la prueba, a
            # los 60 s, como tras la tercera caída del día: así el rato con el servicio muerto es el que se comprueba, y
            # lo arranca la prueba. El servicio vuelve a poner las suyas al arrancar.
            $null = & sc.exe failure guardiana reset= 86400 actions= restart/60000 2>&1
            Linea "     acciones de recuperación del servicio, a 60 s para la prueba (sc.exe: $LASTEXITCODE)"
            $pidS = ProcesoDelServicio
            $muere = Get-Date
            Plazo 120
            $null = & taskkill.exe /F /PID $pidS 2>&1
            Bien ($LASTEXITCODE -eq 0) "taskkill /F /PID $pidS (el proceso del servicio), con el túnel a Helsinki puesto"
            Bien (Espera 10 { $null -eq (Get-Process -Id $pidS -ErrorAction SilentlyContinue) }) "el proceso ya no está (el servicio: $(Servicio))"
            Bien (Espera 10 { (Adaptador) -eq '' }) 'el adaptador del túnel se fue con el proceso'
            $e = Estado
            Bien (($null -ne $e) -and ($e.persistentes -gt 0) -and ($e.arranque -gt 0) -and $e.encendida) ('con el servicio muerto, el bloqueo sigue en Windows: {0} persistentes, {1} de arranque' -f $e.persistentes, $e.arranque) (Json $e)
            $script:t3 = ''
            $ok = Espera 10 { $script:t3 = Tcp '1.1.1.1' 443 '' 2000; $script:t3 -eq 'cortada' }
            Bien $ok "con el servicio muerto, nada sale: TCP a 1.1.1.1:443 $($script:t3) (WSAEACCES, el filtro)"
            if ($script:tarjeta -and $script:tarjeta.Ip) {
                $t = Tcp '1.1.1.1' 443 $script:tarjeta.Ip 2000
                Bien ($t -eq 'cortada') "ni por la tarjeta ($($script:tarjeta.Ip)): TCP a 1.1.1.1:443 $t"
            }
            # el permiso hacia el alta del servidor es solo del programa del servicio: otro programa no llega
            $t = Tcp $hel.Ip 443 '' 2000
            Bien ($t -eq 'cortada') "ni al HTTPS del servidor desde otro programa (TCP a $($hel.Ip):443): $t"
            $h = Https 'https://1.1.1.1/cdn-cgi/trace' 5
            Bien ($h.Codigo -eq 0) 'ni HTTPS a internet' $(if ($h.Codigo) { "contestó HTTP $($h.Codigo): $(Recorta $h.Cuerpo 80)" } else { $h.Fallo })
            # unos segundos muerto (como tarda Windows en arrancarlo tras una caída), mirando cada poco que nada salga
            $vueltas = 0
            $fugas = [System.Collections.Generic.List[string]]::new()
            while (((Get-Date) - $muere).TotalSeconds -lt 12) {
                $vueltas++
                $t = Tcp '1.1.1.1' 443 '' 1500
                if ($t -ne 'cortada') { $fugas.Add(('{0:N1} s: {1}' -f ((Get-Date) - $muere).TotalSeconds, $t)) }
                Start-Sleep -Milliseconds 700
            }
            Bien ($fugas.Count -eq 0) ('durante {0:N0} s con el servicio muerto, nada sale: {1} intentos, todos cortados por el filtro' -f ((Get-Date) - $muere).TotalSeconds, $vueltas) $(if ($fugas.Count) { "no cortados: $($fugas -join '; ')" } else { '' })
            # Tras arrancar el equipo, los filtros de arranque dejan de valer cuando el servicio de filtrado carga los
            # persistentes, y GUARDIANA aún no ha arrancado: entonces el bloqueo es solo el persistente. Para verlo sin
            # reiniciar, la prueba quita los de arranque (el servicio los vuelve a poner al arrancar).
            $q = QuitaLosDeArranque
            $e = Estado
            Bien (($q.Quitados -gt 0) -and -not $q.Fallo -and ($null -ne $e) -and ($e.arranque -eq 0) -and ($e.persistentes -gt 0)) ('la prueba quita los {0} filtros de arranque: queda solo el bloqueo persistente ({1} filtros), como tras arrancar el equipo' -f $q.Quitados, $e.persistentes) $(if ($q.Fallo) { "Windows dijo: $($q.Fallo)" } else { '' })
            $script:t3 = ''
            $ok = Espera 3 { $script:t3 = Tcp '1.1.1.1' 443 '' 1500; $script:t3 -ne 'cortada' } 300
            Bien (-not $ok) "solo con los filtros persistentes, nada sale: TCP a 1.1.1.1:443 $($script:t3)" $(if ($ok) { 'el bloqueo persistente no corta el tráfico general: tras reiniciar el equipo, hasta que arranque el servicio, saldría todo menos el DNS (los persistentes «todo lo demás», sitios 32 a 35, no bastan)' } else { '' })
            if ($script:tarjeta -and $script:tarjeta.Ip) {
                $t = Tcp '1.1.1.1' 443 $script:tarjeta.Ip 1500
                Bien ($t -eq 'cortada') "solo con los persistentes, ni por la tarjeta: TCP a 1.1.1.1:443 $t"
            }
            $sigueMuerto = $null -eq (Get-Process -Id $pidS -ErrorAction SilentlyContinue) -and (Servicio) -ne 'Running'
            Bien $sigueMuerto ('todo eso, con el servicio aún muerto ({0:N1} s desde taskkill; el servicio: {1})' -f ((Get-Date) - $muere).TotalSeconds, (Servicio))
            # Arrancado otra vez: vuelve solo
            $arranca = (& $g service start 2>&1) -join ' '
            Bien (Espera 30 { (Servicio) -eq 'Running' }) ('el servicio, arrancado otra vez ({0:N1} s muerto)' -f ((Get-Date) - $muere).TotalSeconds) (Recorta $arranca)
            $c = EsperaConectada $hel 60 -VigilaBloqueo
            Bien ($c.Bien -and ("$($c.Vista.salida.id)" -eq $hel.Id)) "vuelve a conectar solo a Helsinki, en $($c.Segundos) s desde que arrancó" "fases: $($c.Fases); GET /api/vpn: $(Json $c.Vista)"
            Bien ($c.SinBloqueo -eq 0) 'el bloqueo, en Windows todo el rato' "vistazos sin bloqueo: $($c.SinBloqueo)"
            $e = Estado
            Bien (($null -ne $e) -and ($e.persistentes -gt 0) -and ($e.arranque -gt 0)) ('al arrancar, el servicio vuelve a poner el bloqueo entero: {0} persistentes y {1} de arranque' -f $e.persistentes, $e.arranque) (Json $e)
            $eco = LaDireccion $hel
            Linea ('     sin red: {0:N1} s, de taskkill a ver otra vez la dirección del servidor' -f ((Get-Date) - $muere).TotalSeconds)
            Linea "     acciones de recuperación, otra vez las del servicio: $(Recorta ((& sc.exe qfailure guardiana 2>&1) -join ' ') 300)"
            $conectada = $c.Bien
        }

        # 4. Desconectar
        Fase '4. Desconectar' 150
        $r = Panel 'POST' '/api/vpn/desconecta' -segundos 60
        Bien (($null -ne $r) -and ($r.ok -eq $true)) 'Desconectar (POST /api/vpn/desconecta)' $(if ($null -eq $r) { "sin respuesta: $script:panelFallo" } else { "contestó: $(Json $r)" })
        $v = Panel 'GET' '/api/vpn'
        Bien (($null -ne $v) -and ($v.fase -eq 'apagada') -and ($v.bloqueado -eq $false)) "el panel dice: $($v.fase), sin bloqueo" (Json $v)
        $e = Estado
        Bien (Limpio $e) 'nada del modo estricto en Windows, y la VPN apagada en lo guardado (guardiana vpn estado)' (Json $e)
        Bien (Espera 15 { (Adaptador) -eq '' }) 'el adaptador GUARDIANA ya no está'
        Bien (Espera 20 { (Tcp '1.1.1.1' 443) -eq 'conecta' }) 'la red sale otra vez (TCP a 1.1.1.1:443)' "rutas por defecto: $(Rutas)"
        if ($script:tarjeta -and $script:tarjeta.Ip) {
            Bien ((Tcp '1.1.1.1' 443 $script:tarjeta.Ip) -eq 'conecta') "también por la tarjeta ($($script:tarjeta.Ip))"
        }
        $eco = EcoConReintentos 5
        Bien (($eco.Ip -ne '') -and ($eco.Ip -eq $script:equipo)) "la dirección pública vuelve a ser la del equipo: $($eco.Ip)" "eco: $($eco.De)$(if ($eco.Fallos) { "; fallos: $($eco.Fallos)" })"
        NadaEnWindows 'tras Desconectar'

        # 5. La orden de emergencia, con el servicio parado y el bloqueo puesto
        Fase '5. La orden de emergencia, con el servicio parado' 150
        $v = Conecta $hel 60
        if ($null -eq $v) { Linea '     sin conectar: se sigue igual (el modo estricto pone el bloqueo también sin servidor)' }
        Plazo 150
        $para = (& $g service stop 2>&1) -join ' '
        Bien (Espera 60 { (Servicio) -eq 'Stopped' }) 'el servicio, parado (como lo para una actualización)' (Recorta $para)
        $sinRed = Get-Date
        $e = Estado
        Bien (($null -ne $e) -and ($e.persistentes -gt 0) -and ($e.arranque -gt 0) -and $e.encendida) 'con el servicio parado, el bloqueo se queda y la VPN sigue encendida en lo guardado' (Json $e)
        Bien ((Adaptador) -eq '') 'sin túnel: el adaptador se fue con el servicio'
        $t = Tcp '1.1.1.1' 443 '' 3000
        Bien ($t -eq 'cortada') "sin el servicio, nada sale: TCP a 1.1.1.1:443 $t"
        $ap = Apaga
        Bien ($ap.Termino -and ($ap.Codigo -eq 0)) "guardiana vpn apaga --menu (la de «Apagar la VPN de GUARDIANA»), con el servicio parado: código $($ap.Codigo)" "dijo: $($ap.Salida)"
        $e = Estado
        Bien (Limpio $e) 'la orden quita el bloqueo entero y deja la VPN apagada en lo guardado' (Json $e)
        Bien (Espera 20 { (Tcp '1.1.1.1' 443) -eq 'conecta' }) ('la red vuelve ({0:N1} s sin red)' -f ((Get-Date) - $sinRed).TotalSeconds)
        $eco = EcoConReintentos 5
        Bien (($eco.Ip -ne '') -and ($eco.Ip -eq $script:equipo)) "la dirección pública, la del equipo: $($eco.Ip)" "eco: $($eco.De)$(if ($eco.Fallos) { "; fallos: $($eco.Fallos)" })"
        NadaEnWindows 'tras la orden de emergencia'
        # Y al arrancar otra vez (el siguiente arranque del equipo), el servicio no vuelve a poner el bloqueo
        Plazo 120
        $arranca = (& $g service start 2>&1) -join ' '
        Bien (Espera 30 { (Servicio) -eq 'Running' }) 'el servicio, arrancado otra vez' (Recorta $arranca)
        $script:v5 = $null
        Bien (Espera 60 { $script:v5 = Panel 'GET' '/api/vpn'; $null -ne $script:v5 }) 'su panel contesta'
        Start-Sleep -Seconds 3
        $e = Estado
        $script:v5 = Panel 'GET' '/api/vpn'
        Bien ((Limpio $e) -and ($null -ne $script:v5) -and ($script:v5.fase -eq 'apagada')) 'al arrancar, no vuelve a poner el bloqueo: la VPN sigue apagada' "guardiana vpn estado: $(Json $e); GET /api/vpn: fase $($script:v5.fase)"
        Bien ((Tcp '1.1.1.1' 443) -eq 'conecta') 'y la red sale'
    }
} catch {
    Bien $false "la prueba se cortó: $($_.Exception.Message)" "$($_.InvocationInfo.PositionMessage)"
} finally {
    $script:fase = 'Al final'
    Write-Host ''
    Linea '=== Al final: la VPN apagada y el servicio quitado ==='
    Set-Content -LiteralPath $finArchivo -Value 'fin' -ErrorAction SilentlyContinue
    $null = & $g vpn apaga 2>&1
    $null = & $g service uninstall 2>&1
    Linea "     guardiana vpn estado: $(Json (Estado))"
    if ($vigilante) {
        if (-not $vigilante.WaitForExit(5000)) { Stop-Process -Id $vigilante.Id -Force -ErrorAction SilentlyContinue }
        $registro = if (Test-Path -LiteralPath $vigiaRegistro) { Get-Content -LiteralPath $vigiaRegistro } else { @() }
        foreach ($l in $registro) { Linea "     vigilante: $l" }
        $actuo = @($registro | Where-Object { $_ -like '*plazo vencido*' }).Count -gt 0
        Bien (-not $actuo) 'el vigilante no tuvo que actuar (ninguna fase pasó de su plazo)'
    }
}

# ---- El resumen
$mal = @($hechas | Where-Object { -not $_.Bien })
Write-Host ''
Linea ('{0} comprobaciones: {1} bien, {2} mal; {3} omitidas' -f $hechas.Count, ($hechas.Count - $mal.Count), $mal.Count, $omitidas.Count)
foreach ($m in $mal) { Linea "  MAL [$($m.Fase)] $($m.Que)" }
foreach ($o in $omitidas) { Linea "  OMITIDA $o" }

if ($env:GITHUB_STEP_SUMMARY) {
    $md = [System.Collections.Generic.List[string]]::new()
    $md.Add('### El modo estricto con el servicio de verdad y los servidores de verdad')
    $md.Add('')
    $md.Add(('{0} comprobaciones: **{1} bien, {2} mal**; {3} omitidas.' -f $hechas.Count, ($hechas.Count - $mal.Count), $mal.Count, $omitidas.Count))
    $md.Add('')
    $md.Add('| Fase | | Comprobación |')
    $md.Add('|---|---|---|')
    foreach ($h in $hechas) {
        $md.Add(('| {0} | {1} | {2} |' -f $h.Fase, $(if ($h.Bien) { 'BIEN' } else { '**MAL**' }), ($h.Que -replace '\|', '\|')))
    }
    foreach ($o in $omitidas) { $md.Add("| | omitida | $($o -replace '\|', '\|') |") }
    Add-Content -LiteralPath $env:GITHUB_STEP_SUMMARY -Value $md -Encoding utf8
}

# Las anotaciones de la ejecución (sus propiedades no llevan comas ni dos puntos sin escapar)
function Anota([string]$t) { return ($t -replace '%', '%25' -replace "`r", '%0D' -replace "`n", '%0A') }
function Titulo([string]$t) { return ((Anota $t) -replace ',', '%2C' -replace ':', '%3A') }
foreach ($m in $mal) { Write-Host ('::error title={0}::{1}' -f (Titulo "estricto de verdad, $($m.Fase)"), (Anota $m.Que)) }
foreach ($o in $omitidas) { Write-Host ('::notice title={0}::{1}' -f (Titulo 'estricto de verdad'), (Anota $o)) }
if ($mal.Count -gt 0) { exit 1 }
Write-Host ('::notice title={0}::{1}' -f (Titulo 'estricto de verdad'), (Anota "todas las comprobaciones, bien ($($hechas.Count))"))
exit 0
