@echo off
echo ========================================================
echo   DapCode Direct GPU Engine - Desktop Live Simulator
echo ========================================================
echo.
taskkill /F /IM pratinjau_mobile.exe >nul 2>&1
echo Mengompilasi kode C11 DapCode dan Engine GPU dengan MinGW GCC...
gcc preview_main.c ../engine/app_dapcode.c ../engine/dapcode_engine.c -I../engine -DDAPCODE_CUSTOM_CANVAS -DDAPCODE_NO_MAIN -lopengl32 -lgdi32 -lm -O3 -o pratinjau_mobile.exe
if %ERRORLEVEL% EQU 0 (
    echo Kompilasi berhasil!
    echo Membuka simulator layar perangkat seluler...
    start pratinjau_mobile.exe
) else (
    echo Kompilasi gagal. Pastikan MinGW GCC terpasang di sistem PATH.
    pause
)
