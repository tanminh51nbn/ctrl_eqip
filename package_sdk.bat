@echo off
setlocal enabledelayedexpansion
echo ==============================================
echo    CTRL_EQIP - SDK PACKAGER (C/C++ BINDING)
echo ==============================================
echo.

echo [1/4] Bien dich thu vien AI loi (Phien ban RELEASE toi ieu hieu nang)...
cargo build --release
if %errorlevel% neq 0 (
    echo [Loi] Bien dich Rust that bai! Hay kiem tra lai code.
    pause
    exit /b
)
echo [OK] Bien dich thanh cong.

echo.
echo [2/4] Dang tao thu muc SDK...
set SDK_DIR=ctrl_eqip_sdk
if exist %SDK_DIR% rmdir /s /q %SDK_DIR%
mkdir %SDK_DIR%
mkdir %SDK_DIR%\models
mkdir %SDK_DIR%\lib
mkdir %SDK_DIR%\include
echo [OK] Thu muc khoi tao thanh cong.

echo.
echo [3/4] Dang dong goi file (DLL, Kien truc, Model)...
:: File Thuc Thi C/C++ Header
copy "include\ctrl_eqip.h" "%SDK_DIR%\include\" >nul
:: File Windows Dynamic Link Library (DLL)
copy "target\release\ctrl_eqip.dll" "%SDK_DIR%\lib\" >nul
:: Option: Copy them file .dll.lib de ho tro lien ket MSVC (Neu co)
if exist "target\release\ctrl_eqip.dll.lib" copy "target\release\ctrl_eqip.dll.lib" "%SDK_DIR%\lib\" >nul
:: File ONNX Model tu thu muc hien tai
xcopy "models\*" "%SDK_DIR%\models\" /E /Y /Q >nul

echo [OK] Da dong goi thanh cong.

echo.
echo [4/4] Tao file Huong_dan_nhanh.txt cho dong doi cua ban...
(
    echo HUONG DAN SU DUNG THU VIEN AI CTRL_EQIP
    echo =========================================
    echo 1. Copy toan bo cac file trong thu muc nay vao project C/C++ cua ban.
    echo.
    echo 2. Include file header vao code:
    echo    #include "include/ctrl_eqip.h"
    echo.
    echo 3. Lien ket DLL khi bien dich (Su dung GCC hoac MSVC):
    echo    gcc main.c -I./include -L./lib -lctrl_eqip -o main.exe
    echo.
    echo 4. Chay exe:
    echo    a. De chay duoc main.exe, file ctrl_eqip.dll PHAI nam cung thu muc voi main.exe.
    echo    b. Thu muc 'models' cua AI cung phai nam cung thu muc chay phan mem.
)>"%SDK_DIR%\Huong_dan_nhanh.txt"
echo [OK] File huong dan tao thanh cong.

echo.
echo ==============================================
echo [XONG] SDK da duoc tao trong thu muc: ctrl_eqip_sdk\
echo Ban chi can ZIP thu muc 'ctrl_eqip_sdk' gui cho team C/C++ la ho chay duoc OK!
echo ==============================================
pause
