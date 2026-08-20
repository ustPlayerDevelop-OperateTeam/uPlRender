@echo off
setlocal
rem ------------------------------------------------------------
rem  Build C++ debug_renderer.exe with MSVC cl.exe
rem  Usage: Open "x64 Native Tools Command Prompt for VS" first
rem ------------------------------------------------------------
rem  Ensure we run from the directory containing this script

pushd "%~dp0"

if not exist debug_renderer.cpp (
    echo [ERROR] debug_renderer.cpp not found in %~dp0
    popd
    exit /b 1
)

where cl.exe >nul 2>nul
if %errorlevel% neq 0 (
    echo [ERROR] cl.exe not in PATH. Start "x64 Native Tools Command Prompt for VS" and retry.
    popd
    exit /b 1
)

rem ensure the Rust output dir exists (exe sits next to the DLL)
if not exist "..\target\release" mkdir "..\target\release"

echo [INFO] Compiling debug_renderer.cpp -^> ..\target\release\debug_renderer.exe ...
cl.exe /nologo /W4 /O2 /MT debug_renderer.cpp /Fe:"..\target\release\debug_renderer.exe" /link /SUBSYSTEM:CONSOLE /MACHINE:X64
if %errorlevel% neq 0 (
    echo [ERROR] compile failed
    popd
    exit /b 1
)

echo [INFO] build ok
dir "..\target\release\debug_renderer.exe"
echo.
echo [INFO] run example (from tools dir):
echo     ..\target\release\debug_renderer.exe ..\examples\frame_config.json 60 ..\debug_output
popd
endlocal