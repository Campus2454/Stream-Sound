; Stream Sound setup for Windows, in Thai.
;
; Built by .github/workflows/build.yml:
;   makensis /DVERSION=1.2.3 /DEXE=<StreamSound.exe> /DOUTFILE=<setup.exe> windows.nsi
;
; Started by hand it shows the usual steps: folder, shortcuts, and "open
; Stream Sound" at the end. It adds an uninstaller to Settings > Apps.
;
; The app's updater starts it with
;   /UPDATE /D=<install folder>
; which skips the questions, shows only the progress and opens the app again
; when done. /FROM="<file>" names a copy run from a downloaded file; that
; file is removed once the app is installed.

Unicode true
ManifestDPIAware true
SetCompressor /SOLID lzma
RequestExecutionLevel admin

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!ifndef EXE
  !define EXE "..\..\target\release\stream-sound.exe"
!endif
!ifndef OUTFILE
  !define OUTFILE "StreamSound-setup.exe"
!endif

!define APP "Stream Sound"
!define EXE_NAME "StreamSound.exe"
; setup::WINDOWS_UNINSTALLER: the app looks for it to tell it is installed.
!define UNINSTALLER "uninstall.exe"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\StreamSound"
!define APP_KEY "Software\StreamSound"
; The app's own start-with-Windows entry (os/windows.rs).
!define RUN_KEY "Software\Microsoft\Windows\CurrentVersion\Run"
!define APPROVED_KEY "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run"
!define RUN_NAME "Stream Sound"
!define REPO_URL "https://github.com/Campus2454/Stream-Sound"

; "1.0" or "1.0.5" as the four numbers Windows wants.
!searchparse /noerrors "${VERSION}.0.0" "" VER_1 "." VER_2 "." VER_3 "."

!include MUI2.nsh
!include FileFunc.nsh
!include LogicLib.nsh
!include x64.nsh

Name "${APP}"
OutFile "${OUTFILE}"
InstallDir "$PROGRAMFILES64\${APP}"
InstallDirRegKey HKLM "${UNINST_KEY}" "InstallLocation"
BrandingText "${APP} ${VERSION}"
ShowInstDetails hide
ShowUninstDetails hide
SpaceTexts none
; Thai Windows says ถอนการติดตั้ง for uninstall; NSIS's Thai text says
; ยกเลิกการติดตั้ง, which reads like "cancel the installation".
UninstallCaption "ถอนการติดตั้ง ${APP}"
UninstallButtonText "ถอนการติดตั้ง"

Var Update     ; 1 when started by the app's updater
Var From       ; a downloaded copy to remove after installing
Var OldDir     ; where an earlier version is installed

!define MUI_ICON "..\assets\icon.ico"
!define MUI_UNICON "..\assets\icon.ico"
!define MUI_WELCOMEFINISHPAGE_BITMAP "welcome.bmp"
!define MUI_UNWELCOMEFINISHPAGE_BITMAP "welcome.bmp"
!define MUI_ABORTWARNING
!define MUI_COMPONENTSPAGE_NODESC

!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipWhenUpdating
!define MUI_WELCOMEPAGE_TEXT "ตัวติดตั้งจะติดตั้ง ${APP} ${VERSION} ลงในเครื่องนี้$\r$\n$\r$\nถ้าเปิด ${APP} อยู่ ตัวติดตั้งจะปิดให้เอง$\r$\n$\r$\nกด ต่อไป เพื่อเริ่ม"
!insertmacro MUI_PAGE_WELCOME
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipWhenUpdating
!insertmacro MUI_PAGE_DIRECTORY
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipWhenUpdating
!define MUI_PAGE_HEADER_TEXT "ทางลัด"
!define MUI_PAGE_HEADER_SUBTEXT "เลือกว่าจะเปิด ${APP} จากที่ไหนได้บ้าง"
!define MUI_COMPONENTSPAGE_TEXT_TOP "ติ๊กทางลัดที่ต้องการ แล้วกด ติดตั้ง"
!define MUI_COMPONENTSPAGE_TEXT_COMPLIST "จะติดตั้ง:"
!insertmacro MUI_PAGE_COMPONENTS
!define MUI_PAGE_CUSTOMFUNCTION_SHOW ProgressShown
!insertmacro MUI_PAGE_INSTFILES
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipWhenUpdating
!define MUI_FINISHPAGE_TITLE "ติดตั้ง ${APP} เสร็จแล้ว"
!define MUI_FINISHPAGE_TEXT "ถอนการติดตั้งได้ทุกเมื่อที่ การตั้งค่า > แอป หรือในหน้าตั้งค่าของ ${APP}"
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_TEXT "เปิด ${APP}"
!define MUI_FINISHPAGE_RUN_FUNCTION OpenApp
!insertmacro MUI_PAGE_FINISH

!define MUI_PAGE_HEADER_TEXT "ถอนการติดตั้ง ${APP}"
!define MUI_PAGE_HEADER_SUBTEXT "ลบ ${APP} ออกจากเครื่องนี้"
!define MUI_COMPONENTSPAGE_TEXT_TOP "${APP} จะถูกลบออกจาก $INSTDIR พร้อมทางลัด ถ้าเปิดอยู่จะถูกปิดให้เอง"
!define MUI_COMPONENTSPAGE_TEXT_COMPLIST "สิ่งที่จะลบ:"
!insertmacro MUI_UNPAGE_COMPONENTS
!define MUI_PAGE_HEADER_TEXT "กำลังถอนการติดตั้ง"
!define MUI_PAGE_HEADER_SUBTEXT "โปรดรอสักครู่"
!define MUI_INSTFILESPAGE_FINISHHEADER_TEXT "ถอนการติดตั้งเสร็จแล้ว"
!define MUI_INSTFILESPAGE_FINISHHEADER_SUBTEXT "ลบ ${APP} ออกจากเครื่องนี้แล้ว"
!define MUI_INSTFILESPAGE_ABORTHEADER_TEXT "ถอนการติดตั้งไม่สำเร็จ"
!define MUI_INSTFILESPAGE_ABORTHEADER_SUBTEXT "${APP} ยังอยู่ในเครื่อง"
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "Thai"

VIProductVersion "${VER_1}.${VER_2}.${VER_3}.0"
VIAddVersionKey /LANG=${LANG_THAI} "ProductName" "${APP}"
VIAddVersionKey /LANG=${LANG_THAI} "FileDescription" "ตัวติดตั้ง ${APP}"
VIAddVersionKey /LANG=${LANG_THAI} "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=${LANG_THAI} "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=${LANG_THAI} "CompanyName" "${APP}"
VIAddVersionKey /LANG=${LANG_THAI} "LegalCopyright" "${REPO_URL}"

Function SkipWhenUpdating
  ${If} $Update == 1
    Abort
  ${EndIf}
FunctionEnd

Function ProgressShown
  ${If} $Update == 1
    SendMessage $HWNDPARENT ${WM_SETTEXT} 0 "STR:อัปเดต ${APP}"
    !insertmacro MUI_HEADER_TEXT "กำลังอัปเดต ${APP}" "เป็นเวอร์ชัน ${VERSION} แอปจะเปิดขึ้นมาใหม่เมื่อเสร็จ"
  ${EndIf}
FunctionEnd

; Not as administrator: through Explorer, as a double-click would.
Function OpenApp
  Exec '"$WINDIR\explorer.exe" "$INSTDIR\${EXE_NAME}"'
FunctionEnd

Function .onInstSuccess
  ${If} $Update == 1
  ${AndIfNot} ${Silent}
    Call OpenApp
  ${EndIf}
FunctionEnd

; Close the running app so its file can be replaced. The new program asks
; it to (it hides and quits at once); copies from before the installer
; don't know that request and are stopped instead.
!macro CloseApp ASK_WITH
  DetailPrint "กำลังปิด ${APP}…"
  SetDetailsPrint listonly
  ${If} ${FileExists} "${ASK_WITH}"
    ExecWait '"${ASK_WITH}" --quit' $0
  ${Else}
    StrCpy $0 1
  ${EndIf}
  ${If} $0 != 0
    nsExec::Exec 'taskkill /F /IM ${EXE_NAME} /IM StreamSound-windows-x64.exe'
    Pop $0
  ${EndIf}
  ; Windows may hold the file for a moment after the app has gone.
  StrCpy $R9 0
  ${Do}
    ClearErrors
    Delete "$INSTDIR\${EXE_NAME}"
    ${IfNot} ${Errors}
    ${OrIf} $R9 >= 40
      ${Break}
    ${EndIf}
    IntOp $R9 $R9 + 1
    Sleep 250
  ${Loop}
  SetDetailsPrint both
!macroend

Section "${APP} (จำเป็น)" SecApp
  SectionIn RO
  SetOutPath "$INSTDIR"
  DetailPrint "กำลังคัดลอกไฟล์…"
  SetDetailsPrint listonly
  File "/oname=StreamSound.new.exe" "${EXE}"
  SetDetailsPrint both
  !insertmacro CloseApp "$INSTDIR\StreamSound.new.exe"
  ClearErrors
  Rename "$INSTDIR\StreamSound.new.exe" "$INSTDIR\${EXE_NAME}"
  ${If} ${Errors}
    Delete "$INSTDIR\StreamSound.new.exe"
    MessageBox MB_ICONSTOP "ปิด ${APP} ที่เปิดอยู่ไม่ได้ ลองปิดแอปเองแล้วติดตั้งอีกครั้ง" /SD IDOK
    Abort
  ${EndIf}
  WriteUninstaller "$INSTDIR\${UNINSTALLER}"

  ; Moved to another folder: take the old one away.
  ${If} $OldDir != ""
  ${AndIf} $OldDir != $INSTDIR
    Delete "$OldDir\${EXE_NAME}"
    Delete "$OldDir\${UNINSTALLER}"
    RMDir "$OldDir"
    SetShellVarContext all
    Delete "$DESKTOP\${APP}.lnk"
    Delete "$SMPROGRAMS\${APP}.lnk"
  ${EndIf}

  WriteRegStr HKLM "${UNINST_KEY}" "DisplayName" "${APP}"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\${EXE_NAME},0"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "${UNINST_KEY}" "Publisher" "${APP}"
  WriteRegStr HKLM "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "${UNINST_KEY}" "UninstallString" '"$INSTDIR\${UNINSTALLER}"'
  WriteRegStr HKLM "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\${UNINSTALLER}" /S'
  WriteRegStr HKLM "${UNINST_KEY}" "URLInfoAbout" "${REPO_URL}"
  WriteRegStr HKLM "${UNINST_KEY}" "URLUpdateInfo" "${REPO_URL}/releases"
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKLM "${UNINST_KEY}" "EstimatedSize" $0

  ; Start with Windows from here from now on, if it was on.
  ReadRegStr $0 HKCU "${RUN_KEY}" "${RUN_NAME}"
  ${If} $0 != ""
    WriteRegStr HKCU "${RUN_KEY}" "${RUN_NAME}" '"$INSTDIR\${EXE_NAME}" --minimized'
  ${EndIf}

  ; The downloaded copy this replaces, with what its updates left beside it.
  ${If} $From != ""
  ${AndIf} $From != "$INSTDIR\${EXE_NAME}"
  ${AndIf} ${FileExists} $From
    ${GetFileExt} $From $0
    ${If} $0 == "exe"
      ${GetParent} $From $1
      ${GetBaseName} $From $2
      Delete $From
      Delete "$1\$2.old.exe"
      Delete "$From.update"
      RMDir $1
    ${EndIf}
  ${EndIf}

  ${If} $Update == 1
    SetAutoClose true
  ${EndIf}
SectionEnd

Section "ทางลัดบนเดสก์ท็อป" SecDesktop
  SetShellVarContext all
  CreateShortcut "$DESKTOP\${APP}.lnk" "$INSTDIR\${EXE_NAME}"
SectionEnd

Section "ทางลัดในเมนู Start" SecStartMenu
  SetShellVarContext all
  CreateShortcut "$SMPROGRAMS\${APP}.lnk" "$INSTDIR\${EXE_NAME}"
SectionEnd

; Remember the choices for the next time the installer is run by hand.
Section "-Remember"
  ${If} $Update == 0
    ${If} ${SectionIsSelected} ${SecDesktop}
      WriteRegDWORD HKLM "${APP_KEY}" "DesktopShortcut" 1
    ${Else}
      WriteRegDWORD HKLM "${APP_KEY}" "DesktopShortcut" 0
    ${EndIf}
    ${If} ${SectionIsSelected} ${SecStartMenu}
      WriteRegDWORD HKLM "${APP_KEY}" "StartMenuShortcut" 1
    ${Else}
      WriteRegDWORD HKLM "${APP_KEY}" "StartMenuShortcut" 0
    ${EndIf}
  ${EndIf}
SectionEnd

; After the sections, so it can refer to them.
Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "${APP} ใช้ได้กับ Windows 64 บิตเท่านั้น"
    Abort
  ${EndIf}
  SetRegView 64
  ${GetParameters} $R0
  StrCpy $Update 0
  ClearErrors
  ${GetOptions} $R0 "/UPDATE" $R1
  ${IfNot} ${Errors}
    StrCpy $Update 1
  ${EndIf}
  ${GetOptions} $R0 "/FROM=" $From
  ReadRegStr $OldDir HKLM "${UNINST_KEY}" "InstallLocation"

  ; Offer the shortcuts chosen last time (both at first). An update keeps
  ; the ones that are there and doesn't bring back ones that were deleted.
  ReadRegDWORD $R1 HKLM "${APP_KEY}" "DesktopShortcut"
  ${If} $Update == 1
  ${OrIf} $R1 == "0"
    !insertmacro UnselectSection ${SecDesktop}
  ${EndIf}
  ReadRegDWORD $R1 HKLM "${APP_KEY}" "StartMenuShortcut"
  ${If} $Update == 1
  ${OrIf} $R1 == "0"
    !insertmacro UnselectSection ${SecStartMenu}
  ${EndIf}
FunctionEnd

Function un.onInit
  SetRegView 64
FunctionEnd

Section "un.${APP}" UnSecApp
  SectionIn RO
  !insertmacro CloseApp "$INSTDIR\${EXE_NAME}"
  Delete "$INSTDIR\${UNINSTALLER}"
  RMDir "$INSTDIR"

  SetShellVarContext all
  Delete "$DESKTOP\${APP}.lnk"
  Delete "$SMPROGRAMS\${APP}.lnk"
  DeleteRegKey HKLM "${UNINST_KEY}"
  DeleteRegKey HKLM "${APP_KEY}"
  DeleteRegValue HKCU "${RUN_KEY}" "${RUN_NAME}"
  DeleteRegValue HKCU "${APPROVED_KEY}" "${RUN_NAME}"

  ; Downloaded updates.
  SetShellVarContext current
  RMDir /r "$TEMP\StreamSound"
SectionEnd

Section /o "un.การตั้งค่า (ชื่อเครื่อง ระดับเสียง และอื่น ๆ)" UnSecSettings
  SetShellVarContext current
  RMDir /r "$APPDATA\StreamSound"
SectionEnd
