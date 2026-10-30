# GUARDIANA - vigia (1.0.48), la mirada lenta. Solo lectura: no cambia nada, no instala nada y no
# sale a la red. Lo que Microsoft Defender dice de si mismo y de sus amenazas activas (WMI,
# root\Microsoft\Windows\Defender) y los antivirus que lista el Centro de seguridad de Windows.
# Una linea por hecho, campos separados por tabuladores (vigia.rs, de_salida_defensa):
#   D  servicio  antivirus  tiempo_real  modo
#   TA n                     (las amenazas se leyeron; n = cuantas)
#   T  nombre  gravedad      (una por amenaza activa)
#   AA ok                    (el Centro de seguridad contesto)
#   A  nombre  estado        (uno por antivirus; estado = productState)
#   E  parte  mensaje        (lo que fallo)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding $false
function L($x) { $s = ([string]$x) -replace '[\t\r\n]+', ' '; if ($s.Length -gt 200) { $s = $s.Substring(0, 200) }; return $s.Trim() }
try {
  $s = Get-CimInstance -Namespace root/Microsoft/Windows/Defender -ClassName MSFT_MpComputerStatus
  "D`t$([bool]$s.AMServiceEnabled)`t$([bool]$s.AntivirusEnabled)`t$([bool]$s.RealTimeProtectionEnabled)`t$(L $s.AMRunningMode)"
} catch { "E`tdefender`t$(L $_.Exception.Message)" }
try {
  $t = @(Get-CimInstance -Namespace root/Microsoft/Windows/Defender -ClassName MSFT_MpThreat | Where-Object { $_.IsActive })
  "TA`t$($t.Count)"
  foreach ($x in ($t | Select-Object -First 20)) { "T`t$(L $x.ThreatName)`t$([int]$x.SeverityID)" }
} catch { "E`tamenazas`t$(L $_.Exception.Message)" }
try {
  $a = @(Get-CimInstance -Namespace root/SecurityCenter2 -ClassName AntiVirusProduct)
  "AA`tok"
  foreach ($p in ($a | Select-Object -First 20)) { "A`t$(L $p.displayName)`t$([long]$p.productState)" }
} catch { "E`tcentro`t$(L $_.Exception.Message)" }
