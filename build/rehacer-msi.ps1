# Los tres instaladores de Windows, uno por idioma, desde un mismo .exe.
#
# El .exe tiene que ser el que sale de la compilacion reproducible, no uno recien compilado: el
# sello de la fecha del commit cambia la huella, y la web promete que la huella del MSI esta en el
# registro publico. Por eso este guion no compila nada, solo empaqueta lo que se le da.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File build\rehacer-msi.ps1 -Version 1.0.0
#
# Deja los tres MSI en msi\ y copia una copia al Escritorio, y escribe el principio de cada huella
# para poder compararla de un vistazo con la del Mac.
param(
  [string]$Version = "1.0.0",
  [string]$Base    = "C:\Users\franc\guardiana",
  [string]$Exe     = "",
  [string]$Wix     = ""
)
$ErrorActionPreference = "Stop"
if ($Exe -eq "") { $Exe = "$Base\guardiana.exe" }
if ($Wix -eq "") { $Wix = "$Base\wix5\wix.dll" }
if (-not (Test-Path $Exe)) { throw "No esta $Exe. Copia ahi el .exe de la compilacion reproducible." }
New-Item -ItemType Directory -Force -Path "$Base\msi" | Out-Null

foreach ($idioma in @("es","en","pt")) {
  # El espanol no lleva sufijo: es el nombre que la web enseña por defecto.
  $sufijo = if ($idioma -eq "es") { "" } else { "-$idioma" }
  $salida = "$Base\msi\guardiana-$Version-windows-x64$sufijo.msi"
  & powershell -NoProfile -ExecutionPolicy Bypass -File "$Base\build\msi.ps1" `
      -Exe $Exe -Version $Version -Idioma $idioma -Out $salida -Wix $Wix | Out-Null
  if (-not (Test-Path $salida)) { throw "El MSI de $idioma no se creo." }
  Write-Host "$idioma listo"
}

# La copia al Escritorio es una comodidad, no el resultado: si un MSI esta abierto o el antivirus
# lo tiene cogido, se avisa y se sigue. Lo que vale son los de msi\ (22 sep 2026).
try   { Copy-Item "$Base\msi\guardiana-$Version-windows-x64*.msi" "$env:USERPROFILE\Desktop\" -Force }
catch { Write-Host "Aviso: no se pudo copiar al Escritorio ($($_.Exception.Message)). Los MSI estan en $Base\msi." }
Get-ChildItem "$Base\msi" -Filter "guardiana-$Version-windows-x64*.msi" | ForEach-Object {
  $_.Name + "  " + (Get-FileHash $_.FullName -Algorithm SHA256).Hash.Substring(0,12) }
