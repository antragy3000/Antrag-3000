; Zusaetzliche NSIS-Aktionen beim Installieren (Windows).
;
; NSIS_HOOK_POSTINSTALL laeuft, nachdem die App-Dateien nach $INSTDIR
; kopiert wurden. Wir legen das mitgelieferte Benutzerhandbuch unter
;   Dokumente\Antrag 3000\
; (derselbe Ordner, in dem die App die Projekte anlegt - vorher stand hier
; versehentlich "Antrag3000" ohne Leerzeichen, das ergab zwei Ordner).
; ab und oeffnen es nach einer normalen (interaktiven) Installation einmal.
;
; Bei einer STILLEN Installation (z. B. dem automatischen App-Selbstupdate,
; das den Installer mit /S aufruft) wird das PDF zwar aktualisiert, aber
; NICHT geoeffnet - sonst poppte bei jedem Update der Reader auf.
;
; ${Silent} stammt aus LogicLib, das die Tauri-NSIS-Vorlage bereits einbindet.

!macro NSIS_HOOK_POSTINSTALL
  ; Aufraeumen nach dem frueheren Tippfehler: nur das alte Handbuch-PDF
  ; loeschen und den Ordner "Antrag3000" nur dann, wenn er danach LEER ist
  ; (RMDir ohne /r) - fremde Dateien darin bleiben unangetastet.
  Delete "$DOCUMENTS\Antrag3000\Benutzerhandbuch Antrag 3000.pdf"
  RMDir "$DOCUMENTS\Antrag3000"
  CreateDirectory "$DOCUMENTS\Antrag 3000"
  CopyFiles /SILENT "$INSTDIR\Benutzerhandbuch Antrag 3000.pdf" "$DOCUMENTS\Antrag 3000"
  ${IfNot} ${Silent}
    ExecShell "open" "$DOCUMENTS\Antrag 3000\Benutzerhandbuch Antrag 3000.pdf"
  ${EndIf}
!macroend
