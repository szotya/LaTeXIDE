# LaTeX IDE

Önálló LaTeX IDE egyetemi hallgatóknak, helyi projektekhez és helyi agentek MCP-kapcsolatához.

**Tesztelőknek:** [magyar útmutató hibák, hiányzó funkciók és új ötletek bejelentéséhez](docs/TESTING.hu.md) · [új issue létrehozása](https://github.com/szotya/LaTeXIDE/issues/new/choose). Lépések, kitöltött példák és három külön űrlap; programozói ismeret vagy fejlesztői meghívás nem szükséges.

**Licenc:** [PolyForm Perimeter 1.0.1](LICENSE). A projekt forrása megtekinthető, használható és módosítható a licenc feltételeivel; a kód másoknak kínált versengő termékben való felhasználásához külön engedély szükséges. Ez source-available licenc. A nyilvános repó nem ad írási vagy merge-jogot; issue és pull request beküldhető a GitHub beállításai szerint. [Licenc, közreműködés és hozzáférés](LICENSING.md).

Választott architektúra:

- Közös Rust alkalmazásmag.
- macOS: Swift, SwiftUI és AppKit; kizárólag Apple Silicon (M-sorozat).
- Windows 11 x64: C++/WinRT és WinUI 3, Intel és AMD gépekre.
- Közvetlen, az IDE-ben visszavonható agent-szerkesztés MCP-n át.
- Ellenőrzött forrású, nyílt LaTeX-fordító: kiadási irány TeX Live 2026 + latexmk/XeLaTeX.

A RAM-hatékonyság és a natív rendszerintegráció tervezési prioritás.

## Jelenlegi állapot

A közös Rust dokumentummag, C ABI, helyi IPC, fordításvezérlés és hivatalos MCP stdio bridge működik Macen. A natív arm64 alkalmazás új, tömör fejlesztői felületet kapott: projektfa, fájlfülek, sorszámok, aktuális sor, gyors fájlmegnyitás/parancsok, natív kereső, méretezhető PDF- és kimenetpanelek. Négy előre bekészített paletta (Graphite, Ocean, Dune, High Contrast), mind világos/sötét változattal; külön rendszerkövető vagy fix megjelenés. A közös Rust lexer elkülöníti a LaTeX-parancsokat, környezeteket, megjegyzéseket, matematikát és további szintaxiselemeket a saját szövegtől.

A gépelés közbeni LaTeX-parancskiegészítés natív jelöltlistát ad, nyilakkal és Tab/Enter beszúrással, argumentumpozícióval és közös visszavonással. A PDF/Asszisztens inspectorban valódi, helyi Codex CLI-hez kapcsolódó panel működik: streaming válasz, eszközállapot, megszakítás és az IDE MCP-sessionjén át szerkesztés. Panelméretek/mód és kiegészítési beállítás tartós; az el nem küldött piszkozat panelváltáskor megmarad. A helyi CLI a beállított modellszolgáltatót használja, nem feltétlen offline modellt.

Enter elfogadja a kijelölt jelöltet és bezárja a listát; Esc elfogadás nélkül zár. Az Enter-fázis 5 valódi billentyűzetes teszttel bővült; a bejelentett beragadás akkor nem volt reprodukálható. [Enter-ellenőrzés](docs/completion-enter-verification.md).

**Cmd+kattintás** a saját makrón, `\ref`-címkén vagy `\cite` BibTeX-kulcson megnyitja és kijelöli a pontos definíciót, a még el nem mentett változásokkal együtt. A navigáció megőrzi a szerkesztési előzményeket és a forrás nézetállapotát. Végső Rust 69 PASS; natív Mac 73 futtatott / 69 PASS / 4 opt-in kihagyva / 0 hiba, IPC10/MCP1 és közös MSVC x64 check PASS. [Bizonyítékok és határok](docs/definition-navigation-verification.md), [review és javítások](docs/definition-navigation-review.md). A feloldás a korlátos projektindex sorrendjét követi; teljes TeX-scope feloldás még nincs.

2026-10-31: a kiegészítés már a projekt saját makróit, ref-címkéit és BibTeX-kulcsait ismeri, a még el nem mentett agentváltozásokkal együtt. A jelöltek mutatják a definíció helyét; a kulcslisták és argumentumok megmaradnak, elfogadás előtt frissességellenőrzés történik. A korlátos, lexikális index részleges lefedettségét a felület jelzi. A projektkiegészítés lezárásakor Rust 56 és 53 aktív natív Mac teszt PASS; új projektpopup-render 1, IPC 9, MCP 1, compiler-runtime 3 PASS. Élő makró/ref/cite beszúrás, dirty másik fájl, Cmd+Z/redo és friss PDFKit igazolt. [Pontos bizonyítékok és parserhatárok](docs/project-completion-verification.md), [review és javítások](docs/project-completion-review.md). A Codex-integráció kísérleti; a további szemantikus IDE-képességeket a [termékirány](docs/IDE_PRODUCT_DIRECTION.md) vezeti.

A Windows natív forrásprojekt az új felület és közös paletták portját is tartalmazza; tényleges MSBuild/link/futtatás még nem történt. A gyors fájlszűrő/parancsmenü/PDF képességei részlegesen térnek el a Mactől, lásd [Windows UI-port](docs/windows-ui-port.md). Linux későbbi cél. A korábbi 170 MiB debug RSS-pillanatkép a felületátalakítás előtti kis projektre vonatkozik; új UI- vagy release-RAM-mérés nincs. A teljes hallgatói kiadás még nincs kész.

Helyi Mac build: `bash scripts/build_macos.sh`. Indítás: `bash script/build_and_run.sh --verify`, vagy a `dist/LaTeXIDE.app` megnyitása. A Codex Run gomb is ezt a scriptet használja. Futó példány esetén előbb szabályosan mentsünk és lépjünk ki. Windows eljárás és ellenőrzési határ: [windows-validation.md](docs/windows-validation.md).

A projektbe telepített TeX Live 2026 aláírt/hitelesített beszerzési útból származik. A magyar Unicode/multifile minta offline is fordult. A malware-scan és Windows-futtatás nincs elvégezve; a Tectonic-jelölt advisory-értékelése külön dokumentált.

## Fejlesztői belépési pont

- [Windows/Linux fejlesztői munkatér](docs/development/PLATFORM_DEVELOPMENT.md): közzétett kezdőágak, saját mappák, pontos módosítási határok és a Mac által vezetett [paritásmátrix](docs/development/PLATFORM_PARITY.md).

- [Párhuzamos fejlesztési csomagok](docs/development/README.md): 24 fő + 20 választható terv, mind a 226 auditkövetelmény kizárólagos gazdával, fájlhatárral és elfogadási teszttel.
- [GitHub-repó és tiszta clone](docs/development/REPOSITORY.md), [közreműködés](CONTRIBUTING.md), [audit utáni állapot](docs/development/BASELINE_DELTA.md).
- [Aktuális fejlesztői átadás](docs/DEVELOPER_HANDOFF.md): tényleges állapot, parancsok, Mac-es tapasztalatok, Windows/Linux következmények.
- [Folyamatos munkanapló](docs/implementation-progress.md): döntések, elvégzett és nyitott feladatok, ellenőrzési bizonyíték.
- [Hordozható projekt-skill](skills/latexide-platform-development/SKILL.md): közös szerződések és célplatformi reference-ek.
- [Felületi és színezési szerződés](skills/latexide-platform-development/references/ui.md), [UI-review és javítások](docs/developer-ui-review.md).
- [Billentyűzet/kiegészítés/asszisztens szerződés](skills/latexide-platform-development/references/intelligence.md), [integrált review és javítások](docs/intelligence-review.md).
- [Fordítóértékelés](docs/compiler-assessment.md) és [helyi TeX Live útmutató](tools/texlive/README.md).

Mac fejlesztői ellenőrzés a projektgyökérből: `bash scripts/cargo-local.sh test --workspace --offline`. A további parancsokat és azok aktuális lefedettségét az átadási dokumentum tartalmazza.

A követelményeket, rendszerhatárokat, első kiadást és ellenőrzési feltételeket a [felhasználó által pontosításokkal elfogadott rendszerterv](docs/superpowers/specs/2026-10-30-native-latex-ide-design.md) tartalmazza. Az eredeti tervezhetőségi ellenőrzés és a későbbi pontosítások külön szerepelnek benne.
