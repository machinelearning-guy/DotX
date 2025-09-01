@echo off
rem DotX Windows Installer Build Script
rem Requires WiX Toolset v3 to be installed

echo Building DotX Windows Installer...

rem Check if WiX is installed
where heat.exe >nul 2>&1
if %ERRORLEVEL% neq 0 (
    echo ERROR: WiX Toolset not found in PATH
    echo Please install WiX Toolset v3 from https://wixtoolset.org/releases/
    exit /b 1
)

rem Set variables
set "SOURCE_DIR=..\dist"
set "OUTPUT_DIR=output"
set "WXS_FILE=DotX.wxs"
set "WIXOBJ_FILE=DotX.wixobj"
set "MSI_FILE=DotX-Setup.msi"

rem Create output directory
if not exist "%OUTPUT_DIR%" mkdir "%OUTPUT_DIR%"

rem Compile WiX source
echo Compiling WiX source...
candle.exe -dSourceDir="%SOURCE_DIR%" -out "%OUTPUT_DIR%\%WIXOBJ_FILE%" "%WXS_FILE%"
if %ERRORLEVEL% neq 0 (
    echo ERROR: Failed to compile WiX source
    exit /b 1
)

rem Link to create MSI
echo Creating MSI installer...
light.exe -ext WixUIExtension -out "%OUTPUT_DIR%\%MSI_FILE%" "%OUTPUT_DIR%\%WIXOBJ_FILE%"
if %ERRORLEVEL% neq 0 (
    echo ERROR: Failed to create MSI installer
    exit /b 1
)

echo Success! Installer created: %OUTPUT_DIR%\%MSI_FILE%
pause