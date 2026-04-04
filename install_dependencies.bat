@echo off
setlocal

cd /d "%~dp0"
set "PROJECT_ROOT=%~dp0"

set "PYTHON_CMD="
if exist "%PROJECT_ROOT%.venv\Scripts\python.exe" set "PYTHON_CMD=%PROJECT_ROOT%.venv\Scripts\python.exe"

if defined PYTHON_CMD goto :have_python

where py >nul 2>&1
if not errorlevel 1 (
  echo [INFO] Creating virtual environment .venv...
  py -m venv "%PROJECT_ROOT%.venv"
  if errorlevel 1 goto :fail
  set "PYTHON_CMD=%PROJECT_ROOT%.venv\Scripts\python.exe"
)

if defined PYTHON_CMD goto :have_python

where python >nul 2>&1
if not errorlevel 1 (
  echo [INFO] Creating virtual environment .venv...
  python -m venv "%PROJECT_ROOT%.venv"
  if errorlevel 1 goto :fail
  set "PYTHON_CMD=%PROJECT_ROOT%.venv\Scripts\python.exe"
)

:have_python

if "%PYTHON_CMD%"=="" (
  echo [ERROR] Python not found. Install Python and try again.
  goto :fail
)

echo [INFO] Checking Python dependencies...
"%PYTHON_CMD%" -c "import importlib.util,sys;mods=['markdown','pygments','bs4','weasyprint','bleach','pypdf'];missing=[m for m in mods if importlib.util.find_spec(m) is None];sys.exit(0 if not missing else 1)"
if errorlevel 1 (
  echo [INFO] Installing/upgrading project runtime dependencies...
  "%PYTHON_CMD%" -m pip install --upgrade pip
  if errorlevel 1 goto :fail
  "%PYTHON_CMD%" -m pip install -e .
  if errorlevel 1 goto :fail
) else (
  echo [INFO] Dependencies already installed.
)

where qpdf >nul 2>&1
if errorlevel 1 (
  echo [INFO] qpdf not found. Attempting automatic installation...

  where winget >nul 2>&1
  if not errorlevel 1 (
    winget install --id qpdf.qpdf --accept-package-agreements --accept-source-agreements --silent
  )

  where qpdf >nul 2>&1
  if errorlevel 1 (
    where choco >nul 2>&1
    if not errorlevel 1 (
      choco install qpdf -y
    )
  )

  where qpdf >nul 2>&1
  if errorlevel 1 (
    echo [WARNING] Could not install qpdf automatically.
    echo [WARNING] Install qpdf manually then re-run this script.
    echo [WARNING] winget: winget install --id qpdf.qpdf
    echo [WARNING] choco : choco install qpdf -y
  ) else (
    echo [INFO] qpdf installed successfully.
  )
) else (
  echo [INFO] qpdf already available.
)

call :configure_weasyprint_runtime

"%PYTHON_CMD%" -c "from nectar_render.utils.weasyprint_runtime import prepare_weasyprint_environment; prepare_weasyprint_environment(); import weasyprint"
if errorlevel 1 (
  echo [INFO] WeasyPrint is installed but system DLLs appear to be missing.
  echo [INFO] Attempting automatic installation of MSYS2 + Pango...
  call :install_weasyprint_runtime
  call :configure_weasyprint_runtime
  "%PYTHON_CMD%" -c "from nectar_render.utils.weasyprint_runtime import prepare_weasyprint_environment; prepare_weasyprint_environment(); import weasyprint"
  if errorlevel 1 (
    echo [WARNING] WeasyPrint is installed but system DLLs are still missing.
    echo [WARNING] For PDF on Windows: check MSYS2 + Pango then try again.
    echo [WARNING] The app can still launch ^(HTML export works^).
  ) else (
    echo [INFO] WeasyPrint runtime configured successfully.
  )
)

endlocal & exit /b 0

:fail
endlocal & exit /b 1

:configure_weasyprint_runtime
set "WEASYPRINT_RUNTIME_DIRS=%WEASYPRINT_DLL_DIRECTORIES%"
if exist "C:\msys64\ucrt64\bin" (
  if defined WEASYPRINT_RUNTIME_DIRS (
    set "WEASYPRINT_RUNTIME_DIRS=%WEASYPRINT_RUNTIME_DIRS%;C:\msys64\ucrt64\bin"
  ) else (
    set "WEASYPRINT_RUNTIME_DIRS=C:\msys64\ucrt64\bin"
  )
)
if exist "C:\msys64\mingw64\bin" (
  if defined WEASYPRINT_RUNTIME_DIRS (
    set "WEASYPRINT_RUNTIME_DIRS=%WEASYPRINT_RUNTIME_DIRS%;C:\msys64\mingw64\bin"
  ) else (
    set "WEASYPRINT_RUNTIME_DIRS=C:\msys64\mingw64\bin"
  )
)
set "WEASYPRINT_DLL_DIRECTORIES=%WEASYPRINT_RUNTIME_DIRS%"
set "WEASYPRINT_RUNTIME_DIRS="
exit /b 0

:install_weasyprint_runtime
where winget >nul 2>&1
if errorlevel 1 (
  echo [WARNING] winget not found. Cannot install MSYS2 automatically.
  echo [WARNING] Install MSYS2 then run:
  echo [WARNING]   C:\msys64\usr\bin\bash.exe -lc "pacman -S --needed mingw-w64-ucrt-x86_64-pango"
  exit /b 0
)

if not exist "C:\msys64\usr\bin\bash.exe" (
  winget install --id MSYS2.MSYS2 --accept-package-agreements --accept-source-agreements --silent
)

if not exist "C:\msys64\usr\bin\bash.exe" (
  echo [WARNING] MSYS2 could not be installed automatically.
  echo [WARNING] Install it manually, then run:
  echo [WARNING]   C:\msys64\usr\bin\bash.exe -lc "pacman -S --needed mingw-w64-ucrt-x86_64-pango"
  exit /b 0
)

echo [INFO] Installing Pango via MSYS2...
"C:\msys64\usr\bin\bash.exe" -lc "pacman -S --noconfirm --needed mingw-w64-ucrt-x86_64-pango"
if errorlevel 1 (
  echo [WARNING] Automatic Pango installation failed.
  echo [WARNING] Run manually:
  echo [WARNING]   C:\msys64\usr\bin\bash.exe -lc "pacman -S --needed mingw-w64-ucrt-x86_64-pango"
)
exit /b 0
