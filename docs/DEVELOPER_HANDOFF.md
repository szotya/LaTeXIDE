# Fejlesztői átadás – közös Rust mag, macOS, Windows és későbbi Linux

Frissítve: 2026-10-31. Ez élő fejlesztői dokumentum, nem késztermék-bejelentés.

**Fejlesztésszervezési kiegészítés 2026-10-31:** [44 párhuzamos terv](development/README.md), [kizárólagos fájlhatárok és P00 szerződéskapu](superpowers/specs/2026-10-31-parallel-development-design.md), [privát GitHub-alap](development/REPOSITORY.md). A repó `szotya/LaTeXIDE`, private, létrehozva; a tényleges push/CI lezárást a ledger rögzíti. A 226 követelmény tervezett munka, a definíciós inkrementum utáni [baseline-delta](development/BASELINE_DELTA.md) megőrzésével. Új termékszolgáltatás ebben a fázisban nem készült.

**Folytatva 2026-10-31**, a felhasználó kifejezett kérésére. A [szüneti jegyzet](PAUSED-HANDOFF.md) történeti checkpoint. Legújabb fejlesztés: [Cmd+kattintásos definícióra ugrás](definition-navigation-verification.md), [review és javítások](definition-navigation-review.md). Az [Enter/popup](completion-enter-verification.md), [projektkiegészítés](project-completion-verification.md), [billentyűzet/Codex](intelligence-verification.md) és [UI](ui-verification.md) dokumentumai a saját korábbi fázisuk bizonyítékai.

A felhasználó kifejezetten kérte minden érdemi Macen végzett munka Markdownban és fejlesztői skillben való vezetését. A jelenlegi állapot és a platformok közötti szerződések itt találhatók; a változások időrendi nyoma az [implementációs napló](implementation-progress.md). A [projektbeli skill](../skills/latexide-platform-development/SKILL.md) a folytatáshoz szükséges döntési és karbantartási útmutató.

## 1. Elfogadott termék és platformok

- Saját, önálló LaTeX IDE hallgatóknak, elsőként helyi fájlokkal és helyi fordítással.
- Közös Rust mag; macOS-en Swift/AppKit–SwiftUI és PDFKit; Windowson C++/WinRT–WinUI 3 és natív PDF-megjelenítés.
- Mac: kizárólag M-sorozatú Apple Silicon, arm64, macOS 14+. Intel Mac nem cél.
- Windows: Windows 11 x64, Intel és AMD gépekre. A felhasználó külön tisztázta: nem kér 32 bites x86 alkalmazást. Windows ARM64 nem cél.
- Linux: későbbi fejlesztési célhoz kérünk átadható kontextust. Natív Linux-felület még nincs kiválasztva vagy megvalósítva.
- Külső, helyben futó agent MCP-n csatlakozik. Közvetlenül szerkeszthet és menthet, szerkesztésenkénti felhasználói jóváhagyás nélkül. Az IDE-ben a változások visszavonhatók.
- Natív asszisztenspanel meglévő helyi agenthez; elsőként a felhasználó által választott Codex CLI. Gépelés közbeni LaTeX-parancsjelöltek, személyre szabható, következetes kutatói/fejlesztői munkaterület. Részletes minőségi szerződés: [termékirány](IDE_PRODUCT_DIRECTION.md).
- RAM-hatékonyság és natív rendszerintegráció fontos. A nyelvválasztás nem mérési eredmény; első debug RSS-pillanatkép: 170 MiB kis projekttel és egyoldalas PDF-fel, nem release-mérés.
- A felhasználó megerősítette: jelenleg nincs Windows-tesztgép vagy runner. A Windows-futtatás hiánya ismert külső feltétel; emiatt ne állítsunk kész kétplatformos terméket.

A részletes szerződések és MVP-határok: [elfogadott rendszerterv](superpowers/specs/2026-10-30-native-latex-ide-design.md). Az alap lépései: [megvalósítási terv](superpowers/plans/2026-10-30-native-ide-foundation.md). A legújabb kért fejlesztői felület: [UI-design](superpowers/specs/2026-10-30-developer-ui-design.md), [UI-terv](superpowers/plans/2026-10-30-developer-ui.md). Több előkészített paletta, rendszerkövető/fix megjelenés és a saját szövegtől elkülönülő LaTeX-színezés kötelező. A legutóbbi felhasználói pontosítás elsőbbséget élvez régebbi tervszöveggel szemben.

## 2. Mi létezik ténylegesen?

| Rész | Jelenlegi állapot | Bizonyíték / belépési pont |
| --- | --- | --- |
| Rust dokumentummag | Megvalósult; 13 szerződésteszt átment. | `crates/latexide-core`, `evidence/core-green.log` |
| Közös szintaxis és paletták | Rust lexer 8 tesztje átment; 4 paletta × világos/sötét, külön System/Light/Dark preferencia. | `crates/latexide-core/src/syntax.rs`, `shared/ide-theme`, `evidence/intelligence-2026-10-31/rust-tests.log` |
| Projektismerő LaTeX-kiegészítés | Közös Rust-katalógus és korlátos projektindex: saját makrók, ref-címkék, cite BibTeX-kulcsok; dirty dokumentumok és undo követése. 21 új projektkontraktus PASS. Natív típus/eredethely, friss elfogadás, egy undo. | `crates/latexide-core/src/{completion,tex_scan,project_index}.rs`, `docs/project-completion-verification.md` |
| Definícióra ugrás | Saját makró/ref/cite pontos lexikális célhelye, friss dirty/disk index, Unicode/BOM/CRLF koordináták; natív Cmd+kattintás, kijelölés/görgetés/fókusz. 13 közös és 11 natív új eset PASS; egy review, mindhárom megállapítás reprodukálva javítva. | `crates/latexide-core/src/definition.rs`, `docs/definition-navigation-verification.md` |
| C ABI és közös runtime | Megvalósult. Életciklus, közvetlen edit, mentés, undo/redo C-harnessből átment. | `include/latexide_core.h`, `crates/latexide-ffi`, `tests/ffi_contract.c` |
| Helyi IPC | Macen 10 teszt PASS: korábbi szerződések, dirty másik fájl, megőrzött méret/mtime melletti frissítés és pontos definíció/revízió/nonmutáció. | `tests/ipc_contract.py`, `evidence/definition-navigation-2026-10-31/ipc-tests-final.log` |
| Build supervisor | Megvalósult; nyolc kontrollált gyermekfolyamatos/diagnosztikai teszt átment. | `crates/latexide-build`, `evidence/rust-workspace-tests.log` |
| Runtime build-integráció | Megvalósult; három integrációs teszt átment: valódi PDF/revízió, agent-konfiguráció tiltása és credential-env kiszűrése. | `tests/compiler_runtime_contract.py`, `evidence/compiler-runtime-tests.log` |
| TeX Live 2026 Mac | Projektbe telepítve. Hivatalos installer aláírás/hash ellenőrzött; Unicode/multifile/fontspec/amsmath és változatlan appfixture offline is fordult. | `tools/texlive/README.md`, `evidence/acquisition/compiler-review/texlive-runtime-manifest.json` |
| TeX Live Windows | Installer eredete/GPG/SHA-512 ellenőrzött, x64 profil rendelkezésre áll. Nem futott Windowson. | `tools/texlive/windows-x64.profile` |
| Rust Windows-check | Mind az öt aktuális Rust crate MSVC x64 type-check PASS a definíció-API-val és pontos indexrangekkel együtt. Nem natív app-link vagy futtatás. | `evidence/definition-navigation-2026-10-31/windows-rust-check-final.log` |
| Natív Mac GUI | Arm64 `.app`; projektkiegészítés és Cmd+kattintásos definíció, tartós panelek/PDF/Asszisztens. 69 aktív natív teszt PASS (73 futtatottból 4 opt-in kihagyva), benne 11 definíciós és 5 Enter/popup eset. A Cmd-mouse események saját valódi forrásvezérlőkön teszteltek; az új csomag élő Cmd-click köre unrun. | `apps/macos`, `docs/definition-navigation-verification.md` |
| Natív Codex-asszisztens | Valódi app-server subprocess, stream/eszközállapot/Stop; privát IDE-session, read-only nyers fájlrendszer, kilenc IDE-eszközre szűkített engedély. Kontrollált subprocess 16 aktív teszt PASS; tényleges modell olvasott/szerkesztett, billentyűzettel visszavonva. CLI 0.166.0-alpha.5.1, kísérleti integráció. | `apps/macos/Sources/LaTeXIDEKit/Assistant`, `docs/intelligence-verification.md` |
| Natív Windows GUI | C++/WinRT–WinUI forrásprojekt, új UI/színezés/paletták és buildscript. Strukturális és hordozható helper ellenőrzés PASS. MSBuild/link/futtatás nincs igazolva. | `apps/windows`, `docs/windows-ui-port.md`, `docs/windows-validation.md` |
| MCP stdio bridge | Hivatalos rmcp 3.8.0; tényleges stdio roundtrip és futó GUI-session szerkesztése/mentése átment. | `crates/latexide-mcp`, `tests/mcp_contract.py` |
| Linux | Nincs build-/futtatási eredmény vagy felület. | Skill Linux-reference, alábbi hordozhatósági lista |

A „check”, „build”, „teszt”, „GUI-futtatás”, „vizuális ellenőrzés” külön bizonyítéktípus. Egyiket ne nevezzük át másikká. A táblázatot minden megvalósítási fázis végén frissítsük.

## 3. Forráskód és állapotgazdák

| Útvonal | Felelősség |
| --- | --- |
| `crates/latexide-core/src/document.rs` | UTF-8 tartalom, UTF-16 pozíciók, kódolás/sorvég, tömör módosítási metaadat. |
| `crates/latexide-core/src/workspace.rs` | Projektgyökér, revízió, delta-alapú közös undo/redo, mentés és konfliktusvédelem. |
| `crates/latexide-core/src/syntax.rs`, `shared/ide-theme` | Közös, kölcsönzött forráson futó lexikális UTF-16 színezés; kanonikus paletták és szintaxis-szerepek. |
| `crates/latexide-core/src/{completion,tex_scan,project_index}.rs` | Közös parancskatalógus, lexikális projektmakró/címke/BibTeX index, gyorsítótár/keretek és UTF-16 jelöltek. Nem teljes TeX/LSP. |
| `crates/latexide-protocol` | Verziózott JSON-parancsok, keretek, sessionleíró és helyi IPC-kliens. |
| `crates/latexide-ffi` | C ABI, egyetlen közös runtime, hitelesített IPC-szerver, natív/build műveletek. |
| `crates/latexide-mcp` | Hivatalos rmcp stdio bridge; a meglévő sessionhöz kapcsolódik. |
| `apps/macos` | SwiftPM/AppKit–SwiftUI/PDFKit host; tesztelt natív adapter. |
| `apps/macos/Sources/LaTeXIDEKit/{Appearance,Chrome,Editor,Workspace,Panels}` | Natív fejlesztői felület; a `WorkspaceStore.editorStates` birtokolja a fájlonkénti kijelölést és görgetést. |
| `apps/macos/Sources/LaTeXIDEKit/Assistant` | Natív Codex stdio transport/session/panel. AssistantSession birtokolja az el nem küldött szöveget és csatolmányokat is. |
| `apps/windows` | C++/WinRT–WinUI forrás, natív restore/build/futtatás még unrun. |
| `crates/latexide-build` | Fordítóargumentumok, korlátozott környezet, folyamatfa, timeout/cancel, napló és PDF-generációk. |
| `tools/toolchain-lock.json` | Hivatalos Rust/Tectonic archive URL-ek és upstream SHA-256-ok. |
| `tools/texlive` | Aláírt TeX Live beszerzés, helyi profilok, gyökércsomagok és offline próba. |
| `evidence` | Valós naplók, beszerzési metaadatok, fordítási minták és ellenőrzési scope. |

Natív alkalmazásból a Rust könyvtár linkelődik. A külön MCP bridge ugyanahhoz a futó runtime-hoz kapcsolódik; nem tölthet be második, eltérő tartalmú dokumentummodellt.

## 4. Kötelezően megőrzendő adatszerződés

- A Rust dokumentummodell az egyetlen hiteles véglegesített állapot. A natív vezérlő szövegtárolója megjelenítési adapter.
- Pozíció: nulla alapú sor és UTF-16 kódegység-oszlop. A Rust belső tárolása UTF-8; emoji közepén levő pozíció érvénytelen. Ne keverjük byte offsettel, grapheme-indexszel vagy platformfüggő bekezdéspozícióval.
- Minden edithez `expected_revision` tartozik. Elavult revízió és átfedő/hibás batch nem módosíthat részlegesen.
- Az agent-módosítás azonnal alkalmazható; nincs `propose_edits`/előzetes review-kapu. Külön visszavonható tranzakció és eredetjelzés szükséges.
- A mentés külön `save_document`; az agent is hívhatja. Mentés nem törli az undo-történetet. Undo után az eredmény dirty, és újra menthető.
- A garantált undo az IDE/MCP műveleteire vonatkozik. Nyers külső fájlírás külső változás; ne állítsuk automatikusan, hogy ugyanazt a tranzakciós előzményt adja.
- UTF-8/BOM és LF/CRLF megőrzendő. Kevert vagy régi CR sorvég és hibás UTF-8 jelenleg explicit hiba, nem csendes konverzió.
- Kezdeti limit: fájlonként 1 MiB, 10 nyitott dokumentum; IPC frame 1 MiB; olvasás legfeljebb 64 KiB nyers UTF-8 darabokban.
- Az edit válasza tömör metaadat. Gépelésenként ne szerializáljuk a teljes forrást.
- FFI-puffert kizárólag a Rust felszabadító függvény engedhet el. Handle-lel folyamatban lévő hívás alatt ne történjen felszabadítás. Panic/exception nem léphet át a C ABI-n.

## 5. Fordító: jelenlegi döntés és bizonyíték

A kiadási alap **hivatalos TeX Live 2026 + latexmk/XeLaTeX**. A két cél bináris platformja `universal-darwin` (natív arm64 szelettel) és TeX Live `windows` (Intel/AMD x64). Az IDE saját Mac binárisa csak arm64 lesz; a külső compiler universal binárisának Intel szelete nem jelent Intel Mac-terméktámogatást.

Helyi Mac prefix: `.local/texlive-macos`, bin könyvtár `bin/universal-darwin`. Ennek latexmk-je symlink a `texmf-dist/scripts` könyvtárba. A program létezése canonicalize-zal ellenőrizhető, de a **bin alias útvonala megőrzendő**, hogy a gyermek-PATH előtagjában megtalálható legyen a vele együtt szállított `xelatex` és `kpsewhich`. Ezt a runtime-integráció valódi TeX Live próbája ellenőrzi.

A használható kiindulópont a [TeX Live útmutató](../tools/texlive/README.md), az [értékelés](compiler-assessment.md), a [runtime-manifest; helyi bizonyíték](development/LOCAL_EVIDENCE.md#retained-locally) és a [végső beszerzési ellenőrzés; helyi bizonyíték](development/LOCAL_EVIDENCE.md#retained-locally). A package roots és telepített revíziók rögzítve vannak; a teljes repository még nincs fagyasztva.

A Tectonic 0.17.0 továbbra is kutatási/regressziós baseline: az eredeti Mac/Windows csomag hash-ellenőrzött, és a Mac minta fordult. A teljes lockfile 14 advisory-érintett csomagverziót tartalmaz; a célzott gráfvizsgálat 11-et normál CLI-ágban, hármat feltételesen kizártan azonosított. Ez nem minden hiba runtime-kihasználhatóságának bizonyítása. Részletek az értékelésben, ne csak a „14 találat” számot vigyük tovább.

**Ami nincs igazolva:** bináris malware-scan, teljes natívfüggőség-audit, teljes repository-fagyasztás, Windows-futtatás és széles egyetemi sablonkorpusz. Az aláírás/hash eredethez viszonyított integritást bizonyít; nem abszolút fertőzésmentességet.

## 6. Reprodukálható helyi parancsok

Az alábbi shell-parancsok a projektgyökérből, macOS-en értendők. A `scripts/cargo-local.sh` csak a gyermekfolyamat környezetét állítja; nem változtat shellindítót vagy globális PATH-ot.

```sh
bash scripts/cargo-local.sh test --workspace --offline
bash scripts/cargo-local.sh build -p latexide-ffi --offline
python3 -W error::ResourceWarning -m unittest discover -s tests -p 'test_tool_acquisition.py'
python3 tests/ipc_contract.py
python3 tests/compiler_runtime_contract.py
bash scripts/cargo-local.sh check --workspace --target x86_64-pc-windows-msvc --offline
```

A helyi IPC-bindet a fejlesztői végrehajtási sandbox alapból tiltja (`EPERM`). Ezt külön minimális socketproba igazolta. Az IPC-integráció a szükséges, konkrét toolengedéllyel futtatható; a sandbox hibáját ne javítsuk a hitelesítés vagy a projektgyökér-védelem eltávolításával.

Rust/macOS fejlesztői csomagok újrabeszerzése: `python3 scripts/acquire_tools.py --install`. Ez jelenleg **Mac fejlesztői bootstrap**, nem kész Windows/Linux Rust-installer; Windows alatt nem futtatandó változtatás nélkül.

TeX Live beszerzés és próba pontos parancsai a `tools/texlive/README.md` fájlban vannak. A napi installer URL új tartalmát a rögzített hash szándékosan elutasítja; új verzió felvételéhez új aláírt metaadat és dokumentált lockfrissítés szükséges.

Mac alkalmazás buildje: `bash scripts/build_macos.sh`; szabályos indítása: `bash script/build_and_run.sh --verify`. A Run script futó példányt nem lő ki: előbb mentsünk/lépjünk ki. Az app `dist/LaTeXIDE.app`, a compiler/bridge és a SwiftPM palettabundle a saját Resources könyvtárába másolódik. A Rust **statikus** archívum explicit útvonallal linkelődik; tartalomhashből generált Swift-forrás kényszeríti a relinket Rust-only változáskor. A build teljes staged csomagot aláír/ellenőriz, a régit egyedi `LaTeXIDE.previous.*.app` néven megtartja, majd átnevezi az újat; nem írja felül egy futó processz mmapolt végrehajtható állományát. A helyi ad-hoc aláírás nem kiadási notarizáció.

Swift tesztek (a projektgyökérből):

```sh
CLANG_MODULE_CACHE_PATH="$PWD/.local/clang-module-cache" swift test --package-path apps/macos --scratch-path "$PWD/.local/swift-build" --cache-path "$PWD/.local/swift-cache" --config-path "$PWD/.local/swift-config" --security-path "$PWD/.local/swift-security" --disable-sandbox
python3 tests/mcp_contract.py
```

A Swift/Xcode szolgáltatás a végrehajtási sandboxban manifestírásnál EPERM-mel hibázott; a sikeres futások konkrét toolengedéllyel történtek. A deprecated `--build-system native` sem működött a sandboxban; a napi eljárás a default buildrendszert használja.

## 7. Eddigi érdemi Mac-es hibák és javítások

| Megfigyelés | Ok / javítás | Hordozhatósági tanulság |
| --- | --- | --- |
| A bundled Tectonic doctor cache-hozzáférés miatt hibázott. | A default user-cache a writable rootokon kívül volt. Projektbeli `TECTONIC_CACHE_DIR` mellett később valódi fordítás sikerült. | Hiányzó jogosultság nem sérült/malware-es compiler bizonyítéka. |
| A Rust upstream installer `Data/…` részt ismeretlen opciónak vette. | Szóközös prefix opciófeldolgozása. Az ellenőrzött Rust component payload közvetlen helyi másolása oldotta meg; regressziós teszt van. | Útvonalakat tömbargumentumokkal és szóközökkel teszteljük. |
| Latin Modern Roman fontnév nem volt OS-szinten elérhető. | A fixture fontfájlnevekre váltott; nincs globális fonttelepítés. | A TeX-fa fontjai és a rendszer fontregisztrációja külön dolgok. |
| Mentési cél symlinkkel másik belső fájlra irányítható volt. | Failing teszt után az eredeti canonical cél ellenőrzése bekerült. | Projektgyökéren belül sem írhatunk észrevétlenül másik fájlt. |
| Az aktív compiler végig `queued` állapotú volt. | Failing állapotteszt után `running` átmenet bekerült. | A státusz önálló szerződés; processlétezésből nem következik helyes UI-állapot. |
| A compiler örökölte a mesterséges credential-változót. | Külön subprocess próba után `env_clear` + szűk allowlist + hiteles compilerbin PATH készült. | API-kulcsokat és agent-envet ne adjunk a TeX gyermekfolyamatnak; valós titkot ne naplózzunk tesztként. |
| TeX Live fmtutil hibázott szóközös TMPDIR-rel. | Upstream tlmgr idézés nélküli status-file argumentuma; privát, szóköz nélküli `/private/tmp` temp mellett formátumgenerálás sikerült. | Prefix maradhat szóközös; tempet külön kell kezelni. HOME változatlan. |
| latexmk symlink canonicalize után nem találta a xelatex-et. | A valódi script könyvtára került PATH-ba a bin alias helyett. A konfiguráció most a bin alias-t őrzi meg. | Toolcsomagok indítási aliasa és canonical célja eltérő felelősség. |

Részletek, parancsok és aktuális tesztkimenetek az implementációs naplóban és `evidence` alatt. Ha egy javítás utáni új ellenőrzés még folyamatban van, azt így jelöljük, nem PASS-ként.

## 8. Windows és Linux folytatás

Windows: [skill-reference](../skills/latexide-platform-development/references/windows.md). A legfontosabb következő bizonyíték a tényleges MSVC/WinUI build és Windows 11 x64 futtatás, a közös editor/save/undo/compile/PDF folyamattal. A jelenlegi felhasználói környezetben ez nem elérhető.

Linux: [skill-reference](../skills/latexide-platform-development/references/linux.md). Konkrét jelenlegi akadály, hogy a `session.rs` minden `cfg(unix)` ágon `/private/tmp/latexide-{uid}` könyvtárat választ. Ez Mac-specifikus, nem megfelelő Linux runtime-directory kezelés. XDG/runtime tulajdonos- és jogosultságellenőrzést, Linux toolchaint, natív felületet és saját futtatási bizonyítékot kell hozzáadni. Ezek még nem elvégzett munkák.

## 9. Hogyan vezessük tovább?

Minden érdemi fázis után:

1. Frissítsük a jelenlegi állapottáblát és a napló megfelelő Task-sorát.
2. Jegyezzük fel a változtatást, az okot, érintett fájlokat, a tényleges ellenőrző parancsot és kimenetét, valamint a Windows/Linux következményt.
3. A reusable szerződés vagy platformfolyamat változását vezessük át a skill megfelelő reference-ébe. A teljes időrendi történetet a Markdown-napló őrizze, ne duplikáljuk minden skillbetöltéskor.
4. Ha nincs friss mérés vagy platformfuttatás, jelöljük `unrun/pending` állapotúnak. A hiányzó Windows-környezetet ne rejtsük el újabb Mac-ellenőrzéssel.

Az aktuális továbblépés a megvalósítási terv következő nyitott Task-ja. A végső átadás előtt minden itt linkelt fájlt, parancsot és állapotot a tényleges munkafával egyeztetni kell.

## 10. Natív és agent-alap legutóbbi fejlesztési szerződései

- A fájllista `FileEntry { path, size }` objektumokat ad, nem stringtömböt. Valódi Mac projektmegnyitás előhozta a hibás cast összeomlását; teszttel reprodukálva javult.
- A Swift NSRange-konverzió önmagában nem elég a surrogate-pár határának validálásához; a `SourceEdit` külön ellenőrzi.
- `begin_composition` / `end_composition`: csak a natív oldal hívhatja. Aktív kompozíció alatt módosítás/mentés/undo/build explicit `composition_active` hibát kap; az end egyetlen undo-műveletként véglegesít. Ezt a Windows adapternek is követnie kell.
- A natív host eltérhet a Rust/Python SIGPIPE-beállításától. Sessionleállításnál connect ébreszt, nincs versenyző wake-write; macOS write oldalon per-socket SO_NOSIGPIPE védi a hostot.
- A Mac prototype 250 ms-os tömör metaadat-pollinggal követi a külső agentet. Teljes dokumentumot csak megváltozott revíziónál olvas, a normál keystroke UTF-16 range-et küld. Windows első adaptere UTF-16 diffet számol; RichEdit tényleges viselkedése még ellenőrizetlen.
- Ismert fennmaradó feladatok és a korábbi GUI-próba: `docs/verification.md`; az új felület: `docs/ui-verification.md`. Natív fájlon belüli Find és lexikális színezés már van. Projekt-tartalomkeresés, teljes külső fájlmanifest/watcher, crash recovery és kiadási teljesítmény-/hozzáférhetőségmérés még nincs.

## 11. Független review és végső javítások

Az alap/UI és az intelligenciafázis review-jai történetiek. Az új projektkiegészítés külön egyszeri friss review-t kapott; három megállapítás, valamint a fix passban feltárt pontos-kulcs Tab-hiba reprodukálva javult. [Review és döntések](project-completion-review.md). A projektkiegészítés-fázis eredménye: Rust 56; natív Mac 53 aktív (57 futtatottból 4 opt-in kihagyva), új projektpopup-render 1; IPC 9, MCP 1, compiler-runtime 3; MSVC x64 type-check. Windows/Linux natív futtatás továbbra sincs.

Az ezt követő Enter-bejelentésnél a beragadás az akkori kódban és futó alkalmazásban nem volt reprodukálható; a felhasználó is működőnek látta a vizsgálat közben. Új termékkódjavítás helyett 5 valódi billentyűeseményes karakterizációs teszt került be: a popup tényleges eltűnése, késleltetett frissítések, forrásfókusz, második Enter és undo. Az Enter-fázis teljes natív eredménye 62 futtatott / 58 PASS / 4 opt-in skip / 0 hiba. [Pontos ellenőrzés](completion-enter-verification.md).

A definíciós inkrementum egy új, csak olvasó integrált review-t kapott: 2 Important és 1 Minor, utóbbit a szerző a dirty/összefésülés alatti BibTeX-következetesség miatt Importantnak minősítette. Mindhárom RED→GREEN javult egy körben; nincs újra-review vagy maradó minor. Végső eredmény: Rust 69 PASS, natív 73 futtatott / 69 PASS / 4 opt-in skip / 0 hiba, IPC10/MCP1 és MSVC x64 check PASS. [Review](definition-navigation-review.md), [ellenőrzés](definition-navigation-verification.md).

Projektváltáskor és kilépéskor a natív mentés/elvetés/mégse átmenet megvédi a dirty runtime-ot; a programozott megnyitás alapból nem dobhatja el. Az `end_composition` a hibás commit esetén is lezárja a kompozíciót, a vezérlő pedig visszaáll a magban maradt szövegre. A hibát jelezni kell, nem szabad kompozíciósnak látszó, zárolt szerkesztőt hagyni.

A Mac installer profil a tényleges checkoutból generálódik. A Windows közös prefix `.local/texlive-windows-x64`. A több pontot tartalmazó főfájlnév teljes stemje megmarad a PDF-névben. XeLaTeX `file:line:` hibák is diagnosztikává alakulnak.

A helyi Codex futtatási akció `.codex/environments/environment.toml` alatt a projekt saját `./script/build_and_run.sh` parancsára mutat. Nem készít fiókot, nem publikál és nem telepít rendszerszinten.

## 12. Végső csomag és ellenőrzési határ

A végső `dist/LaTeXIDE.app` újraépült, arm64 binárissal és érvényes helyi ad-hoc aláírással. A compiler/bridge/paletták a csomagban vannak; a docs nélküli TeX-fa törött man aliasa csak a generált bundle-ből marad ki. Ez nem notarizáció vagy malware-igazolás. Aktuális manifest: `evidence/definition-navigation-2026-10-31/macos-app-manifest.json`; a korábbi manifestek történeti pillanatképek.

A régebbi mintákban lévő felhasználói szöveg megmaradt. A projektkiegészítés előtt a futó intelligence-minta már user-eredetű r25/mentett állapotban volt; az eredeti mac-gui-project mintával együtt hash-rögzítve, normál kilépéssel megőriztük. Az új külön saját mintában tényleges makró/ref/cite beszúrás, egy undo/redo, másik fájl nem mentett agentcímkéje és annak visszavonása igazolt. Végül minden megtartott dokumentum tiszta, a11889-byte PDF friss és PDFKit AX-ben látható, a korábbi hash-ek változatlanok. A live screenshot torzult, ezért a kontrollált render és AX/input bizonyíték külön marad. Részletek: `docs/project-completion-verification.md`, `evidence/project-completion-2026-10-31/live-*.json`.

A korábbi MCP-mentés utáni elavult dirty-jelzés javult: a metaadatot a szövegrevíziótól függetlenül frissítjük minden megtartott dokumentumhoz.

Enter-follow-up: az alkalmazáskód és a csomag változatlan, a négy artifact hash-e és a strict aláírás újra ellenőrzött (`evidence/completion-enter-2026-10-31/bundle-check.json`). A felhasználó közben a nyitott minta `main.tex` bufferét szerkesztette; az r6/dirty tartalom megmaradt, a két saját tesztbuffer külön revízióellenőrzött visszaállítást kapott. A futó appot nem indítottuk újra, és a korábbi „minden dokumentum tiszta” pillanatkép nem a mostani felhasználói munkamenet állapota.

## 13. Fejlesztői UI: új közös szerződések

- Négy kanonikus JSON-paletta, összesen nyolc változat. A rendszerkövetés vagy fix világos/sötét mód külön tartós preferencia. A Mac a saját SwiftPM-bundle-jét, a Windows az exe mellé másolt azonos JSON-t olvassa; nincs checkout-relative product fallback.
- `get_syntax` revízióhoz kötött, legfeljebb32768 UTF-16 egységnyi ablakot kér. A rendezett `[start,length,kind]` abszolút, scalar-safe tokenek a kért ablakon túlnyúlhatnak, ha metszenek vele. A prefix kontextus a1MiB forráslimittel korlátos; nem másolunk teljes snapshotot színezésenként. Lexikális kiemelés, nem teljes TeX-értelmező.
- Csak attribútumok változnak, a forrás/revízió/undo nem. Kompozíció alatt nincs recolor. Normál gépelés/görgetés késleltetett, első megjelenés és téma azonnali. A kijelölés, görgetés és gépelési formátum megőrzendő.
- A fájlonkénti kijelölés és viewport a WorkspaceStore-ban marad, így az utolsó fül elrejtése/újranyitása sem veszíti el. A vizuális elrejtés nem core-close: továbbra is foglalja a tíz megtartott dokumentum keretét, és megőrzi a nem mentett tartalmat/undo-t.
- A projektfa és Quick Open fájlnevet szűr; a natív Find a fájl tartalmában keres. Az MCP-állapot elérhetőséget mutat, nem kitalált csatlakozott kliensszámot. A kimenet valódi compiler log/diagnosztika.
- Windows: az új projekt saját fájljából választ fődokumentumot; RichEdit pontos teljes/range roundtrip ellenőrzése, plain-text clipboard, AltGr/IME és aszinkron race-védelem forrásban szerepel. Tényleges target-ellenőrzések és részleges paritás: `docs/windows-ui-port.md`.
- Folytatási skill-reference: `skills/latexide-platform-development/references/ui.md`. Linuxnak ugyanezeket a szerződéseket kell natív adapterrel átvennie; megvalósított Linux UI-t nem állítunk.

## 14. Billentyűzet, kiegészítés és asszisztens — legújabb szerződések

- Magyar kiosztáson a CUA `super+z` logikai y-t adott; `super+y` logikai z-t. Ez mért eszköz/kiosztás-viselkedés, nem az IDE Cmd+Y-átkötése. SourceTextView csak saját fókusznál kezel shortcutot; a menü a fókuszált natív mező saját UndoManagerét vagy a Rust dokumentumtörténetet hívja.
- `get_completions {document_id,expected_revision,caret_utf16}`: abszolút scalar-safe UTF-16 cseretartomány, max 20 jelölt; az argumentumkijelölés a beszúrt szövegen belüli. Elfogadás előtt dokumentum/revízió/kurzor/szöveg ellenőrzés; elavult Enter/Tab normál szerkesztéssé válik. Popupbezáráskor a hosting hierarchia is felszabadul.
- A PDF/Asszisztens egy jobb oldali inspector két módja. Panelméret/mód és kiegészítés tartós; a draft/csatolmány a munkaterület életében marad meg, nem lemezes beszélgetéstörténet. Projektváltás megszakítja a régi agentet.
- A Codex inicializálás nem modellhívás. A saját modell/provider marad; a valódi modellkör hálózatot és előfizetést/API-keretet használhat. `auto` nem feltétlen automatikus engedély: csak a meglévő kilenc IDE-eszköz szerepel az `enabled_tools` listán és kap `approve` módot, ismeretlen eszköz nem.
- A Mac stdio/fókusz/popup implementációt Windows/Linux alatt saját natív processz- és szerkesztő API-kra kell portolni. A részletes reusable referencia: [intelligence.md](../skills/latexide-platform-development/references/intelligence.md).
- Következő termékmunka: szemantikus projektgráf/dinamikus makrók és csomagadatok, outline/navigáció, SyncTeX, projektkeresés/Git, fájléletciklus/helyreállítás és a dokumentumlimit kezelése. A lexikális saját makró-, címke- és BibTeX-kiegészítés már elkészült; ez nem jelenti a teljes szemantikus IDE elkészültét.


## 15. Projektismerő kiegészítés — hordozható szerződés

- A Rust projektindex nem nyit editorfájlokat. A nyitott, hiteles buffer felülírja ugyanazon fájl lemezképét; az agent edit/undo azonnal számít. Sorrend: aktív fájl, további nyitott fájlok rendezve, maradék projektfájlok rendezve; fájlon belül utolsó deklaráció.
- Keretek:128fájl/8MiBösszes/1MiBfájlonként/8192szimbólum/10000bejárt elem/20jelölt. A részleges index a popupban és a státuszsoron is látszik, nulla találatnál is. Symlink/rejtett/artifact útvonal nem indexelődik.
- Gépelés alatt legfeljebb1s-os disk cache; elfogadás előtt `refresh_project:true` újraolvassa a nem nyitott forrásokat méret/mtime egyezésekor is. A natív oldal a kiválasztott tartományt és teljes beszúrási/kijelölési jelentést ellenőrzi; másik fájl revíziója is számít.
- Kulcslistában csak az aktuális elem cserélődik. A vessző utáni szóközös üres elem működik; korábbi elemnél lezáratlan listát nem zárunk le a többi kulcs előtt. Pontos, már kész kulcs elfogadása nem ír tabot és nem hoz létre undo-t.
- A lookahead szorzódását is korlátozzuk: rövid környezetnév/makró/aritás olvasás és közös signature-budget. A közel1MiB-os hibás TeX-próba másodperces blokkolását teszt bizonyította és megszüntette. A core-only dev opt-level2 megtartja a debug jelképeket/assertionöket; nem release-mérés.
- Részletes wire/parser/cache/port szerződés: [projektkiegészítési skill-reference](../skills/latexide-platform-development/references/project-completion.md). A natív Windows/Linux popup/elfogadás/IME/státusz továbbra portfeladat.
- Az új mintafordítás végső PDF-je feloldotta a hivatkozást, de a meglévő logalapú diagnosztika megtartott három első-passzos rerun warningot. A végső compiler-passhoz igazított diagnosztika további minőségi feladat.

## 16. Teljes IDE-felmérés és implementációs hiánylista — 2026-10-31

A felhasználó teljes JetBrains/VS Code összevetést és módosítási tervet kért. Az eredmény: [részletes felmérés](IDE_AUDIT_2026-10-31.md), [226 tételes backlog](ide-audit-2026-10-31/IMPLEMENTATION_BACKLOG.md), CSV/JSON és 50 képességcsaládos mátrix, 51 elsődleges webes forrással. A 206 LaTeX/research/dev-workspace tétel és a 20 univerzális szakterületi vertikum tervezett munka; a felmérés nem implementálta őket és nem módosítja önmagában az elfogadott termékscope-ot.

- Friss saját auditellenőrzés: Rust56; natív59executed/55PASS/4opt-in skip/0failure; C ABI harness, IPC9, MCP1, valódi compiler3, Python6 PASS; 5-crate MSVC check, Windows5XML/42handler és két hordozható C++ harness PASS. A később lezáródó Enter-karakterizáció külön bizonyítéka62executed/58PASS/4skip; ezek nem összeadandó suite-eredmények.
- Hét saját, ideiglenes projekten reprodukált termékhatár: olvasás fogyasztja a tíz dokumentumhelyet; 300 külön editből256undo; runtime recovery hiánya; külső disk-változás mellett régi buffer/clean státusz, helyes save-konfliktussal; öt nem-PDF byte sikeres artifactként; meg nem nyitott buildfüggőség változása mellett stale=false; hiányos első IPC frame blokkolja a második klienst.
- Első módosítási irány: recovery/bufferéletciklus, közös VFS, többfájlos tranzakció és typed/async runtime, teljes buildinput-manifest és PDF-validáció; ezek fölött outline/search/definition/rename/SyncTeX/Git. A közös Rust-authority, UTF-16/revízió és közvetlen agentedit/explicit-save/undo szerződés megőrzendő.
- Auditbizonyíték: `evidence/ide-audit-2026-10-31`. Az első sandboxos Swift-futás IPC EPERM miatt sikertelen; engedélyezett helyi IPC-vel PASS. Meglévő `.app` signature read-only ellenőrzött; új csomagolás, live GUI/model kör, release-performancia, Windows/Linux natív futás ebben az auditban unrun.
- Új hordozható folytatási útmutató: [audit skill-reference](../skills/latexide-platform-development/references/audit.md). A dátumozott felmérés nem helyettesíti az aktuális aktív feladatot és a későbbi platformbizonyítékokat.
- Az audit saját tesztköre után megjelent az aktív Cmd+click/get_definition fejlesztés Rust/protokoll/Mac forrása. A LANG-06/C10 ezért részleges, új forrásképesség, nem teljesen hiányzó. Az audit korábbi green számai ezt az új inkrementumot nem minősítik; `snapshot-delta.json` és az olvasott forrásmásolat külön őrzi a határt.
