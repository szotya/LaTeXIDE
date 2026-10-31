# Kutatói és fejlesztői IDE — termékirány és minőségi szerződés

Frissítve2026-10-31. A felhasználó megerősítette: tényleges, igényes IDE-t akar, JetBrains és VSCode ihlettel, kutatóknak és fejlesztőknek. A prémium jelleg következetes munkafolyamatot és megbízható viselkedést jelent. A témaválasztó vagy egy új panel önmagában nem teljesíti ezt a célt.

## A munkaterület logikája

Bal oldalon projekt és navigáció, középen natív forrás, jobb oldalon váltható PDF/Asszisztens, alul problémák és fordítási kimenet. A dokumentum marad a munkaterület központja; a panelek megnyithatók, elrejthetők és méretezhetők. A jobb oldali módváltás megőrzi a forrás terét kisebb ablakban is. Közös ritmus, tipográfia, szemantikus színek, fókuszállapotok és billentyűparancsok kötik össze a funkciókat.

Személyre szabható és tartós: System/Light/Dark, négy palettacsalád, betűméret, kiegészítés be/ki, panelméretek és jobb oldali mód. A későbbi kutatói/fejlesztői munkaterületprofilok ezek fölé épüljenek, ne eltérő kinézetű alkalmazásokat hozzanak létre.

## Valós képességek és következő lépések

| Munkafolyamat | Jelenlegi állapot | Következő érdemi lépés |
| --- | --- | --- |
| Helyi projekt, natív forrás és PDF | Megvalósult Macen; TeX Live/MCP alapfolyamat ellenőrzött. | Dokumentumzárás/evikció, külső fájlfigyelés, helyreállítás; a tíz megtartott dokumentum prototípuslimitjének kezelése. |
| Billentyűzet és közös szerkesztési előzmény | Agent edit/save utáni tényleges Cmd+Z/redo igazolt. Fókuszolt mező saját undo-ja külön út. | Teljes IME/hardware/accessibility kvalifikáció, hosszabb vegyes user/agent stressz. |
| LaTeX-kiegészítés | Közös Rust-katalógus és korlátos projektindex: saját makrók, ref/cref címkék, cite/BibTeX-kulcsok, dirty buffer/undo és friss natív elfogadás. | [Ellenőrzés és korlátok](project-completion-verification.md); szemantikus include-gráf, dinamikus deklarációk, csomagadatok és language-service integráció. |
| Definícióra ugrás | Natív Cmd+kattintás saját makróra, ref-címkére és cite-kulcsra; friss dirty/disk index, pontos célkijelölés/görgetés, forrásállapot és undo megőrzése. | [Ellenőrzés](definition-navigation-verification.md); teljes TeX include/scope, többes találatválasztó, vissza/előre navigáció és célplatformi natív adapterek. |
| Kutatói dokumentumértelmezés | Lexikális színezés, matematika, fordítási hibák, forrás/PDF és korlátos definíciónavigáció egy munkaterületen. | Outline/szimbólumnavigáció, teljes hivatkozásfeloldás, SyncTeX oda-vissza, egyetemi sablonkorpusz és reprodukálható csomagkészlet. |
| Fejlesztői projektműveletek | Projektfa, Quick Open/parancsok, natív Find, fordítás/diagnosztika és projektmakró-definícióra ugrás. | Tartalomkeresés, átnevezés, Git-diff/history és jól körülhatárolt bővítési API. Ezek még nem kész funkciók. |
| Natív asszisztens | A felhasználó Codex CLI-t választott; helyi stdio-kliens, streaming és MCP dokumentumkapcsolat megvalósult. Tényleges modell olvasott/szerkesztett; Cmd+Z/redo és friss PDF-folyamat igazolt. | [Pontos ellenőrzés](intelligence-verification.md); alfa CLI/provider-kvalifikáció, beszélgetési előzmény export/perzisztencia további termékmunka. |
| Mac/Windows/Linux | Macen fejlesztünk és futtatunk; közös Rust-szerződések hordozhatók. | Windows/Linux natív futtatás a rendelkezésre álló célgépeken. Hiányát dokumentáljuk, nem újabb Mac-teszttel pótoljuk. |

## Minőségi kapuk

- Egy műveletnek egyértelmű célja és állapotgazdája van. A forrás/undo Rustban hiteles, a natív vezérlő megjelenít és a megfelelő tranzakciót küldi.
- A Ctrl/Cmd-modifier és az aktív szövegmező számít. Magyar kiosztáson a tényleges karaktert követjük, nem az amerikai fizikai Z pozíciót. A prompt/kereső szerkesztésének visszavonása nem módosíthatja a dokumentumot.
- A személyre szabott elrendezés és a még el nem küldött munka panelváltáskor ne vesszen el. Projektváltáskor az asszisztens ne dolgozhasson tovább véletlenül az előző projekt kontextusával.
- A kiegészítő csak érvényes aktuális jelöltet szúrhat be. Tab/Enter/Escape a helyzetnek megfelelően működjön; IME-t, meglévő argumentumot és saját szöveget ne rontson el.
- Az asszisztens valódi állapotot, streamet, eszközműveletet és hibát jelenítsen meg. Ne legyen szimulált válasz, automatikus modellindítás vagy kitalált „csatlakozott” jelzés.
- Az agent a megnyitott IDE-dokumentumokat MCP-n szerkessze, közös visszavonással. Az első Codex-adapter nyers fájlrendszere read-only; mentés külön művelet. A helyi agentfolyamat nem jelent automatikusan helyi modellfuttatást.
- A széles körű sablon-, teljesítmény-, hozzáférhetőségi és platformteszt a késztermék feltétele. Debug build és kontrollált render nem kiadási minősítés.

Aktív részletes design és terv: [IDE-intelligencia](superpowers/specs/2026-10-31-ide-intelligence-design.md), [megvalósítási terv](superpowers/plans/2026-10-31-ide-intelligence.md). A tényleges állapotot mindig a [fejlesztői átadás](DEVELOPER_HANDOFF.md), az ellenőrzések és a munkanapló határozza meg; ez a dokumentum a következetes termékdöntéseket vezeti.
