@echo off
rem Abre Orfeon con un formato de OpenGL mas simple: usalo si Orfeon.exe no abre la ventana
rem (placas de video viejas, escritorio remoto, maquinas virtuales).
set ORFEON_GL_SIMPLE=1
start "" "%~dp0Orfeon.exe"
