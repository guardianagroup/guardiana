# GUARDIANA - vigilancia constante (1.0.45). Solo lectura: no cambia nada, no instala nada y no
# sale a la red. Lo que arranca solo, los ajustes de privacidad y si el antivirus y el cortafuegos
# estan encendidos, para compararlo con la vez anterior. Corre cada pocos minutos con prioridad baja.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding $false
$R = [ordered]@{ v = 1; arranque = $null; privacidad = $null; proteccion = $null; cuentas = [ordered]@{}; errores = [ordered]@{} }
function Prop($ruta, $nombre) { try { return (Get-ItemProperty -LiteralPath $ruta -Name $nombre -ErrorAction Stop).$nombre } catch { return $null } }
function Corta($s) { $s = ([string]$s) -replace '\s+', ' '; if ($s.Length -gt 300) { $s = $s.Substring(0, 300) }; return $s.Trim() }
$PERFILES = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList'
# The accounts whose registry is loaded now (signed in): only theirs can be read.
$USUARIOS = @()
try { $USUARIOS = @(Get-ChildItem 'Registry::HKEY_USERS' | Where-Object { $_.PSChildName -match '^S-1-5-21-[\d-]+$' } | ForEach-Object { $_.PSChildName }) } catch {}
foreach ($sid in $USUARIOS) { $p = Prop "$PERFILES\$sid" 'ProfileImagePath'; if ($p) { $R.cuentas[$sid] = (Split-Path -Leaf ([string]$p)) } }

# ---- What starts on its own: Run and RunOnce, the Startup folders, scheduled tasks, services.
try {
  $l = New-Object System.Collections.ArrayList
  $raices = @(@{ r = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion'; c = 'equipo' }, @{ r = 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion'; c = 'equipo' })
  foreach ($sid in $USUARIOS) { $raices += @{ r = "Registry::HKEY_USERS\$sid\Software\Microsoft\Windows\CurrentVersion"; c = $sid } }
  foreach ($x in $raices) {
    foreach ($k in 'Run', 'RunOnce') {
      $ruta = $x.r + '\' + $k
      if (-not (Test-Path -LiteralPath $ruta)) { continue }
      $it = Get-Item -LiteralPath $ruta
      foreach ($n in @($it.GetValueNames())) {
        if (-not $n) { continue }
        [void]$l.Add([ordered]@{ tipo = $k.ToLower(); donde = (($x.r -replace '^Registry::', '') + '\' + $k); nombre = [string]$n; ruta = (Corta $it.GetValue($n)); cuenta = $x.c })
      }
    }
  }
  $carpetas = @(@{ d = (Join-Path $env:ProgramData 'Microsoft\Windows\Start Menu\Programs\Startup'); c = 'equipo' })
  foreach ($sid in $USUARIOS) { $p = Prop "$PERFILES\$sid" 'ProfileImagePath'; if ($p) { $carpetas += @{ d = (Join-Path ([string]$p) 'AppData\Roaming\Microsoft\Windows\Start Menu\Programs\Startup'); c = $sid } } }
  foreach ($c in $carpetas) {
    if (-not (Test-Path -LiteralPath $c.d)) { continue }
    Get-ChildItem -LiteralPath $c.d -File -Force -ErrorAction SilentlyContinue | Where-Object { $_.Name -ne 'desktop.ini' } | ForEach-Object {
      [void]$l.Add([ordered]@{ tipo = 'carpeta'; donde = $c.d; nombre = [string]$_.Name; ruta = [string]$_.FullName; cuenta = $c.c })
    }
  }
  try {
    Get-ScheduledTask | ForEach-Object {
      $a = @($_.Actions | Where-Object { $_.Execute } | Select-Object -First 1)
      $orden = ''; if ($a.Count) { $orden = Corta ([string]$a[0].Execute + ' ' + [string]$a[0].Arguments) }
      [void]$l.Add([ordered]@{ tipo = 'tarea'; donde = [string]$_.TaskPath; nombre = [string]$_.TaskName; ruta = $orden; cuenta = 'equipo' })
    }
  } catch { $R.errores['tareas'] = (Corta $_.Exception.Message) }
  try {
    Get-CimInstance Win32_Service | ForEach-Object {
      [void]$l.Add([ordered]@{ tipo = 'servicio'; donde = [string]$_.StartMode; nombre = [string]$_.Name; visible = [string]$_.DisplayName; ruta = (Corta $_.PathName); cuenta = 'equipo' })
    }
  } catch { $R.errores['servicios'] = (Corta $_.Exception.Message) }
  $R.arranque = $l
} catch { $R.errores['arranque'] = (Corta $_.Exception.Message) }

# ---- The privacy settings the check-up reads, and the camera, microphone and location switches.
$P = [ordered]@{}
function Pon([string]$k, $v) { if ($null -eq $v) { $P[$k] = 'sin_fijar' } else { $P[$k] = [string]$v } }
try {
  Pon 'telemetria_politica' (Prop 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection' 'AllowTelemetry')
  Pon 'telemetria' (Prop 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\DataCollection' 'AllowTelemetry')
  try { Pon 'diagtrack_inicio' ([string](Get-Service DiagTrack).StartType) } catch {}
  $CAPS = @(@('webcam', 'camara'), @('microphone', 'microfono'), @('location', 'ubicacion'))
  foreach ($cap in $CAPS) { Pon ('acceso_' + $cap[1]) (Prop ('HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\' + $cap[0]) 'Value') }
  foreach ($sid in $USUARIOS) {
    $h = "Registry::HKEY_USERS\$sid\Software\Microsoft\Windows\CurrentVersion"
    Pon ('publicidad:' + $sid) (Prop "$h\AdvertisingInfo" 'Enabled')
    foreach ($cap in $CAPS) { Pon ('acceso_' + $cap[1] + ':' + $sid) (Prop ("$h\CapabilityAccessManager\ConsentStore\" + $cap[0]) 'Value') }
  }
  $R.privacidad = $P
} catch { $R.errores['privacidad'] = (Corta $_.Exception.Message) }

# ---- Whether the antivirus and the firewall are on, as Windows Security Center and Defender say.
try {
  $o = [ordered]@{ antivirus = $null; cortafuegos_otros = $null; defender = $null; cortafuegos = $null }
  try { $o.antivirus = @(Get-CimInstance -Namespace root/SecurityCenter2 -ClassName AntiVirusProduct | ForEach-Object { [ordered]@{ nombre = [string]$_.displayName; estado = [long]$_.productState } }) } catch {}
  try { $o.cortafuegos_otros = @(Get-CimInstance -Namespace root/SecurityCenter2 -ClassName FirewallProduct | ForEach-Object { [ordered]@{ nombre = [string]$_.displayName; estado = [long]$_.productState } }) } catch {}
  try {
    $m = Get-MpComputerStatus
    $o.defender = [ordered]@{ servicio = [bool]$m.AMServiceEnabled; antivirus = [bool]$m.AntivirusEnabled; tiempo_real = [bool]$m.RealTimeProtectionEnabled; manipulacion = [bool]$m.IsTamperProtected; modo = [string]$m.AMRunningMode }
  } catch {}
  try { $o.cortafuegos = @(Get-NetFirewallProfile | ForEach-Object { [ordered]@{ perfil = [string]$_.Name; activo = ([string]$_.Enabled -eq 'True') } }) } catch {}
  $R.proteccion = $o
} catch { $R.errores['proteccion'] = (Corta $_.Exception.Message) }

$R | ConvertTo-Json -Depth 6 -Compress
