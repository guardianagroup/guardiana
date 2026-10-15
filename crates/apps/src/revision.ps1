# GUARDIANA - revision del equipo (1.0.24). Solo lectura: no cambia nada, no instala nada.
# Lo unico que sale a la red son tres preguntas DNS por nombres publicos (dns.google,
# one.one.one.one y un nombre inventado bajo example.com) a los servidores DNS que usa el equipo.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding $false
$R = [ordered]@{ v = 1 }
$T = [ordered]@{}
function Ms($d) { if ($null -eq $d) { return $null }; try { return ([DateTimeOffset][DateTime]$d).ToUnixTimeMilliseconds() } catch { return $null } }
function MsFt($ft) { try { $n = [long]$ft; if ($n -le 0) { return 0 }; return ([DateTimeOffset][DateTime]::FromFileTimeUtc($n)).ToUnixTimeMilliseconds() } catch { return $null } }
function Corta($s) { $s = ([string]$s) -replace '\s+', ' '; if ($s.Length -gt 200) { $s = $s.Substring(0, 200) }; return $s }
# What it is doing, live, for the page (1.0.31): one line per step on standard error, which the
# panel reads while the review runs. Only what is really being read, never the answers' contents.
function Paso([string]$tipo, [string]$texto) { try { [Console]::Error.WriteLine('@@' + $tipo + "`t" + (($texto -replace '[\r\n\t]', ' ').Trim())) } catch {} }
function Seccion([string]$nombre, [scriptblock]$sb) {
  Paso 'seccion' $nombre
  $w = [Diagnostics.Stopwatch]::StartNew()
  try { $R[$nombre] = (& $sb) } catch { $R[$nombre] = [ordered]@{ error = (Corta $_.Exception.Message) } }
  $T[$nombre] = $w.ElapsedMilliseconds
  Paso 'hecho' ($nombre + "`t" + $w.ElapsedMilliseconds)
}
function Prop($ruta, $nombre) { try { return (Get-ItemProperty -LiteralPath $ruta -Name $nombre -ErrorAction Stop).$nombre } catch { return $null } }
$USUARIOS = @()
try { $USUARIOS = @(Get-ChildItem 'Registry::HKEY_USERS' | Where-Object { $_.PSChildName -match '^S-1-5-21-[\d-]+$' } | ForEach-Object { $_.PSChildName }) } catch {}
$FIRMAS = @{}
function RutaDe([string]$c) {
  if (-not $c) { return $null }
  $c = [Environment]::ExpandEnvironmentVariables($c.Trim())
  if ($c.StartsWith('"')) { $i = $c.IndexOf('"', 1); if ($i -gt 1) { return $c.Substring(1, $i - 1) } }
  $m = [regex]::Match($c, '^(.+?\.(exe|dll|sys|com|bat|cmd|ps1|vbs|js|scr|msi|lnk))(\s|,|$)', 'IgnoreCase')
  if ($m.Success) { return $m.Groups[1].Value }
  return ($c -split ' ')[0]
}
function Firma([string]$ruta) {
  $f = [ordered]@{ existe = $false; estado = $null; firmante = $null }
  if (-not $ruta) { return $f }
  if ($ruta.StartsWith('\??\')) { $ruta = $ruta.Substring(4) }
  if ($ruta -match '^\\SystemRoot\\') { $ruta = $env:SystemRoot + $ruta.Substring(11) }
  elseif ($ruta -match '^(?i)system32\\') { $ruta = Join-Path $env:SystemRoot $ruta }
  if ($FIRMAS.ContainsKey($ruta)) { return $FIRMAS[$ruta] }
  try {
    if (Test-Path -LiteralPath $ruta -PathType Leaf) {
      $f.existe = $true
      Paso 'firma' (Split-Path -Leaf $ruta)
      $s = Get-AuthenticodeSignature -LiteralPath $ruta
      $f.estado = [string]$s.Status
      if ($s.SignerCertificate) { $f.firmante = $s.SignerCertificate.GetNameInfo([Security.Cryptography.X509Certificates.X509NameType]::SimpleName, $false) }
    }
  } catch {}
  $FIRMAS[$ruta] = $f
  return $f
}
function EsDeMicrosoft($f) { return ($f.estado -eq 'Valid' -and ([string]$f.firmante) -like 'Microsoft*') }

Seccion 'equipo' {
  $cs = Get-CimInstance Win32_ComputerSystem
  $os = Get-CimInstance Win32_OperatingSystem
  $b = $null; try { $b = Get-CimInstance Win32_BIOS } catch {}
  $cv = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
  [ordered]@{ fabricante = [string]$cs.Manufacturer; modelo = [string]$cs.Model; sistema = [string]$os.Caption; version = [string]$os.Version
    compilacion = [string]$os.BuildNumber; ubr = (Prop $cv 'UBR'); edicion = (Prop $cv 'EditionID'); display = (Prop $cv 'DisplayVersion')
    arranque = (Ms $os.LastBootUpTime); instalado = (Ms $os.InstallDate); bios = [string]$b.SMBIOSBIOSVersion; bios_fecha = (Ms $b.ReleaseDate); bios_fabricante = [string]$b.Manufacturer }
}

Seccion 'actualizaciones' {
  $o = [ordered]@{ ultima_instalacion = $null; ultima_busqueda = $null; ultimo_parche = $null; reinicio_pendiente = $false; servicio = $null; pausada_hasta = $null; politica_sin_auto = $false }
  try { $au = New-Object -ComObject Microsoft.Update.AutoUpdate; $o.ultima_instalacion = Ms $au.Results.LastInstallationSuccessDate; $o.ultima_busqueda = Ms $au.Results.LastSearchSuccessDate } catch {}
  try { $hf = @(Get-HotFix | Where-Object { $_.InstalledOn } | Sort-Object InstalledOn -Descending | Select-Object -First 1); if ($hf.Count) { $o.ultimo_parche = Ms $hf[0].InstalledOn } } catch {}
  $o.reinicio_pendiente = (Test-Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update\RebootRequired')
  try { $o.servicio = [string](Get-Service wuauserv).StartType } catch {}
  $p = Prop 'HKLM:\SOFTWARE\Microsoft\WindowsUpdate\UX\Settings' 'PauseUpdatesExpiryTime'; if ($p) { try { $o.pausada_hasta = Ms ([DateTime]::Parse($p, [Globalization.CultureInfo]::InvariantCulture)) } catch {} }
  $o.politica_sin_auto = ((Prop 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU' 'NoAutoUpdate') -eq 1)
  $o
}

Seccion 'defensa' {
  $o = [ordered]@{ defender = $null; productos = $null }
  try {
    $m = Get-MpComputerStatus
    $o.defender = [ordered]@{ servicio = [bool]$m.AMServiceEnabled; antivirus = [bool]$m.AntivirusEnabled; tiempo_real = [bool]$m.RealTimeProtectionEnabled
      firmas = (Ms $m.AntivirusSignatureLastUpdated); firmas_dias = [long]$m.AntivirusSignatureAge; rapido_dias = [long]$m.QuickScanAge
      manipulacion = [bool]$m.IsTamperProtected; modo = [string]$m.AMRunningMode }
  } catch {}
  try { $o.productos = @(Get-CimInstance -Namespace root/SecurityCenter2 -ClassName AntiVirusProduct | ForEach-Object { [ordered]@{ nombre = [string]$_.displayName; estado = [long]$_.productState } }) } catch {}
  $o
}

Seccion 'sistema' {
  $o = [ordered]@{ cortafuegos = $null; uac = $null; smb1 = $null; arranque_seguro = $null; tpm = $null; cifrado = $null; integridad_memoria = $null; unidad = $env:SystemDrive }
  try { $o.cortafuegos = @(Get-NetFirewallProfile | ForEach-Object { [ordered]@{ perfil = [string]$_.Name; activo = ([string]$_.Enabled -eq 'True'); entrada = [string]$_.DefaultInboundAction } }) } catch {}
  $o.uac = Prop 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System' 'EnableLUA'
  try { $o.smb1 = [bool](Get-SmbServerConfiguration).EnableSMB1Protocol } catch {}
  if (Test-Path 'HKLM:\SYSTEM\CurrentControlSet\Control\SecureBoot\State') { $o.arranque_seguro = ((Prop 'HKLM:\SYSTEM\CurrentControlSet\Control\SecureBoot\State' 'UEFISecureBootEnabled') -eq 1) } else { $o.arranque_seguro = 'sin_uefi' }
  try {
    $t = @(Get-CimInstance -Namespace root/cimv2/Security/MicrosoftTpm -ClassName Win32_Tpm)
    if ($t.Count) { $o.tpm = [ordered]@{ activo = [bool]$t[0].IsEnabled_InitialValue; version = ([string]$t[0].SpecVersion -split ',')[0].Trim() } } else { $o.tpm = 'ninguno' }
  } catch {}
  try { $o.cifrado = @(Get-CimInstance -Namespace root/cimv2/security/microsoftvolumeencryption -ClassName Win32_EncryptableVolume | ForEach-Object { [ordered]@{ unidad = [string]$_.DriveLetter; proteccion = [int]$_.ProtectionStatus } }) } catch {}
  try { $dg = Get-CimInstance -Namespace root/Microsoft/Windows/DeviceGuard -ClassName Win32_DeviceGuard; $o.integridad_memoria = (@($dg.SecurityServicesRunning) -contains 2) } catch {}
  $o
}

Seccion 'acceso' {
  $o = [ordered]@{ cuentas = $null; rdp = $null; rdp_nla = $null; asistencia = $null; autologon = $false; autologon_clave = $false; fallidos_7d = $null; rdp_entradas_30d = $null; creadas_30d = $null; puertos = $null; carpetas = $null }
  $adm = $null
  try { $adm = @(Get-LocalGroupMember -SID 'S-1-5-32-544' | ForEach-Object { [string]$_.SID }) } catch {}
  try { $o.cuentas = @(Get-LocalUser | ForEach-Object { $es = $null; if ($null -ne $adm) { $es = ($adm -contains [string]$_.SID) }
    [ordered]@{ nombre = [string]$_.Name; sid = [string]$_.SID; activa = [bool]$_.Enabled; admin = $es; ultima = (Ms $_.LastLogon); clave_opcional = (-not $_.PasswordRequired); origen = [string]$_.PrincipalSource } }) } catch {}
  $ts = 'HKLM:\SYSTEM\CurrentControlSet\Control\Terminal Server'
  $d = Prop $ts 'fDenyTSConnections'; if ($null -ne $d) { $o.rdp = ($d -eq 0) }
  $n = Prop "$ts\WinStations\RDP-Tcp" 'UserAuthentication'; if ($null -ne $n) { $o.rdp_nla = ($n -eq 1) }
  $a = Prop 'HKLM:\SYSTEM\CurrentControlSet\Control\Remote Assistance' 'fAllowToGetHelp'; if ($null -ne $a) { $o.asistencia = ($a -eq 1) }
  $wl = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon'
  $o.autologon = ([string](Prop $wl 'AutoAdminLogon') -eq '1')
  $o.autologon_clave = ($null -ne (Prop $wl 'DefaultPassword'))
  try { $o.fallidos_7d = @(Get-WinEvent -FilterHashtable @{ LogName = 'Security'; Id = 4625; StartTime = (Get-Date).AddDays(-7) } -MaxEvents 5000 -ErrorAction Stop).Count } catch { if ($_.FullyQualifiedErrorId -like 'NoMatchingEventsFound*') { $o.fallidos_7d = 0 } }
  try { $o.rdp_entradas_30d = @(Get-WinEvent -LogName Security -FilterXPath "*[System[(EventID=4624) and TimeCreated[timediff(@SystemTime) <= 2592000000]] and EventData[Data[@Name='LogonType']='10']]" -MaxEvents 1000 -ErrorAction Stop).Count } catch { if ($_.FullyQualifiedErrorId -like 'NoMatchingEventsFound*') { $o.rdp_entradas_30d = 0 } }
  try { $o.creadas_30d = @(Get-WinEvent -FilterHashtable @{ LogName = 'Security'; Id = 4720; StartTime = (Get-Date).AddDays(-30) } -MaxEvents 50 -ErrorAction Stop | ForEach-Object { $x = [xml]$_.ToXml(); [ordered]@{ nombre = [string](($x.Event.EventData.Data | Where-Object { $_.Name -eq 'TargetUserName' }).'#text'); cuando = (Ms $_.TimeCreated) } }) } catch { if ($_.FullyQualifiedErrorId -like 'NoMatchingEventsFound*') { $o.creadas_30d = @() } }
  try {
    $procs = @{}; Get-Process | ForEach-Object { $procs[[int]$_.Id] = [string]$_.ProcessName }
    $vistos = @{}; $l = @()
    Get-NetTCPConnection -State Listen | ForEach-Object {
      $ad = [string]$_.LocalAddress
      $alc = 'red'; if ($ad -eq '::1' -or $ad.StartsWith('127.')) { $alc = 'equipo' }
      $pr = $procs[[int]$_.OwningProcess]; if ([int]$_.OwningProcess -eq 4) { $pr = 'System' }
      $k = "$($_.LocalPort)|$alc|$pr"
      if (-not $vistos.ContainsKey($k)) { $vistos[$k] = 1; $l += [ordered]@{ puerto = [int]$_.LocalPort; alcance = $alc; programa = $pr } }
    }
    $o.puertos = $l
  } catch {}
  try { $o.carpetas = @(Get-SmbShare | Where-Object { -not $_.Special } | ForEach-Object { [ordered]@{ nombre = [string]$_.Name; ruta = [string]$_.Path } }) } catch {}
  $o
}

Seccion 'arranque' {
  $o = [ordered]@{ inicio = $null; tareas = $null; servicios = $null; controladores = $null }
  try {
    $o.inicio = @(Get-CimInstance Win32_StartupCommand | ForEach-Object { $r = RutaDe $_.Command; $f = Firma $r
      [ordered]@{ nombre = [string]$_.Name; orden = (Corta $_.Command); ruta = $r; donde = [string]$_.Location; usuario = [string]$_.User; existe = $f.existe; firma = $f.estado; firmante = $f.firmante } })
    Paso 'cuenta' ('inicio' + "`t" + @($o.inicio).Count)
  } catch {}
  try {
    $o.tareas = @(Get-ScheduledTask | Where-Object { $_.TaskPath -notlike '\Microsoft\*' } | ForEach-Object {
      $acs = @($_.Actions | Where-Object { $_.Execute })
      $r = $null; $args2 = $null; $f = Firma $null
      foreach ($a in $acs) {
        $ra = RutaDe ('"' + ([string]$a.Execute).Trim('"') + '"'); $fa = Firma $ra
        if ($null -eq $r -or -not $fa.existe -or ($fa.estado -ne 'Valid' -and $f.estado -eq 'Valid')) { $r = $ra; $f = $fa; $args2 = Corta $a.Arguments }
      }
      $dis = @($_.Triggers | ForEach-Object { ([string]$_.CimClass.CimClassName) -replace '^MSFT_Task', '' -replace 'Trigger$', '' })
      [ordered]@{ nombre = [string]$_.TaskName; carpeta = [string]$_.TaskPath; estado = [string]$_.State; autor = [string]$_.Author; ruta = $r; args = $args2; disparadores = $dis; oculta = [bool]$_.Settings.Hidden; existe = $f.existe; firma = $f.estado; firmante = $f.firmante } })
    Paso 'cuenta' ('tareas' + "`t" + @($o.tareas).Count)
  } catch {}
  try {
    $o.servicios = @(Get-CimInstance Win32_Service | ForEach-Object { $r = RutaDe $_.PathName
      if ($r -match '(?i)\\svchost\.exe$') { $dll = Prop "HKLM:\SYSTEM\CurrentControlSet\Services\$($_.Name)\Parameters" 'ServiceDll'; if ($dll) { $r = [Environment]::ExpandEnvironmentVariables([string]$dll) } }
      $f = Firma $r
      if (-not (EsDeMicrosoft $f)) { [ordered]@{ nombre = [string]$_.Name; visible = [string]$_.DisplayName; estado = [string]$_.State; inicio = [string]$_.StartMode; ruta = $r; existe = $f.existe; firma = $f.estado; firmante = $f.firmante } } })
    Paso 'cuenta' ('servicios' + "`t" + @($o.servicios).Count)
  } catch {}
  try {
    $o.controladores = @(Get-CimInstance Win32_SystemDriver | ForEach-Object { $r = RutaDe $_.PathName; $f = Firma $r
      if ($f.existe -and -not (EsDeMicrosoft $f)) { [ordered]@{ nombre = [string]$_.Name; visible = [string]$_.DisplayName; estado = [string]$_.State; inicio = [string]$_.StartMode; ruta = $r; firma = $f.estado; firmante = $f.firmante } } })
    Paso 'cuenta' ('controladores' + "`t" + @($o.controladores).Count)
  } catch {}
  $o
}

Seccion 'salida' {
  $o = [ordered]@{ dns = $null; prueba_dns = $null; adaptadores = $null; vpn = $null; proxy = @(); proxy_sistema = $null; hosts = $null; wifi = $null; perfiles = $null }
  try { $o.dns = @(Get-DnsClientServerAddress | Where-Object { $_.ServerAddresses } | ForEach-Object { [ordered]@{ interfaz = [string]$_.InterfaceAlias; servidores = @($_.ServerAddresses | ForEach-Object { [string]$_ }) } }) } catch {}
  try {
    $srv = @($o.dns | ForEach-Object { $_.servidores } | Where-Object { $_ -notmatch '^(127\.|::1$|fec0:)' } | Select-Object -Unique)
    $falso = 'guardiana-' + [guid]::NewGuid().ToString('N').Substring(0, 12) + '.example.com'
    $o.prueba_dns = @($srv | Select-Object -First 4 | ForEach-Object {
      $s = $_; $p = [ordered]@{ servidor = $s; google = $null; cloudflare = $null; inexistente = $null }
      Paso 'dns' 'dns.google'; try { $p.google = @(Resolve-DnsName -Name 'dns.google' -Type A -Server $s -DnsOnly -NoHostsFile -QuickTimeout -ErrorAction Stop | Where-Object { $_.IPAddress } | ForEach-Object { [string]$_.IPAddress }) } catch { $p.google = 'error' }
      Paso 'dns' 'one.one.one.one'; try { $p.cloudflare = @(Resolve-DnsName -Name 'one.one.one.one' -Type A -Server $s -DnsOnly -NoHostsFile -QuickTimeout -ErrorAction Stop | Where-Object { $_.IPAddress } | ForEach-Object { [string]$_.IPAddress }) } catch { $p.cloudflare = 'error' }
      Paso 'dns' $falso; try { $p.inexistente = @(Resolve-DnsName -Name $falso -Type A -Server $s -DnsOnly -NoHostsFile -QuickTimeout -ErrorAction Stop | Where-Object { $_.IPAddress } | ForEach-Object { [string]$_.IPAddress }) } catch { if ([string]$_.FullyQualifiedErrorId -like 'DNS_ERROR_RCODE_NAME_ERROR*') { $p.inexistente = @() } else { $p.inexistente = 'error' } }
      $p })
  } catch {}
  try { $o.adaptadores = @(Get-NetAdapter | Where-Object { $_.Status -eq 'Up' } | ForEach-Object { [ordered]@{ nombre = [string]$_.Name; desc = [string]$_.InterfaceDescription; virtual = [bool]$_.Virtual } }) } catch {}
  try { $o.vpn = @(@(Get-VpnConnection -AllUserConnection -ErrorAction SilentlyContinue) + @(Get-VpnConnection -ErrorAction SilentlyContinue) | Where-Object { $_ } | ForEach-Object { [ordered]@{ nombre = [string]$_.Name; conectada = ([string]$_.ConnectionStatus -eq 'Connected') } }) } catch {}
  foreach ($sid in $USUARIOS) {
    $is = "Registry::HKEY_USERS\$sid\Software\Microsoft\Windows\CurrentVersion\Internet Settings"
    $en = Prop $is 'ProxyEnable'; $sv = Prop $is 'ProxyServer'; $pac = Prop $is 'AutoConfigURL'
    if ($en -eq 1 -or $pac) { $o.proxy += [ordered]@{ sid = $sid; servidor = $(if ($en -eq 1) { [string]$sv } else { $null }); pac = [string]$pac } }
  }
  try { $b = Prop 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Internet Settings\Connections' 'WinHttpSettings'; if ($b -and $b.Length -ge 16) { $n = [BitConverter]::ToInt32($b, 12); if ($n -gt 0 -and (16 + $n) -le $b.Length) { $px = [Text.Encoding]::ASCII.GetString($b, 16, $n); if ($px -match '^[\w\.\-:;=\[\]/<>]+$') { $o.proxy_sistema = $px } } } } catch {}
  try {
    $o.hosts = @(Get-Content -LiteralPath "$env:SystemRoot\System32\drivers\etc\hosts" -ErrorAction Stop | ForEach-Object { ($_ -split '#')[0].Trim() } | Where-Object { $_ } | ForEach-Object { $p = $_ -split '\s+'; if ($p.Count -ge 2) { [ordered]@{ direccion = $p[0]; nombres = @($p[1..($p.Count - 1)]) } } })
  } catch {}
  try {
    $w = @(netsh.exe wlan show interfaces 2>$null)
    $ssid = $null; $aut = @(); $cif = @()
    foreach ($ln in $w) {
      $i = $ln.IndexOf(':'); if ($i -lt 0) { continue }
      $k = $ln.Substring(0, $i).Trim(); $v = $ln.Substring($i + 1).Trim()
      if ($k -match '^SSID$') { $ssid = $v }
      if ($v -match '^(WPA3|WPA2|WPA|WEP|Open|Abierta|Aberta|Ouvrir|Offen)') { $aut += $v }
      if ($v -match '^(CCMP|GCMP|TKIP|WEP|None|Ninguno|Nenhum)') { $cif += $v }
    }
    if ($ssid) { $o.wifi = [ordered]@{ conectado = $true; autenticacion = ($aut | Select-Object -First 1); cifrado = ($cif | Select-Object -First 1) } } else { $o.wifi = [ordered]@{ conectado = $false } }
  } catch {}
  try { $o.perfiles = @(Get-NetConnectionProfile | ForEach-Object { [ordered]@{ interfaz = [string]$_.InterfaceAlias; categoria = [string]$_.NetworkCategory } }) } catch {}
  $o
}

Seccion 'certificados' {
  # The Cert: PowerShell drive is absent in some service/non-interactive sessions. .NET opens
  # the same Windows stores directly and works there too.
  $authStore = [Security.Cryptography.X509Certificates.X509Store]::new('AuthRoot', 'LocalMachine')
  $rootStore = [Security.Cryptography.X509Certificates.X509Store]::new('Root', 'LocalMachine')
  try {
    $authStore.Open([Security.Cryptography.X509Certificates.OpenFlags]::ReadOnly)
    $rootStore.Open([Security.Cryptography.X509Certificates.OpenFlags]::ReadOnly)
    $auth = @{}; @($authStore.Certificates) | ForEach-Object { $auth[$_.Thumbprint] = 1 }
    $todos = @($rootStore.Certificates)
  } finally {
    $authStore.Close(); $rootStore.Close()
  }
  $maq = @{}; $todos | ForEach-Object { $maq[$_.Thumbprint] = 1 }
  $fuera = @($todos | Where-Object { -not $auth.ContainsKey($_.Thumbprint) } | ForEach-Object {
    [ordered]@{ huella = [string]$_.Thumbprint; nombre = $_.GetNameInfo([Security.Cryptography.X509Certificates.X509NameType]::SimpleName, $false); emisor = $_.GetNameInfo([Security.Cryptography.X509Certificates.X509NameType]::SimpleName, $true); desde = (Ms $_.NotBefore); hasta = (Ms $_.NotAfter) } })
  $usr = @()
  foreach ($sid in $USUARIOS) {
    $k = "Registry::HKEY_USERS\$sid\Software\Microsoft\SystemCertificates\Root\Certificates"
    if (Test-Path $k) { Get-ChildItem $k | ForEach-Object { if (-not $maq.ContainsKey($_.PSChildName) -and -not $auth.ContainsKey($_.PSChildName)) { $usr += [ordered]@{ sid = $sid; huella = [string]$_.PSChildName } } } }
  }
  [ordered]@{ total = $todos.Count; programa_microsoft = $auth.Count; fuera = $fuera; de_usuario = $usr }
}

function Usos([string]$raiz, [string]$quien) {
  $out = @()
  foreach ($cap in 'webcam', 'microphone', 'location') {
    $base = "$raiz\Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\$cap"
    if (-not (Test-Path $base)) { continue }
    $ks = @(Get-ChildItem $base -ErrorAction SilentlyContinue | Where-Object { $_.PSChildName -ne 'NonPackaged' })
    if (Test-Path "$base\NonPackaged") { $ks += @(Get-ChildItem "$base\NonPackaged" -ErrorAction SilentlyContinue) }
    foreach ($k in $ks) {
      $p = Get-ItemProperty -LiteralPath $k.PSPath -ErrorAction SilentlyContinue
      if ($p -and $p.LastUsedTimeStart) { $out += [ordered]@{ cap = $cap; app = ([string]$k.PSChildName -replace '#', '\'); desde = (MsFt $p.LastUsedTimeStart); hasta = (MsFt $p.LastUsedTimeStop); quien = $quien } }
    }
  }
  return ,$out
}

Seccion 'privacidad' {
  $o = [ordered]@{ telemetria_politica = $null; telemetria = $null; diagtrack = $null; publicidad = @(); usos = @(); usb = $null; bluetooth = $null }
  $o.telemetria_politica = Prop 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\DataCollection' 'AllowTelemetry'
  $o.telemetria = Prop 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\DataCollection' 'AllowTelemetry'
  try { $s = Get-Service DiagTrack; $o.diagtrack = [ordered]@{ estado = [string]$s.Status; inicio = [string]$s.StartType } } catch {}
  foreach ($sid in $USUARIOS) {
    $e = Prop "Registry::HKEY_USERS\$sid\Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo" 'Enabled'
    $o.publicidad += [ordered]@{ sid = $sid; activo = ($e -eq 1) }
    $o.usos += (Usos "Registry::HKEY_USERS\$sid" $sid)
  }
  $o.usos += (Usos 'HKLM:' 'equipo')
  try { $o.usb = @(Get-ChildItem 'HKLM:\SYSTEM\CurrentControlSet\Enum\USBSTOR' -ErrorAction Stop | ForEach-Object { Get-ChildItem $_.PSPath } | ForEach-Object { [ordered]@{ nombre = [string](Prop $_.PSPath 'FriendlyName'); id = [string]$_.PSChildName } }) } catch { $o.usb = @() }
  try { $o.bluetooth = @(Get-ChildItem 'HKLM:\SYSTEM\CurrentControlSet\Services\BTHPORT\Parameters\Devices' -ErrorAction Stop | ForEach-Object { $n = Prop $_.PSPath 'Name'; if ($n) { ([Text.Encoding]::UTF8.GetString([byte[]]$n)).Trim([char]0) } } | Where-Object { $_ }) } catch { $o.bluetooth = @() }
  $o
}

Seccion 'programas' {
  $claves = @('HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*', 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*') + @($USUARIOS | ForEach-Object { "Registry::HKEY_USERS\$_\Software\Microsoft\Windows\CurrentVersion\Uninstall\*" })
  $vistos = @{}; $l = @()
  foreach ($c in $claves) {
    Get-ItemProperty $c -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName -and $_.SystemComponent -ne 1 -and -not $_.ParentKeyName } | ForEach-Object {
      $k = "$($_.DisplayName)|$($_.Publisher)"
      if (-not $vistos.ContainsKey($k)) { $vistos[$k] = 1; $l += [ordered]@{ nombre = [string]$_.DisplayName; editor = [string]$_.Publisher; version = [string]$_.DisplayVersion; instalado = [string]$_.InstallDate } }
    }
  }
  $ax = @()
  try { $ax = @(Get-AppxPackage -AllUsers -ErrorAction Stop | Where-Object { -not $_.IsFramework -and [string]$_.SignatureKind -ne 'System' } | ForEach-Object { [ordered]@{ nombre = [string]$_.Name; editor = (([string]$_.Publisher -split ',')[0] -replace '^CN=', ''); version = [string]$_.Version; tienda = ([string]$_.SignatureKind -eq 'Store') } }) } catch {}
  [ordered]@{ instalados = $l; appx = $ax }
}

Seccion 'defender_sucesos' {
  $l = @()
  try {
    $l = @(Get-WinEvent -FilterHashtable @{ LogName = 'Microsoft-Windows-Windows Defender/Operational'; Id = 1000, 1001, 1002, 1116, 1117, 2000, 2001, 5001; StartTime = (Get-Date).AddDays(-7) } -MaxEvents 300 -ErrorAction Stop | ForEach-Object {
      $x = [xml]$_.ToXml(); $d = @{}; foreach ($e in @($x.Event.EventData.Data)) { if ($e.Name) { $d[[string]$e.Name] = [string]$e.'#text' } }
      [ordered]@{ id = [int]$_.Id; cuando = (Ms $_.TimeCreated); amenaza = $d['Threat Name']; gravedad = $d['Severity Name']; accion = $d['Action Name']; analisis = $d['Scan Type'] } })
  } catch {}
  [ordered]@{ sucesos = $l }
}

$R.tiempos = $T
$R | ConvertTo-Json -Depth 8 -Compress
