# installer.nsi - NSIS script to install an application as a Windows Service
#--------------------------------
# 1. Defines and Variables
# These are the main settings you might want to change.
#--------------------------------
!define APP_NAME "Vuio DLNA Server"
!define PUBLISHER "vuio" # Change this to your name/company
!define SERVICE_NAME "vuio" # The internal name for the Windows Service
!define SERVICE_DISPLAY_NAME "Vuio DLNA" # The name shown in services.msc
# These variables are passed in from the GitHub Actions command line
# !define APP_EXE "${APP_EXE}" <- REMOVED: This was causing the duplicate definition error
# !define VERSION "${VERSION}" <- REMOVED: This should also be passed from command line
# !define OUTFILE "${OUTFILE}" <- REMOVED: This should also be passed from command line
#--------------------------------
# 2. Installer Attributes
#--------------------------------
Name "${APP_NAME} ${VERSION}"
OutFile "${OUTFILE}"
InstallDir "$PROGRAMFILES64\${APP_NAME}"
InstallDirRegKey HKLM "Software\${APP_NAME}" "InstallDir"
RequestExecutionLevel admin # CRITICAL: Required to install a service.
#--------------------------------
# 3. Modern UI Configuration
#--------------------------------
!include "MUI2.nsh"
!define MUI_ABORTWARNING
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
#--------------------------------
# 4. Installation Section
# This is where the main logic happens.
#--------------------------------
Section "Install" SEC_INSTALL
  SetOutPath $INSTDIR
  # Write the installation path and uninstaller information to the registry
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "DisplayName" "${APP_NAME}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "Publisher" "${PUBLISHER}"
  WriteRegStr HKLM "Software\${APP_NAME}" "InstallDir" "$INSTDIR"
  # Copy the application executable
  File "${APP_EXE}"
  # Create the uninstaller
  WriteUninstaller "$INSTDIR\uninstall.exe"
  # Create and start the Windows Service
  # We use nsExec to run the command without showing a command prompt.
  # The binPath needs to be quoted correctly.
  nsExec::ExecToLog 'sc create "${SERVICE_NAME}" binPath= "\"$INSTDIR\${APP_EXE}\"" start= auto DisplayName= "${SERVICE_DISPLAY_NAME}"'
  
  # Optionally, start the service right after installation
  nsExec::ExecToLog 'sc start "${SERVICE_NAME}"'
SectionEnd
#--------------------------------
# 5. Uninstallation Section
# This runs when the user uninstalls the application.
#--------------------------------
Section "Uninstall"
  # ALWAYS stop and delete the service before removing files.
  nsExec::ExecToLog 'sc stop "${SERVICE_NAME}"'
  Sleep 2000 ; Give the service a moment to stop
  nsExec::ExecToLog 'sc delete "${SERVICE_NAME}"'
  Sleep 2000 ; Give Windows a moment to delete it
  # Remove the files and directories
  Delete "$INSTDIR\${APP_EXE}"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  # Remove the registry keys
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}"
  DeleteRegKey HKLM "Software\${APP_NAME}"
SectionEnd