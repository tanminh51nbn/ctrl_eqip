@echo off
echo --- Building C/C++ Examples for ctrl_eqip ---

:: Kiểm tra xem thư mục include và target có tồn tại không
if not exist "include\ctrl_eqip.h" (
    echo [Error] Khong tim thay include/ctrl_eqip.h. Hay dam bao ban dang o thu mục goc cua project.
    pause
    exit /b
)

if not exist "target\debug\ctrl_eqip.dll" (
    echo [Error] Khong tim thay target/debug/ctrl_eqip.dll. Hay chay 'cargo build' truoc.
    pause
    exit /b
)

:: Copy DLL ra thu muc hien tai de EXE co the chay duoc
copy "target\debug\ctrl_eqip.dll" "examples\" > nul

echo [1/2] Bien dich C Example (main_loop.c)...
gcc examples\main_loop.c -I./include -L./target/debug -lctrl_eqip -o examples\main_loop_c.exe
if %errorlevel% neq 0 (
    echo [Fail] Khong the bien dich C Example. Hay kiem tra xem ban da cai GCC chua.
) else (
    echo [Success] Da tao: examples\main_loop_c.exe
)

echo.
echo [2/2] Bien dich C++ Example (main_loop.cpp)...
g++ examples\main_loop.cpp -I./include -L./target/debug -lctrl_eqip -o examples\main_loop_cpp.exe
if %errorlevel% neq 0 (
    echo [Fail] Khong the bien dich C++ Example.
) else (
    echo [Success] Da tao: examples\main_loop_cpp.exe
)

echo.
echo --- Hoan thanh ---
echo De chay thu, hay go: .\examples\main_loop_c.exe (hoac cpp)
pause
