@echo off
setlocal

cd /d "%~dp0"

set "PROJECT_ROOT=%~dp0"
set "PYTHONPATH=%PROJECT_ROOT%src;%PYTHONPATH%"

call "%PROJECT_ROOT%install_dependencies.bat"
if errorlevel 1 (
  echo.
  echo Dependency installation/check failed.
  echo.
  pause
  goto :end
)

if exist "%PROJECT_ROOT%.venv\Scripts\python.exe" (
  "%PROJECT_ROOT%.venv\Scripts\python.exe" -m md_to_pdf.main
  goto :end
)

where py >nul 2>&1
if %errorlevel%==0 (
  py -m md_to_pdf.main
  goto :end
)

where python >nul 2>&1
if %errorlevel%==0 (
  python -m md_to_pdf.main
  goto :end
)

echo.
echo Could not find Python to launch the application.
echo Install Python or create a .venv in this folder.
echo.
pause

:end
endlocal
