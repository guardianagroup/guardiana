# build/capturar-aviso.ps1: graba la pantalla del Windows real mientras se instala GUARDIANA, para
# poder enseñar en la web el aviso azul tal como sale, sin recrearlo (regla del brief: no se recrea).
# Se ejecuta EN el equipo, no por SSH: una sesión de SSH no tiene escritorio y CopyFromScreen falla.
# No envía nada a ninguna parte: escribe fotogramas en una carpeta y los comprime.
param(
    [int]$Segundos = 60,
    [int]$PorSegundo = 2,
    [string]$Salida = "$env:USERPROFILE\Desktop\guardiana-aviso"
)
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Windows.Forms, System.Drawing

if (Test-Path $Salida) { Remove-Item $Salida -Recurse -Force }
New-Item -ItemType Directory -Path $Salida | Out-Null

Write-Host ""
Write-Host "  GUARDIANA - captura del aviso de Windows" -ForegroundColor Cyan
Write-Host ""
Write-Host "  Se van a grabar $Segundos segundos de pantalla, $PorSegundo fotogramas por segundo."
Write-Host "  Cierra antes lo que no quieras que se vea: sale la pantalla entera."
Write-Host ""
Write-Host "  Cuando empiece, haz la instalacion normal:"
Write-Host "    1. doble clic en el .msi que descargaste"
Write-Host "    2. pulsa 'Mas informacion'"
Write-Host "    3. pulsa 'Ejecutar de todas formas'"
Write-Host "    4. siguiente, siguiente, hasta el final"
Write-Host ""
Read-Host "  Pulsa Enter para empezar"
for ($i = 3; $i -ge 1; $i--) { Write-Host "  $i..."; Start-Sleep -Seconds 1 }
Write-Host "  GRABANDO. Haz la instalacion." -ForegroundColor Yellow

$b = [System.Windows.Forms.SystemInformation]::VirtualScreen
$intervalo = [int](1000 / $PorSegundo)
$total = $Segundos * $PorSegundo
for ($n = 1; $n -le $total; $n++) {
    $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($b.Left, $b.Top, 0, 0, $bmp.Size)
    $bmp.Save((Join-Path $Salida ("f{0:d4}.png" -f $n)), [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
    Start-Sleep -Milliseconds $intervalo
}

$zip = "$Salida.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path "$Salida\*.png" -DestinationPath $zip
$mb = [math]::Round((Get-Item $zip).Length / 1MB, 1)
Write-Host ""
Write-Host "  Listo: $zip ($mb MB, $total fotogramas)" -ForegroundColor Green
Write-Host "  Ya esta en el Escritorio. No se ha enviado a ninguna parte."
Write-Host ""
Read-Host "  Pulsa Enter para cerrar"
