@echo off
set DB_PATH=%APPDATA%\com.ghub.app\ghub.db
if exist "%DB_PATH%" (
  mkdir backups 2>nul
  copy "%DB_PATH%" "backups\ghub_initial.db"
  echo Backup created: backups\ghub_initial.db
) else (
  echo DB not found at %DB_PATH%
)
