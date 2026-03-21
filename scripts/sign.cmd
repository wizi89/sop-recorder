@echo off
echo === Sign script started === 1>&2
echo Signing: %1 1>&2
trusted-signing-cli -e https://weu.codesigning.azure.net -a signing-sop-sorcery -c sop-recorder-profile -d SOP-Sorcery %1
set EXIT_CODE=%ERRORLEVEL%
echo === Sign script finished with code %EXIT_CODE% === 1>&2
exit /b %EXIT_CODE%
