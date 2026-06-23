; Zusaetzliche NSIS-Aktionen beim Installieren (Windows).
;
; NSIS_HOOK_POSTINSTALL laeuft, nachdem die App-Dateien nach $INSTDIR
; kopiert wurden. Wir legen das mitgelieferte Benutzerhandbuch unter
;   Dokumente\Antrag3000\
; ab und oeffnen es nach einer normalen (interaktiven) Installation einmal.
;
; Bei einer STILLEN Installation (z. B. dem automatischen App-Selbstupdate,
; das den Installer mit /S aufruft) wird das PDF zwar aktualisiert, aber
; NICHT geoeffnet - sonst poppte bei jedem Update der Reader auf.
;
; ${Silent} stammt aus LogicLib, das die Tauri-NSIS-Vorlage bereits einbindet.

!macro NSIS_HOOK_POSTINSTALL
  CreateDirectory "$DOCUMENTS\Antrag3000"
  CopyFiles /SILENT "$INSTDIR\Benutzerhandbuch Antrag 3000.pdf" "$DOCUMENTS\Antrag3000"
  ${IfNot} ${Silent}
    ExecShell "open" "$DOCUMENTS\Antrag3000\Benutzerhandbuch Antrag 3000.pdf"
  ${EndIf}
!macroend
