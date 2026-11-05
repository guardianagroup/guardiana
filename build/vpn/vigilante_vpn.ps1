# build/vpn/vigilante_vpn.ps1: el vigilante de build/vpn/estricto_de_verdad.ps1, en un proceso aparte.
#
#   pwsh -File build/vpn/vigilante_vpn.ps1 -Exe <guardiana.exe> -Plazo <archivo> -Fin <archivo> -Registro <archivo>
#
# La prueba escribe en -Plazo, al empezar cada fase, hasta cuándo puede durar («ticks|fase», en UTC). Si una fase se
# pasa de su plazo (la prueba se colgó, o tarda más de la cuenta con la red cortada), el vigilante, pase lo que pase:
#  1. apaga la VPN y quita el bloqueo del modo estricto (`guardiana vpn apaga`: le pide al servicio que desconecte y lo
#     quita él mismo de Windows por sus claves fijas, también con el servicio parado);
#  2. para el servicio y, si no para en 30 s, mata su proceso (un túnel abierto se va con él);
#  3. otra vez `guardiana vpn apaga`, por si algo volvió a ponerlo mientras tanto;
# y lo apunta en -Registro, donde la prueba y el último paso del trabajo lo leen (que el vigilante actúe es un fallo de
# la prueba). Termina sin hacer nada en cuanto aparece -Fin. -Maximo es un tope total, por si el plazo no se escribe.
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Plazo,
    [Parameter(Mandatory = $true)][string]$Fin,
    [Parameter(Mandatory = $true)][string]$Registro,
    [int]$Maximo = 1500
)

$ErrorActionPreference = 'Continue'
$empezo = Get-Date

function Apunta([string]$t) {
    Add-Content -LiteralPath $Registro -Value ('{0:HH:mm:ss.f} {1}' -f (Get-Date), $t) -Encoding utf8
}

# guardiana.exe con `$argumentos`, como mucho `$segundos`: su código, o -1 si no terminó a tiempo (y se mata).
function Ejecuta([string[]]$argumentos, [int]$segundos = 60) {
    try {
        $p = Start-Process -FilePath $Exe -ArgumentList $argumentos -WindowStyle Hidden -PassThru
        # sin esto, .NET no guarda el código de salida de un proceso que ya terminó
        $null = $p.Handle
        if ($p.WaitForExit($segundos * 1000)) { return $p.ExitCode }
        Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
        return -1
    } catch {
        return -2
    }
}

# El plazo, leído sin estorbar a la prueba, que lo cambia poniendo otro archivo en su sitio; '' si no se puede leer.
function LeePlazo {
    try {
        $compartido = [System.IO.FileShare]::ReadWrite -bor [System.IO.FileShare]::Delete
        $fs = [System.IO.FileStream]::new($Plazo, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, $compartido)
        try { return [System.IO.StreamReader]::new($fs).ReadToEnd() } finally { $fs.Dispose() }
    } catch {
        return ''
    }
}

Apunta "vigilando (proceso $PID)"
$fase = ''
while ($true) {
    if (Test-Path -LiteralPath $Fin) {
        Apunta 'la prueba terminó: nada que hacer'
        exit 0
    }
    $vence = [long]0
    $linea = LeePlazo
    if ($linea) {
        $partes = $linea.Trim() -split '\|', 2
        $null = [long]::TryParse($partes[0], [ref]$vence)
        if ($partes.Count -gt 1) { $fase = $partes[1] }
    }
    # un plazo de hace más de un día no es un plazo (un archivo a medio escribir): se lee otra vez en un segundo
    $ahora = [DateTime]::UtcNow.Ticks
    if ($vence -gt $ahora - [TimeSpan]::TicksPerDay -and $ahora -gt $vence) { break }
    if (((Get-Date) - $empezo).TotalSeconds -gt $Maximo) {
        $fase = "$fase (tope total de $Maximo s)"
        break
    }
    Start-Sleep -Seconds 1
}

Apunta "plazo vencido en «$fase»: se apaga la VPN, se quita el bloqueo y se para el servicio"
Apunta "guardiana vpn apaga: código $(Ejecuta @('vpn', 'apaga'))"
$null = & sc.exe stop guardiana 2>&1
$hasta = (Get-Date).AddSeconds(30)
while ((Get-Date) -lt $hasta -and "$((Get-Service -Name guardiana -ErrorAction SilentlyContinue).Status)" -notin @('Stopped', '')) {
    Start-Sleep -Milliseconds 500
}
$s = Get-CimInstance Win32_Service -Filter "Name='guardiana'" -ErrorAction SilentlyContinue
if ($s -and $s.ProcessId -gt 0) {
    $null = & taskkill.exe /F /PID $s.ProcessId 2>&1
    Apunta "el servicio no paraba: taskkill /F /PID $($s.ProcessId)"
}
Apunta "el servicio: $((Get-Service -Name guardiana -ErrorAction SilentlyContinue).Status)"
Apunta "guardiana vpn apaga, otra vez: código $(Ejecuta @('vpn', 'apaga'))"
Apunta "guardiana vpn estado: $((& $Exe vpn estado --json 2>&1) -join ' ')"
exit 0
