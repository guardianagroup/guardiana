@echo off
rem Doble clic aqui. Llama a capturar-aviso.ps1 sin pelearse con la politica de ejecucion.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0capturar-aviso.ps1"
