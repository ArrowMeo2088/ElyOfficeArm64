; Per-user installer; pass each define the checks below name.
Unicode true
!include "MUI2.nsh"

!ifndef APP_NAME
  !error "pass -DAPP_NAME"
!endif
!ifndef APP_SLUG
  !error "pass -DAPP_SLUG"
!endif
!ifndef VERSION
  !error "pass -DVERSION"
!endif
!ifndef STAGE
  !error "pass -DSTAGE, a folder from packaging/stage.sh"
!endif
!ifndef ICON
  !error "pass -DICON"
!endif
!ifndef TYPES
  !error "pass -DTYPES, from packaging/windows/types.sh"
!endif
!include "${TYPES}"
!ifndef FILES
  !error "pass -DFILES, from packaging/windows/files.sh"
!endif
!include "${FILES}"
!ifndef OUTFILE
  !define OUTFILE "${APP_SLUG}-${VERSION}-setup.exe"
!endif
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\Ely Office ${APP_NAME}"

!define MUI_ICON "${ICON}"
!define MUI_UNICON "${ICON}"
Name "${APP_NAME}"
OutFile "${OUTFILE}"
RequestExecutionLevel user
InstallDir "$LOCALAPPDATA\Programs\Ely Office\${APP_NAME}"
InstallDirRegKey HKCU "Software\Ely Office\${APP_NAME}" "InstallDir"
SetCompressor /SOLID lzma
AllowSkipFiles off

!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "English"

LangString DeleteData ${LANG_SIMPCHINESE} "同时删除 ${APP_NAME} 的缓存、账号与本地历史吗？你的文档不受影响。"
LangString DeleteData ${LANG_ENGLISH} "Also delete the cache, accounts and local history of ${APP_NAME}? Your documents stay."

LangString Busy ${LANG_SIMPCHINESE} "${APP_NAME} 的文件正被占用，未作任何改动。请关闭 ${APP_NAME} 后重试："
LangString Busy ${LANG_ENGLISH} "A file of ${APP_NAME} is in use; nothing was changed. Close ${APP_NAME} and try again:"
LangString Stuck ${LANG_SIMPCHINESE} "旧版的文件放回原处失败，留在："
LangString Stuck ${LANG_ENGLISH} "Could not put the old files back; they are in:"
LangString Left ${LANG_SIMPCHINESE} "新版已装好，旧版的文件删不掉，留在："
LangString Left ${LANG_ENGLISH} "The new version is installed; the old files could not be deleted from:"
LangString NoList ${LANG_SIMPCHINESE} "找不到安装清单，未删除任何文件："
LangString NoList ${LANG_ENGLISH} "No install list found; nothing was deleted:"

!macro ElyLegacy ID PATTERN
  FindFirst $R3 $R4 "$INSTDIR\${PATTERN}"
  ely_legacy_${ID}:
    StrCmp $R4 "" ely_legacy_end_${ID}
    FileWrite $R2 "$R4$\r$\n"
    FindNext $R3 $R4
    Goto ely_legacy_${ID}
  ely_legacy_end_${ID}:
  FindClose $R3
!macroend

; Sets $R0 to what an earlier install owns.
Function ElyOwned
  StrCpy $R0 ""
  InitPluginsDir
  ; A copy, as this install rewrites the list.
  IfFileExists "$INSTDIR\.ely-files" 0 unlisted
  StrCpy $R0 "$PLUGINSDIR\owned.txt"
  StrCpy $R7 "listed"
  CopyFiles /SILENT "$INSTDIR\.ely-files" "$R0"
  Goto done
unlisted:
  ; Older releases: only the folder our key names.
  ReadRegStr $R1 HKCU "Software\Ely Office\${APP_NAME}" "InstallDir"
  StrCmp $R1 "$INSTDIR" 0 done
  IfFileExists "$INSTDIR\${APP_SLUG}.exe" 0 done
  StrCpy $R0 "$PLUGINSDIR\legacy.txt"
  FileOpen $R2 "$R0" w
  FileWrite $R2 "libreoffice$\r$\ntessdata$\r$\ngit$\r$\ncontent$\r$\nlocales$\r$\nlicenses$\r$\n"
  FileWrite $R2 "${APP_SLUG}.ico$\r$\n"
  !insertmacro ElyLegacy exe "*.exe"
  !insertmacro ElyLegacy dll "*.dll"
  !insertmacro ElyLegacy pak "*.pak"
  !insertmacro ElyLegacy bin "*.bin"
  !insertmacro ElyLegacy dat "*.dat"
  !insertmacro ElyLegacy json "*.json"
  FileClose $R2
done:
FunctionEnd

!macro ElyAside UN
; Strips the line end from $R3.
Function ${UN}ElyTrim
again:
  StrCpy $R6 "$R3" 1 -1
  StrCmp $R6 "$\r" cut
  StrCmp $R6 "$\n" cut
  Return
cut:
  StrCpy $R3 "$R3" -1
  Goto again
FunctionEnd

; Moves $R0's names into .ely-old; $R5 names a blocker.
Function ${UN}ElyAside
  StrCpy $R5 ""
  StrCpy $R9 "$INSTDIR\.ely-old"
  RMDir /r "$R9"
  IfFileExists "$R9" 0 +3
    StrCpy $R5 "$R9"
    Return
  CreateDirectory "$R9"
  ClearErrors
  FileOpen $R2 "$R0" r
  IfErrors 0 check
    StrCpy $R5 "$R0"
    Return
check:
  ; A running or locked file refuses writing.
  ClearErrors
  FileRead $R2 $R3
  IfErrors checked
  Call ${UN}ElyTrim
  StrCmp $R3 "" check
  IfFileExists "$INSTDIR\$R3\*.*" check
  IfFileExists "$INSTDIR\$R3" 0 check
  ClearErrors
  FileOpen $R4 "$INSTDIR\$R3" a
  IfErrors refused
  FileClose $R4
  Goto check
checked:
  FileSeek $R2 0 SET
move:
  ClearErrors
  FileRead $R2 $R3
  IfErrors moved
  Call ${UN}ElyTrim
  StrCmp $R3 "" move
  IfFileExists "$INSTDIR\$R3" 0 move
  ClearErrors
  Rename "$INSTDIR\$R3" "$R9\$R3"
  IfErrors 0 move
  StrCpy $R5 "$INSTDIR\$R3"
  FileClose $R2
  Call ${UN}ElyBack
  Return
moved:
  FileClose $R2
  Return
refused:
  StrCpy $R5 "$INSTDIR\$R3"
  FileClose $R2
  RMDir "$R9"
FunctionEnd

; Puts every name in $R0 back from .ely-old.
Function ${UN}ElyBack
  FileOpen $R2 "$R0" r
back:
  ClearErrors
  FileRead $R2 $R3
  IfErrors done
  Call ${UN}ElyTrim
  StrCmp $R3 "" back
  IfFileExists "$R9\$R3" 0 back
  Rename "$R9\$R3" "$INSTDIR\$R3"
  Goto back
done:
  FileClose $R2
  RMDir "$R9"
  IfFileExists "$R9" 0 +2
    MessageBox MB_ICONSTOP "$(Stuck)$\r$\n$R9" /SD IDOK
FunctionEnd
!macroend
!insertmacro ElyAside ""
!insertmacro ElyAside "un."

; Removes what this install created, then restores the old.
Function .onInstFailed
  StrCmp $R8 "aside" 0 done
  FileOpen $R2 "$PLUGINSDIR\created.txt" r
drop:
  ClearErrors
  FileRead $R2 $R3
  IfErrors dropped
  Call ElyTrim
  StrCmp $R3 "" drop
  RMDir /r "$INSTDIR\$R3"
  Delete "$INSTDIR\$R3"
  Goto drop
dropped:
  FileClose $R2
  Call ElyBack
  Delete "$INSTDIR\.ely-files"
  StrCmp $R7 "listed" 0 done
  CopyFiles /SILENT "$R0" "$INSTDIR\.ely-files"
done:
FunctionEnd

Section "Install"
  ; An earlier install moves aside whole, or nothing changes.
  Call ElyOwned
  StrCmp $R0 "" fresh
  Call ElyAside
  StrCmp $R5 "" 0 +2
    StrCpy $R8 "aside"
  StrCmp $R5 "" fresh
  MessageBox MB_ICONSTOP "$(Busy)$\r$\n$R5" /SD IDOK
  SetErrorLevel 2
  Quit
fresh:
  InitPluginsDir
  FileOpen $R2 "$PLUGINSDIR\created.txt" w
  !insertmacro ElyNoteNew
  FileClose $R2
  ; A file that cannot be written fails the install.
  ClearErrors
  SetOutPath "$INSTDIR"
  File /r "${STAGE}\*.*"
  File "/oname=${APP_SLUG}.ico" "${ICON}"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  !insertmacro ElyWriteOwned
  IfErrors 0 +2
    Abort
  RMDir /r "$INSTDIR\.ely-old"
  IfFileExists "$INSTDIR\.ely-old" 0 +2
    MessageBox MB_ICONEXCLAMATION "$(Left)$\r$\n$INSTDIR\.ely-old" /SD IDOK
  CreateShortcut "$SMPROGRAMS\${APP_NAME}.lnk" "$INSTDIR\${APP_SLUG}.exe" "" "$INSTDIR\${APP_SLUG}.ico"
  WriteRegStr HKCU "Software\Ely Office\${APP_NAME}" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "${APP_NAME}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "ZacharyZhang"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "URLInfoAbout" "https://zacharyzhang.com"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\${APP_SLUG}.ico"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  !insertmacro ElyTypesInstall
SectionEnd

Section "Uninstall"
  StrCpy $R0 "$INSTDIR\.ely-files"
  IfFileExists "$R0" listed
  MessageBox MB_ICONSTOP "$(NoList)$\r$\n$INSTDIR" /SD IDOK
  SetErrorLevel 2
  Quit
listed:
  Call un.ElyAside
  StrCmp $R5 "" gone
  MessageBox MB_ICONSTOP "$(Busy)$\r$\n$R5" /SD IDOK
  SetErrorLevel 2
  Quit
gone:
  RMDir /r "$INSTDIR\.ely-old"
  Delete "$R0"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\${APP_NAME}.lnk"
  DeleteRegKey HKCU "Software\Ely Office\${APP_NAME}"
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
  !insertmacro ElyTypesUninstall
  MessageBox MB_YESNO|MB_DEFBUTTON2 "$(DeleteData)" /SD IDNO IDNO done
  RMDir /r "$LOCALAPPDATA\ElyOffice\${APP_SLUG}"
done:
SectionEnd
