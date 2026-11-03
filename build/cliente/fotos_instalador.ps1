# build/cliente/fotos_instalador.ps1: lo que ve una persona al instalar GUARDIANA, foto a foto.
#
#   powershell -File build/cliente/fotos_instalador.ps1 -Instalador <.msi o .exe> [-Antes <.msi>] [-Nombre fotos]
#
# Abre el instalador como un doble clic (con todas sus pantallas), pulsa lo que pulsaría una persona
# (aceptar los términos, «Instalar», y «Aceptar» si Windows pregunta por programas abiertos) y, mientras
# tanto, saca una foto de la pantalla entera cada 0,15 s y apunta cada ventana que aparece (título, clase
# y programa), aunque dure un instante: así se ven las «cosas raras» (dueño, 2 Nov 2026: «Tiene que ser
# más limpio… una pantalla profesional… nada de tirón ni cosas raras»).
# Desde el 3 nov 2026 (el dueño: «hay que darle a la tecla finalizar, si no, no funciona… quiero que lo
# quites») no hay nada que pulsar al final. Sale bien solo si, antes de -Segundos, la ventana del instalador
# se cierra sola (su proceso termina sin que nadie pulse nada) y se abre una ventana de GUARDIANA de un
# proceso nuevo (la de ZERO, clase GuardianaZeroVentana, o con ZERO=0 el panel en el navegador). Si el
# instalador se queda en una última pantalla («Finalizar», «Cerrar»…), es un fallo: se apunta, se deja para
# la foto y se pulsa para que termine. Si no sale bien, el script termina con error.
# Solo se pulsa dentro de las ventanas del instalador (msiexec): al actualizar, ZERO de la versión anterior
# está abierto con su página, y en la vuelta del 3 nov 2026 se pulsó un «Siguiente» de esa página.
# Con -Antes instala primero esa versión en silencio y deja abiertos ZERO y sus avisos, como los tiene
# quien actualiza. Al final escribe en el registro las fotos distintas entre sí (JPEG en base64, líneas
# «FOTO|nombre|n|datos») para leerlas sin descargar artefactos, y la lista de ventanas.
# Escrito el 2 Nov 2026 para la 1.0.54.
# Ojo: en PowerShell $Antes y $antes (o $Nombre y $nombre) son la misma variable: dentro, otros nombres.
param(
    [Parameter(Mandatory = $true)] [string]$Instalador,
    [string]$Antes = "",
    [string]$Nombre = "fotos",
    [int]$Segundos = 300,
    [int]$MaxFotos = 60
)
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Windows.Forms, System.Drawing, UIAutomationClient, UIAutomationTypes

$dir = Join-Path $env:RUNNER_TEMP $Nombre
New-Item -ItemType Directory -Force -Path $dir | Out-Null
$alto = Join-Path $dir "ALTO"
$eventos = Join-Path $dir "eventos.txt"

function Ahora { [DateTime]::UtcNow.ToString("HH:mm:ss.fff") }
# Write-Host y no Write-Output: lo que escribe no se mezcla con lo que devuelven las funciones (con Write-Output, un «no se pudo
# pulsar» devolvía también texto y contaba como si se hubiera pulsado)
function Evento([string]$t) { $l = "$(Ahora)  $t"; Add-Content -Path $eventos -Value $l -Encoding UTF8; Write-Host $l }

$zero = "C:\Program Files\Guardiana\ZERO\GUARDIANA ZERO.exe"
if ($Antes) {
    Evento "instalando en silencio la versión de antes: $Antes"
    $p = Start-Process msiexec.exe -ArgumentList "/i `"$Antes`" /qn /norestart /l*v `"$dir\antes.log`"" -Wait -PassThru
    Evento "versión de antes instalada (código $($p.ExitCode))"
    if (Test-Path $zero) {
        Start-Process -FilePath $zero -ArgumentList "--avisos"
        Start-Process -FilePath $zero -ArgumentList "--panel"
        Start-Sleep -Seconds 20
        Evento "ZERO abierto con el panel, y sus avisos en marcha"
    }
}

# Las fotos, en otro proceso de la misma sesión: el nombre de cada una es su hora (la misma que la de los eventos).
$captura = Start-Job -ArgumentList $dir, $alto -ScriptBlock {
    param($dir, $alto)
    Add-Type -AssemblyName System.Windows.Forms, System.Drawing
    $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    while (-not (Test-Path $alto)) {
        try {
            $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
            $g = [System.Drawing.Graphics]::FromImage($bmp)
            $g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
            $bmp.Save((Join-Path $dir ("f-" + [DateTime]::UtcNow.ToString("HHmmss.fff") + ".png")), [System.Drawing.Imaging.ImageFormat]::Png)
            $g.Dispose(); $bmp.Dispose()
        } catch { }
        Start-Sleep -Milliseconds 150
    }
}
Start-Sleep -Seconds 2

$A = [System.Windows.Automation.AutomationElement]
$raiz = $A::RootElement
$todas = [System.Windows.Automation.Condition]::TrueCondition
$esBoton = New-Object System.Windows.Automation.PropertyCondition($A::ControlTypeProperty, [System.Windows.Automation.ControlType]::Button)
$esCasilla = New-Object System.Windows.Automation.PropertyCondition($A::ControlTypeProperty, [System.Windows.Automation.ControlType]::CheckBox)
$esRadio = New-Object System.Windows.Automation.PropertyCondition($A::ControlTypeProperty, [System.Windows.Automation.ControlType]::RadioButton)

# Un clic de verdad con el ratón, para lo que la automatización no sabe pulsar.
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public static class Raton {
  [DllImport("user32.dll")] static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] static extern void mouse_event(uint f, uint x, uint y, uint d, UIntPtr e);
  [DllImport("user32.dll")] static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  public static void Clic(int x, int y) { SetCursorPos(x, y); System.Threading.Thread.Sleep(80); mouse_event(2, 0, 0, 0, UIntPtr.Zero); mouse_event(4, 0, 0, 0, UIntPtr.Zero); }
  // BM_CLICK: el botón (o la casilla) se pulsa como con el ratón y avisa a su ventana (el instalador cambia su propiedad)
  public static bool Pulsa(IntPtr h) { return PostMessage(h, 0x00F5, IntPtr.Zero, IntPtr.Zero); }
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h, uint f);
}
"@

$vistas = @{}
$arboles = @{}
# La consola del ordenador de pruebas, minimizada: en la vuelta del 2 Nov 2026 tapaba la ventana de GUARDIANA al abrirse.
foreach ($w in [System.Windows.Automation.AutomationElement]::RootElement.FindAll([System.Windows.Automation.TreeScope]::Children, [System.Windows.Automation.Condition]::TrueCondition)) {
    try { if ($w.Current.ClassName -match "CASCADIA_HOSTING_WINDOW_CLASS|ConsoleWindowClass") { [void][Raton]::ShowWindow([IntPtr]$w.Current.NativeWindowHandle, 6) } } catch { }
}
function Arbol($w) {
    # Una vez por ventana: sus controles (tipo, nombre, si está activo y qué sabe hacer), para ver en el registro qué hay
    # en cada pantalla aunque la foto no lo diga.
    $clave = try { "$($w.Current.NativeWindowHandle)|$($w.Current.Name)" } catch { return }
    if ($arboles.ContainsKey($clave)) { return }
    $arboles[$clave] = $true
    foreach ($e in $w.FindAll([System.Windows.Automation.TreeScope]::Descendants, $todas)) {
        try {
            $c = $e.Current
            $pat = ($e.GetSupportedPatterns() | ForEach-Object { $_.ProgrammaticName -replace "PatternIdentifiers.Pattern", "" }) -join ","
            Evento "  control: $($c.ControlType.ProgrammaticName -replace 'ControlType.', '') «$($c.Name)» activo=$($c.IsEnabled) [$pat]"
        } catch { }
    }
}

# El programa de cada ventana, por su proceso (una vez por proceso).
$programas = @{}
function Programa([int]$id) {
    if (-not $programas.ContainsKey($id)) { $programas[$id] = try { (Get-Process -Id $id -ErrorAction Stop).ProcessName } catch { "?" } }
    return $programas[$id]
}
# Una ventana de GUARDIANA: la de ZERO (su clase, desde la página de espera hasta el panel) o, con ZERO=0, el panel en el navegador.
function EsGuardiana($c) {
    if ($c.ClassName -eq "GuardianaZeroVentana") { return $true }
    return ($c.Name -match "GUARDIANA" -and (Programa $c.ProcessId) -match "^(msedge|chrome|firefox|brave|opera)$")
}
# Las ventanas de GUARDIANA vistas, por «proceso|ventana», con la hora a la que aparecieron.
$deGuardiana = @{}
function MiraVentanas {
    # Cada ventana nueva de primer nivel, una vez: título, clase y programa.
    $ws = $raiz.FindAll([System.Windows.Automation.TreeScope]::Children, $todas)
    foreach ($w in $ws) {
        try {
            $c = $w.Current
            $clave = "$($c.NativeWindowHandle)|$($c.Name)"
            if (-not $vistas.ContainsKey($clave)) {
                $vistas[$clave] = $true
                Evento "ventana: «$($c.Name)» · clase $($c.ClassName) · $(Programa $c.ProcessId) ($($c.ProcessId))"
            }
            $k = "$($c.ProcessId)|$($c.NativeWindowHandle)"
            if (-not $deGuardiana.ContainsKey($k) -and (EsGuardiana $c)) { $deGuardiana[$k] = @{ t = Get-Date; nombre = $c.Name; proceso = $c.ProcessId } }
        } catch { }
    }
    return $ws
}

# Los controles del instalador de Windows llegan a la automatización como «Pane» sin patrones (la vuelta del 2 Nov 2026 lo
# enseñó): se pulsan con BM_CLICK en su propia ventana, y si no tienen ventana, con Invoke.
# La ventana del instalador, delante antes de pulsar nada (vuelta del 3 Nov 2026: al actualizar, ZERO de la versión
# anterior se abrió a pantalla completa encima; con el diálogo detrás, BM_CLICK no marcó la casilla y el clic de ratón
# cayó en ZERO). Una persona tendría el instalador delante: así se prueba lo que ella vería.
function Delante($e) {
    try {
        $h = [IntPtr]$e.Current.NativeWindowHandle
        if ($h -eq [IntPtr]::Zero) { return }
        $raiz = [Raton]::GetAncestor($h, 2)
        if ($raiz -eq [IntPtr]::Zero) { $raiz = $h }
        [void][Raton]::ShowWindow($raiz, 9)
        [void][Raton]::SetForegroundWindow($raiz)
        Start-Sleep -Milliseconds 200
    } catch { }
}

function Pulsa($e, [string]$que) {
    $h = [IntPtr]::Zero
    try { $h = [IntPtr]$e.Current.NativeWindowHandle } catch { }
    Delante $e
    try {
        if ($h -ne [IntPtr]::Zero -and [Raton]::Pulsa($h)) { Evento "pulsado (BM_CLICK): $que"; return $true }
        $e.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
        Evento "pulsado: $que"
        return $true
    } catch { Evento "no se pudo pulsar «$que»: $($_.Exception.Message)"; return $false }
}

# Marcar la casilla de aceptar, con lo que haga falta: la automatización (Toggle o Invoke) y, si la casilla no cambia,
# un clic de ratón en su cuadrado, como una persona. Cada intento queda en el registro.
function Marca($cb, [string]$n, [int]$intento) {
    Delante $cb
    if ($intento -eq 0) { if (Pulsa $cb "casilla: $n") { return } }
    if ($intento -eq 1) {
        try {
            $t = $cb.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern)
            if ($t.Current.ToggleState -ne [System.Windows.Automation.ToggleState]::On) { $t.Toggle() }
            Evento "marcada (Toggle): $n"; return
        } catch { Evento "Toggle no: $($_.Exception.Message)" }
    }
    if ($intento -eq 1) {
        try { $cb.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke(); Evento "marcada (Invoke): $n"; return } catch { Evento "Invoke no: $($_.Exception.Message)" }
    }
    try {
        $r = $cb.Current.BoundingRectangle
        [Raton]::Clic([int]($r.X + 7), [int]($r.Y + $r.Height / 2))
        Evento "marcada (clic en $([int]($r.X + 7)),$([int]($r.Y + $r.Height / 2))): $n"
    } catch { Evento "clic no: $($_.Exception.Message)" }
}

$sigue = @("Instalar", "Install", "Aceptar", "OK", "Siguiente", "Next")
$fin = @("Abrir GUARDIANA", "Open GUARDIANA", "Abrir o GUARDIANA", "Finalizar", "Finish", "Concluir", "Cerrar", "Close", "Fechar")

# Lo que ya estaba abierto de GUARDIANA antes del instalador (al actualizar, ZERO de la versión anterior): no cuenta como
# «se abrió al terminar»; vale solo una ventana de un proceso nuevo.
$null = MiraVentanas
$previos = @{}
foreach ($v in $deGuardiana.Values) { $previos[$v.proceso] = $true }
if ($previos.Count) { Evento "ya abierto de GUARDIANA antes de instalar: procesos $($previos.Keys -join ', ')" }

Evento "abriendo el instalador como un doble clic: $Instalador"
$inicio = Get-Date
$proceso = Start-Process -FilePath $Instalador -PassThru
$null = $proceso.Handle  # sin esto, ExitCode llega vacío cuando el proceso ya terminó
Evento "el instalador es el proceso $($proceso.Id)"
$limite = $inicio.AddSeconds($Segundos)
$cerrado = $null   # cuándo terminó solo el proceso del instalador
$abierto = $null   # la ventana de GUARDIANA que se abrió (de un proceso nuevo)
$ultima = $null    # el botón de una última pantalla, si el instalador se quedó en una
$hechos = @{}
while ((Get-Date) -lt $limite -and -not ($cerrado -and $abierto)) {
    $ws = MiraVentanas
    if (-not $abierto) {
        foreach ($v in $deGuardiana.Values) {
            if (-not $previos.ContainsKey($v.proceso)) { $abierto = $v; Evento "se abrió GUARDIANA: «$($v.nombre)» (proceso $($v.proceso))"; break }
        }
    }
    if (-not $cerrado -and $proceso -and $proceso.HasExited) { $cerrado = Get-Date; Evento "el instalador se cerró solo (código $($proceso.ExitCode))" }
    foreach ($w in $ws) {
        $titulo = try { $w.Current.Name } catch { "" }
        # Solo las ventanas del instalador (Windows Installer y sus diálogos): nunca dentro de GUARDIANA ni de otro programa.
        $deMsiexec = try { (Programa $w.Current.ProcessId) -eq "msiexec" } catch { $false }
        if (-not $deMsiexec) { continue }
        if ($null -eq $proceso) {
            # Si Windows no devolvió el proceso al abrir el .msi, el del instalador es el de su ventana.
            $proceso = try { Get-Process -Id $w.Current.ProcessId -ErrorAction Stop } catch { $null }
            if ($proceso) { $null = $proceso.Handle; Evento "el instalador es el proceso $($proceso.Id) (por su ventana)" }
        }
        Arbol $w
        try {
            # La casilla de aceptar: se marca, y si a los 4 s «Instalar» sigue apagado, se prueba de otra manera.
            # por su nombre, sea del tipo que sea (en el instalador de Windows todos llegan como «Pane»)
            $controles = @($w.FindAll([System.Windows.Automation.TreeScope]::Descendants, $todas))
            $instalarApagado = @($controles | Where-Object { ($_.Current.Name -replace "&", "").Trim() -match "^(Instalar|Install)$" -and -not $_.Current.IsEnabled }).Count -gt 0
            foreach ($cb in $controles) {
                $n = $cb.Current.Name
                if ($n -notmatch "acepto|accept|aceito" -or -not $cb.Current.IsEnabled) { continue }
                $k = "casilla|$($w.Current.NativeWindowHandle)"
                $previo = $hechos[$k]
                if ($null -eq $previo) { $hechos[$k] = @{ n = 0; t = Get-Date }; Marca $cb $n 0; Start-Sleep -Milliseconds 800 }
                elseif ($instalarApagado -and ((Get-Date) - $previo.t).TotalSeconds -gt 4 -and $previo.n -lt 3) {
                    $previo.n++; $previo.t = Get-Date
                    Evento "«Instalar» sigue apagado: otro intento ($($previo.n))"
                    Marca $cb $n $previo.n; Start-Sleep -Milliseconds 800
                }
            }
            # En la pregunta de Windows por programas abiertos: la primera opción (cerrarlos), como haría casi todo el mundo.
            $radios = $w.FindAll([System.Windows.Automation.TreeScope]::Descendants, $esRadio)
            if ($radios.Count -gt 0 -and -not $hechos.ContainsKey("radio")) {
                $hechos["radio"] = $true
                Evento "Windows pregunta: $(($radios | ForEach-Object { $_.Current.Name }) -join ' / ')"
                try { $radios[0].GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern).Select() } catch { }
                Start-Sleep -Milliseconds 800
            }
            $botones = @($controles | Where-Object { $_.Current.IsEnabled -and $_.Current.IsOffscreen -eq $false })
            $porNombre = @{}
            foreach ($b in $botones) { $porNombre[($b.Current.Name -replace "&", "").Trim()] = $b }
            # Una última pantalla (sin «Instalar» y con «Finalizar», «Cerrar»…): desde el 3 nov 2026 no tiene que salir, ni la de
            # error ni la de cancelar. Es un fallo: se apunta, se deja 3 s para la foto y se pulsa para que el instalador termine.
            if (-not $porNombre.ContainsKey("Instalar") -and -not $porNombre.ContainsKey("Install")) {
                $cual = $fin | Where-Object { $porNombre.ContainsKey($_) } | Select-Object -First 1
                if ($cual -and -not $hechos.ContainsKey("fin|$cual")) {
                    $hechos["fin|$cual"] = $true
                    $ultima = $cual
                    Evento "FALLO: el instalador se quedó en una última pantalla («$titulo»), con «$cual»"
                    Start-Sleep -Seconds 3
                    [void](Pulsa $porNombre[$cual] $cual)
                    continue
                }
            }
            $cual = $sigue | Where-Object { $porNombre.ContainsKey($_) -and -not $hechos.ContainsKey("$($porNombre[$_].Current.NativeWindowHandle)|$_") } | Select-Object -First 1
            if ($cual) {
                Start-Sleep -Seconds 2
                if (Pulsa $porNombre[$cual] $cual) { $hechos["$($porNombre[$cual].Current.NativeWindowHandle)|$cual"] = $true }
            }
        } catch {
            # Cada error distinto, una vez en el registro (antes se callaban todos y la prueba se quedaba quieta sin decir por qué).
            $m = "$($_.Exception.Message)"
            if (-not $hechos.ContainsKey("error|$m")) { $hechos["error|$m"] = $true; Evento "error en la ventana «$titulo»: $m" }
        }
    }
    Start-Sleep -Milliseconds 150
}
if (-not $cerrado -and $proceso -and $proceso.HasExited) { $cerrado = Get-Date; Evento "el instalador se cerró solo (código $($proceso.ExitCode))" }
$bien = $cerrado -and $abierto -and -not $ultima
if ($bien) {
    $resultado = "bien: el instalador se cerró solo a los $([int]($cerrado - $inicio).TotalSeconds) s y se abrió GUARDIANA («$($abierto.nombre)») a los $([int]($abierto.t - $inicio).TotalSeconds) s, sin pulsar nada al final"
} else {
    $que = @()
    if (-not $cerrado) { $que += "el instalador no se cerró solo en $Segundos s" }
    if ($ultima) { $que += "se quedó en una última pantalla con «$ultima»" }
    if (-not $abierto) { $que += "no se abrió ninguna ventana de GUARDIANA" }
    $resultado = "FALLO: $($que -join '; ')"
}
Evento "RESULTADO: $resultado"
# Lo que pasa después de cerrar el instalador (lo que se abre solo).
$despues = (Get-Date).AddSeconds(45)
while ((Get-Date) -lt $despues) { $null = MiraVentanas; Start-Sleep -Milliseconds 150 }
New-Item -ItemType File -Path $alto | Out-Null
Wait-Job $captura -Timeout 30 | Out-Null
Evento "fin"

# Las fotos distintas entre sí (miniatura de 96x72 comparada con la última que se quedó).
Add-Type -TypeDefinition @"
using System; using System.Drawing;
public static class Mini {
  public static byte[] Huella(string f) {
    using (var b = new Bitmap(f)) using (var m = new Bitmap(b, new Size(96, 72))) {
      var r = new byte[96 * 72];
      for (int y = 0; y < 72; y++) for (int x = 0; x < 96; x++) { var c = m.GetPixel(x, y); r[y * 96 + x] = (byte)((c.R + c.G + c.B) / 3); }
      return r;
    }
  }
  public static double Distancia(byte[] a, byte[] b) { long s = 0; for (int i = 0; i < a.Length; i++) s += Math.Abs(a[i] - b[i]); return (double)s / a.Length; }
}
"@ -ReferencedAssemblies System.Drawing
$fotos = Get-ChildItem $dir -Filter "f-*.png" | Sort-Object Name
$quedan = New-Object System.Collections.ArrayList
$ultima = $null
foreach ($f in $fotos) {
    $h = [Mini]::Huella($f.FullName)
    if ($null -eq $ultima -or [Mini]::Distancia($h, $ultima) -gt 0.6) { [void]$quedan.Add($f); $ultima = $h }
}
Write-Output "fotos: $($fotos.Count) sacadas, $($quedan.Count) distintas"
if ($quedan.Count -gt $MaxFotos) {
    # Si son demasiadas, repartidas: la primera, la última y las de en medio a intervalos iguales.
    $paso = ($quedan.Count - 1) / ($MaxFotos - 1)
    $quedan = @(0..($MaxFotos - 1) | ForEach-Object { $quedan[[int][Math]::Round($_ * $paso)] })
}
$jpeg = [System.Drawing.Imaging.ImageCodecInfo]::GetImageEncoders() | Where-Object { $_.MimeType -eq "image/jpeg" }
$par = New-Object System.Drawing.Imaging.EncoderParameters 1
$par.Param[0] = New-Object System.Drawing.Imaging.EncoderParameter([System.Drawing.Imaging.Encoder]::Quality, [long]82)
Write-Output "::group::fotos ($Nombre)"
foreach ($f in $quedan) {
    $ms = New-Object System.IO.MemoryStream
    $img = [System.Drawing.Image]::FromFile($f.FullName)
    $img.Save($ms, $jpeg, $par); $img.Dispose()
    $b64 = [Convert]::ToBase64String($ms.ToArray())
    $id = $f.BaseName
    for ($i = 0; $i -lt $b64.Length; $i += 3000) {
        Write-Output ("FOTO|{0}|{1}|{2}" -f $id, [int]($i / 3000), $b64.Substring($i, [Math]::Min(3000, $b64.Length - $i)))
    }
}
Write-Output "::endgroup::"
Write-Output "::group::ventanas y pasos ($Nombre)"
Get-Content $eventos -Encoding UTF8
Write-Output "::endgroup::"
if ($bien) { Write-Output "::notice title=instalador ($Nombre)::$resultado" }
else { Write-Output "::error title=instalador ($Nombre)::$resultado"; exit 1 }
