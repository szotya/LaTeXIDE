# P07 — Natív szerkesztői termelékenység Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Billentyűzettel és IME-vel megbízható, testreszabható írási/szerkesztési workflow-kat épít.

**Architecture:** A meglevő SourceTextView vékony adapter marad; külön feature-modul kezeli a kurzorokat, snippetállapotot, foldot és diagnosztikai nézetet. A szöveget továbbra Rust tranzakció módosítja.

**Tech Stack:** Rust 1.99.1 / edition 2024, serde; célplatformi natív adapter és meglévő tesztkeret. Új függőség csak rögzített lockfile-lal és indokolt licenc-/eredetellenőrzéssel.

**Spec:** [közös csomagszerződés](../../specs/2026-10-31-parallel-development-design.md), [audit](../../../IDE_AUDIT_2026-10-31.md), [aktuális állapotváltozások](../../../development/BASELINE_DELTA.md).

## Global Constraints

- Egyetlen hiteles Rust dokumentummodell; a natív adapter és az MCP ugyanazt a sessiont használja.
- Pozíció: nulla alapú sor/UTF-16-oszlop; minden módosítás expected_revision mellett atomi. UTF-8/BOM és LF/CRLF megőrzendő.
- Agent edit közvetlen; nincs szerkesztésenkénti jóváhagyási kapu. Save explicit; save után is működő undo/redo szükséges.
- Mac: arm64, macOS 14+; Windows: Windows 11 x64 Intel/AMD; Linux saját natív igazolásig planned.
- A kezdeti 1 MiB/fájl, 10 buffer, 1 MiB IPC és 64 KiB read limit csak külön verziózott/tesztelt döntéssel változhat.
- A csomag jelenleg planned/unrun. Meglévő teszt vagy ez a terv nem bizonyítja a teljes csomag megvalósítását.
- Csak a saját alábbi útvonalak írhatók párhuzamosan. Meglévő közös fájlhoz integration-manifest és P00-bekötés kell; P00 külön felsorolt integrátori útvonalai kivételek.

## Review Focus

- R1: Marked text mellett snippet/formatter ne sértse meg az IME-kompozíciót.
- R2: Több kurzor átfedő tartományai determinisztikus hibát kapjanak.
- R3: Popup/fold/diagnosztika ne vegye el a forrás billentyűzetfókuszát.
- R4: Soft wrap után a sor-/UTF-16-mapping és scroll-visszaállítás maradjon pontos.
- R5: Nagy fájlban minimap/fold kikapcsolható és mért kereten belül maradjon.

---

## Tulajdon és indulás

**Branch:** `feature/p07-editor`. **Profil:** core.

**Kizárólagosan létrehozható/módosítható útvonalak:**

- `apps/macos/Sources/LaTeXIDEKit/Features/Editor/`
- `apps/macos/Tests/LaTeXIDEKitTests/EditorFeatureTests.swift`
- `integration/requests/P07.json`
- `docs/development/results/P07.md`

**Teszt:** `apps/macos/Tests/LaTeXIDEKitTests/EditorFeatureTests.swift`. **Eredménynapló:** `docs/development/results/P07.md`.

**Olvasandó jelenlegi belépési pontok:** `apps/macos/Sources/LaTeXIDEKit/Editor/SourceTextView.swift`, `apps/macos/Sources/LaTeXIDEKit/NativeEditor.swift`, `docs/IDE_PRODUCT_DIRECTION.md`.

Indulás: P00 contract-v1 után, fagyasztott fixture-ökkel és fake portokkal; a többi csomag teljes elkészülte nem indulási feltétel.

Integrációs szomszédok: P02, P05, P08, P09, P15, P23. Ezek **nem csomagszintű topologikus sorrend**: az egyedi követelményfüggőségek maradnak irányadók. A kölcsönös portfüggést fixture és fokozatos bekötés oldja, nem két teljes csomag egymásra várása.

## Task 1: Önállóan ellenőrizhető szerződésszelet

**Create:** `apps/macos/Sources/LaTeXIDEKit/Features/Editor/EditorFeatureState.swift`, `apps/macos/Tests/LaTeXIDEKitTests/EditorFeatureTests.swift`. **Consumes / produces:**

```rust
pub fn plan_editor_action(snapshot:&WorkspaceSnapshot, action:EditorAction) -> Result<EditorTransaction,EditorError>;
// EditorAction { document_id:String, revision:u64, selections:Vec<Utf16Range>, command:String }; EditorTransaction { edits:Vec<RevisionedEdit>, selections:Vec<Utf16Range> }; EditorError::{CompositionActive,InvalidSelection,StaleRevision,UnsupportedCommand}
```

Az itt felsorolt DTO-mezőket P00 helyezi a `crates/latexide-contracts/src/` moduljaiba, serde serialize/deserialize és egyértelmű hiba-enummal. A szolgáltatás a saját modulban marad; ne hozz létre protocol→core→protocol függőségi kört. A port v1 helyi Rust-határ, nem automatikus új IPC-eszköz.

**Saját adapter belépési pontja:**

```swift
func planEditorAction(snapshot: WorkspaceSnapshot, action: EditorAction) throws -> EditorTransaction
```

A fenti Rust-alak az interoperábilis szerződést rögzíti; a saját adapter az itt megadott nyelven készül. Az első assertiont az adapter saját tesztkerete ellenőrzi.

- [ ] **RED:** `apps/macos/Tests/LaTeXIDEKitTests/EditorFeatureTests.swift` alatt `contract_v1_first_slice` néven rögzítsd ezt az ellenőrzést: **Többkurzoros beszúrás egy undo-val visszaáll; aktív IME alatt ugyanaz CompositionActive, változatlan tartalommal.**
- [ ] Futtasd: `swift test --package-path apps/macos --filter EditorFeatureTests`. Először a hiányzó szolgáltatásra vagy állított viselkedésre FAIL legyen; környezet-/linkhibát ne könyvelj reprodukciónak.
- [ ] Valósítsd meg az első portot `apps/macos/Sources/LaTeXIDEKit/Features/Editor/EditorFeatureState.swift` alatt; az alkalmazás-regisztrációt add át `integration/requests/P07.json` manifestben (schema, package_id, contract_version, requirement_ids, source_paths, hook_targets, test_commands, compatibility).
- [ ] Ugyanazzal a paranccsal legyen GREEN; tedd mellé R1–R5 célzott regresszióját a `review_focus_r1` … `review_focus_r5` tesztekbe. Az állítások a fenti öt elvárt viselkedést vizsgálják.
- [ ] Ellenőrizd a saját diffet, majd készíts kis commitot: `git add -- apps/macos/Sources/LaTeXIDEKit/Features/Editor/EditorFeatureState.swift apps/macos/Tests/LaTeXIDEKitTests/EditorFeatureTests.swift integration/requests/P07.json` és `git commit -m "feat(p07): establish tested service slice"`. A dokumentált későbbi munkavégzéskor commitolj; ez a terv most nem futtatja ezeket.

## Követelményenkénti feladatsor

Minden alábbi sor külön tesztelhető részeredmény és saját kis commit. A teszt neve az ID kisbetűs, aláhúzásos alakja; az elfogadás mondata a kötelező assertion. A dependency provider mockja csak izolált teszthez elég; release-hez a valódi providerrel is teljesüljön.

### EDT-01 — Több kurzor és box selection (P2, M)

**Kiindulás:** A szállított adapter egy NSRange-et és egy pending editet kezel.

**Módosítás:** Több kijelölésből egy validált batch; scalar/IME/undo szerződés megőrzése.

**Valódi bekötés előtt:** DOC-08 (P02).

- [ ] `edt_01` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Tíz kurzor Unicode-szövegen egy tranzakcióban szerkeszt; átfedés elutasítható.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `swift test --package-path apps/macos --filter EditorFeatureTests` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P07` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `EDT-01` implemented/tested állapotúnak.

### EDT-02 — Soft wrap és prózaszélesség (P1, M)

**Kiindulás:** A Mac szövegkonténer szélessége végtelen, vízszintes görgetéssel.

**Módosítás:** Fájlonként kapcsolható soft wrap és olvasási szélesség; vizuális sorok guttertérképe.

**Valódi bekötés előtt:** nincs.

- [ ] `edt_02` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Hosszú bekezdés ablakméretváltáskor olvasható; caret/SyncTeX valódi forrássort használ.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `swift test --package-path apps/macos --filter EditorFeatureTests` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P07` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `EDT-02` implemented/tested állapotúnak.

### EDT-03 — Behúzás és páros zárójelek (P1, M)

**Kiindulás:** Natív szövegbevitel van; nincs LaTeX-tudatos indent/pair motor.

**Módosítás:** Közös indent/comment/pair szabályok, beállítható tab/spaces és EditorConfig-adapter.

**Valódi bekötés előtt:** LANG-01 (P05).

- [ ] `edt_03` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Környezet, math, verbatim és paste helyesen viselkedik; egy undo visszaállít.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `swift test --package-path apps/macos --filter EditorFeatureTests` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P07` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `EDT-03` implemented/tested állapotúnak.

### EDT-04 — Többhelyes snippet és Tab-kilépés (P1, M)

**Kiindulás:** A completion első argumentumhoz állít kurzort.

**Módosítás:** Snippet-session több tabstoppal, tükrözött mezőkkel és megszakítási szabályokkal.

**Valódi bekötés előtt:** EDT-03 (P07).

- [ ] `edt_04` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **frac mindkét argumentuma bejárható; stale/IME/fókuszváltás nem nyeli el a Tabot.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `swift test --package-path apps/macos --filter EditorFeatureTests` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P07` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `EDT-04` implemented/tested állapotúnak.

### EDT-05 — Kódösszecsukás (P1, M)

**Kiindulás:** Lexer van; nincs folding állapot vagy tartományszolgáltatás.

**Módosítás:** Section/environment/comment/import fold range és dokumentumonkénti állapot.

**Valódi bekötés előtt:** LANG-01 (P05).

- [ ] `edt_05` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Összecsukott részen agentedit után helyes tartományok és visszaállítható caret marad.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `swift test --package-path apps/macos --filter EditorFeatureTests` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P07` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `EDT-05` implemented/tested állapotúnak.

### EDT-06 — Sor- és kijelölésműveletek (P2, S)

**Kiindulás:** Nincsenek közös move/duplicate/join/comment szerkesztőparancsok.

**Módosítás:** Command registryre kötött sor-/blokkmozgatás, duplikálás, komment és surround.

**Valódi bekötés előtt:** WS-06 (P08).

- [ ] `edt_06` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Billentyűzet és menü azonos eredményt ad; egy tranzakció és Unicode-helyes kijelölés.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `swift test --package-path apps/macos --filter EditorFeatureTests` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P07` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `EDT-06` implemented/tested állapotúnak.

### EDT-07 — Formázóintegráció (P1, M)

**Kiindulás:** Nincs LaTeX/BibTeX formatter API.

**Módosítás:** Korlátos külső formatter vagy LSP adapter; diff alapú WorkspaceEdit a Rust magon át.

**Valódi bekötés előtt:** DOC-08 (P02), EXT-01 (P15).

- [ ] `edt_07` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Kijelölt blokk és teljes fájl formázása megőrzi revízióvédelmet és egy undo-t ad.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `swift test --package-path apps/macos --filter EditorFeatureTests` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P07` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `EDT-07` implemented/tested állapotúnak.

### EDT-08 — Helyesírás és nyelvhelyesség (P1, M)

**Kiindulás:** A Mac adapter kifejezetten kikapcsolja a spell/grammar ellenőrzést.

**Módosítás:** Prózaszakaszokon nyelvválasztás, személyes/projektszótár és opcionális grammar provider.

**Valódi bekötés előtt:** LANG-01 (P05).

- [ ] `edt_08` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Magyar/angol próza vizsgálható; parancs, formula, citation és verbatim nem zajos.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `swift test --package-path apps/macos --filter EditorFeatureTests` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P07` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `EDT-08` implemented/tested állapotúnak.

### EDT-09 — Fájlon belüli fejlett keresés/csere (P2, M)

**Kiindulás:** Mac natív Find van; Windows egyszerű előre/hátra keresés forrásban.

**Módosítás:** Közös regex/case/whole-word/selection/replace szerződés natív adapterekkel.

**Valódi bekötés előtt:** DOC-08 (P02).

- [ ] `edt_09` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Zero-length regex, Unicode és Replace All egy undo; nincs félbevágott surrogate.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `swift test --package-path apps/macos --filter EditorFeatureTests` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P07` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `EDT-09` implemented/tested állapotúnak.

### EDT-10 — Szerkesztőbeli diagnosztikai jelölők (P1, M)

**Kiindulás:** Fordítási problémalista van; nincs revíziózott inline jelölés.

**Módosítás:** Range-ek, gutter/scrollbar jelölők, hover és következő/előző probléma parancs.

**Valódi bekötés előtt:** BLD-05 (P09), LANG-05 (P05).

- [ ] `edt_10` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Régi build hibája nem jelenik meg új szöveg hibás koordinátáján.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `swift test --package-path apps/macos --filter EditorFeatureTests` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P07` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `EDT-10` implemented/tested állapotúnak.

### EDT-11 — Betű- és sorközbeállítások (P2, M)

**Kiindulás:** Rögzített monospaced font, betűméret és négy paletta van.

**Módosítás:** Betűcsalád, line-height, tab width és whitespace kapcsolók tartós beállítása.

**Valódi bekötés előtt:** WS-04 (P08).

- [ ] `edt_11` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Téma/fontváltás nem szerkeszt szöveget; minden UI-szöveg olvasható nagyításkor.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `swift test --package-path apps/macos --filter EditorFeatureTests` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P07` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `EDT-11` implemented/tested állapotúnak.

### EDT-12 — Delimiter és környezet navigáció (P2, M)

**Kiindulás:** Lexikális színezés megkülönböztet szerkezeteket.

**Módosítás:** Matching pair, selection expansion és begin/end közti ugrás.

**Valódi bekötés előtt:** LANG-01 (P05).

- [ ] `edt_12` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Verbatim/comment és escaped delimiter nem hamis pár; hibás dokumentum is kezelhető.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `swift test --package-path apps/macos --filter EditorFeatureTests` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P07` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `EDT-12` implemented/tested állapotúnak.

### EDT-14 — Kurzor pozíció és szerkesztési hely története (P2, S)

**Kiindulás:** Státuszsor mutat sort/oszlopot; általános Go to Line nincs.

**Módosítás:** Sor/oszlopra ugrás, utolsó szerkesztés és prev/next caret hely parancs.

**Valódi bekötés előtt:** WS-06 (P08).

- [ ] `edt_14` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Túl nagy sor/oszlop biztonságosan korlátozott; fókusz és viewport következetes.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `swift test --package-path apps/macos --filter EditorFeatureTests` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P07` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `EDT-14` implemented/tested állapotúnak.

### EDT-15 — IME és billentyűkészletek (P1, M)

**Kiindulás:** Kompozíciós/Unicode/logikai-key szerződések teszteltek szűk körben.

**Módosítás:** HU/US/AltGr/dead-key/CJK, dictation és natív mezőfókusz kvalifikáció.

**Valódi bekötés előtt:** QA-06 (P23).

- [ ] `edt_15` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Minden támogatott célon commit egyszeri; prompt/Find/source undo egymástól független.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `swift test --package-path apps/macos --filter EditorFeatureTests` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P07` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `EDT-15` implemented/tested állapotúnak.

### EDT-16 — Minimap, sticky scroll és inlay kapcsolók (P3, M)

**Kiindulás:** Nincs ilyen kiegészítő nézet.

**Módosítás:** Szemantikus navigáció után opcionális, kikapcsolható áttekintő és inlay réteg.

**Valódi bekötés előtt:** LANG-01 (P05), QA-03 (P23).

- [ ] `edt_16` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Nagy dokumentumnál is mért költség; képernyőolvasóval kihagyható, source nem változik.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `swift test --package-path apps/macos --filter EditorFeatureTests` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P07` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `EDT-16` implemented/tested állapotúnak.

## Task Z: Bekötés és átadás

- [ ] `python3 scripts/validate_development_packages.py` és `swift test --package-path apps/macos --filter EditorFeatureTests` sikeres; a tesztparancs itt célállapot, futtatását külön rögzíteni kell.
- [ ] P00 alkalmazza a manifestben felsorolt közös hookokat; stagingben egyesével fusson a korábbi Rust/native/IPC/MCP regresszió a megváltozott rétegnek megfelelően. Nincs automatikus Save vagy új agent-edit jóváhagyás.
- [ ] Mac natív workflow valódi fókusz-/IME-/undo-eseménnyel; Windows külön MSBuild/link/launch/funkcionális kör. Linux saját runner/felület nélkül unrun. Nem natív csomagnál ezeket az adaptergazda végzi.
- [ ] `docs/development/results/P07.md` tartalmazza a teljesített ID-ket és reprodukálható bizonyítékot. P00 ennek alapján frissíti a handoffot, ledgert és a releváns skill-reference-et; más csomag ne írja párhuzamosan ezeket.

## Másolható indítófeladat

> Hajtsd végre a `docs/superpowers/plans/2026-10-31-parallel/p07-editor.md` tervet. Előbb olvasd az AGENTS.md-t, a handoffot, a projekt-skillt, a közös csomagszerződést és a BASELINE_DELTA-t. Csak P07 saját útvonalait módosítsd, a közös fájlokhoz integration-manifestet adj. Őrizd meg a közvetlen agent-edit + explicit Save + Save után is elérhető undo szerződést. A követelményfüggőséget és platformi bizonyítékot ellenőrizd; planned/unrun állapotot ne nevezd késznek.
