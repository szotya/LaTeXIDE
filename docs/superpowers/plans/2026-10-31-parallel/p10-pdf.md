# P10 — PDF munkafolyamat és SyncTeX Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Revízióhelyes kétirányú forrás–PDF navigációt és teljes natív PDF-munkafolyamatot ad.

**Architecture:** A PDF, SyncTeX és BuildManifest egy generation-lease alatt él. Közös koordinátaszerződés fölött Mac PDFKit és külön Windows adapter; nincs rossz generationre való csendes navigáció.

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

- R1: Új buildre cserélt PDF mellett régi SyncTeX-t ne használjon az adapter.
- R2: Forgatott/eltérő méretű oldal koordinátája mindkét irányban pontos legyen.
- R3: Hibás vagy több ezer oldalas PDF ne blokkolja korlátlanul a felületet.
- R4: Preview-csere megőrizze az érvényes zoomot/viewportot és ne dobjon fókuszt.
- R5: Windows szövegkeresés/kijelölés saját futtatási bizonyíték nélkül ne kapjon Mac eredményt.

---

## Tulajdon és indulás

**Branch:** `feature/p10-pdf`. **Profil:** core.

**Kizárólagosan létrehozható/módosítható útvonalak:**

- `crates/latexide-core/src/services/pdf/`
- `crates/latexide-core/tests/pdf_service_contract.rs`
- `integration/requests/P10.json`
- `docs/development/results/P10.md`

**Teszt:** `crates/latexide-core/tests/pdf_service_contract.rs`. **Eredménynapló:** `docs/development/results/P10.md`.

**Olvasandó jelenlegi belépési pontok:** `apps/macos/Sources/LaTeXIDEKit/Panels/PDFPanel.swift`, `crates/latexide-build/src/lib.rs`.

Indulás: P00 contract-v1 után, fagyasztott fixture-ökkel és fake portokkal; a többi csomag teljes elkészülte nem indulási feltétel.

Integrációs szomszédok: P05, P06, P09, P21, P23. Ezek **nem csomagszintű topologikus sorrend**: az egyedi követelményfüggőségek maradnak irányadók. A kölcsönös portfüggést fixture és fokozatos bekötés oldja, nem két teljes csomag egymásra várása.

## Task 1: Önállóan ellenőrizhető szerződésszelet

**Create:** `crates/latexide-core/src/services/pdf/mod.rs`, `crates/latexide-core/tests/pdf_service_contract.rs`. **Consumes / produces:**

```rust
pub fn forward_sync(bundle:&PdfBundle, source:SourceAnchor) -> Result<PdfLocation,PdfError>;
// PdfBundle { generation:String, snapshot_id:String, pdf_path:String, synctex_path:String }; PdfLocation { page:u32, x:f64, y:f64, width:f64, height:f64, generation:String }; PdfError::{StaleGeneration,MissingSync,InvalidPage,InvalidPdf}
```

Az itt felsorolt DTO-mezőket P00 helyezi a `crates/latexide-contracts/src/` moduljaiba, serde serialize/deserialize és egyértelmű hiba-enummal. A szolgáltatás a saját modulban marad; ne hozz létre protocol→core→protocol függőségi kört. A port v1 helyi Rust-határ, nem automatikus új IPC-eszköz.

- [ ] **RED:** `crates/latexide-core/tests/pdf_service_contract.rs` alatt `contract_v1_first_slice` néven rögzítsd ezt az ellenőrzést: **Régi generationhöz tartozó SyncTeX cél StaleGeneration; helyes bundle pontos oldalt és forgatott koordinátát ad.**
- [ ] Futtasd: `cargo +1.99.1 test -p latexide-core --test pdf_service_contract --locked`. Először a hiányzó szolgáltatásra vagy állított viselkedésre FAIL legyen; környezet-/linkhibát ne könyvelj reprodukciónak.
- [ ] Valósítsd meg az első portot `crates/latexide-core/src/services/pdf/mod.rs` alatt; az alkalmazás-regisztrációt add át `integration/requests/P10.json` manifestben (schema, package_id, contract_version, requirement_ids, source_paths, hook_targets, test_commands, compatibility).
- [ ] Ugyanazzal a paranccsal legyen GREEN; tedd mellé R1–R5 célzott regresszióját a `review_focus_r1` … `review_focus_r5` tesztekbe. Az állítások a fenti öt elvárt viselkedést vizsgálják.
- [ ] Ellenőrizd a saját diffet, majd készíts kis commitot: `git add -- crates/latexide-core/src/services/pdf/mod.rs crates/latexide-core/tests/pdf_service_contract.rs integration/requests/P10.json` és `git commit -m "feat(p10): establish tested service slice"`. A dokumentált későbbi munkavégzéskor commitolj; ez a terv most nem futtatja ezeket.

## Követelményenkénti feladatsor

Minden alábbi sor külön tesztelhető részeredmény és saját kis commit. A teszt neve az ID kisbetűs, aláhúzásos alakja; az elfogadás mondata a kötelező assertion. A dependency provider mockja csak izolált teszthez elég; release-hez a valódi providerrel is teljesüljön.

### PDF-01 — Forward SyncTeX (P1, L)

**Kiindulás:** Compiler -synctex=1-et ad, navigációs szolgáltatás nincs.

**Módosítás:** Buildgenerációhoz kötött SyncTeX lekérdezés és PDFKit/Windows koordinátaadapter.

**Valódi bekötés előtt:** BLD-01 (P09), PDF-03 (P10).

- [ ] `pdf_01` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Caretből helyes oldal/pozíció; source-hash eltérésnél nem mutat pontosnak régi koordinátát.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test pdf_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P10` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `PDF-01` implemented/tested állapotúnak.

### PDF-02 — Inverse SyncTeX (P1, L)

**Kiindulás:** PDF-kattintásból nincs forrásugrás.

**Módosítás:** Natív PDF hit-test → SyncTeX location → közös editor navigation.

**Valódi bekötés előtt:** PDF-01 (P10), NAV-04 (P06).

- [ ] `pdf_02` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Másik include fájlba is helyesen ugrik; zoom/rotation/scaling és Unicode path tesztelt.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test pdf_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P10` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `PDF-02` implemented/tested állapotúnak.

### PDF-03 — PDF és szinkronartifact életciklusa (P1, M)

**Kiindulás:** Generációnként PDF és régi jelzés van; artifactmodell szűk.

**Módosítás:** Egységes BuildArtifact: PDF/SyncTeX/log/manifest; megnyitott generáció pinelése.

**Valódi bekötés előtt:** BLD-01 (P09).

- [ ] `pdf_03` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **PDF-re és SyncTeXre ugyanaz a generáció érvényes; takarítás nem töri a nézetet.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test pdf_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P10` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `PDF-03` implemented/tested állapotúnak.

### PDF-04 — PDF kereső és találatnavigáció (P1, M)

**Kiindulás:** Mac PDFKit szövegrender; alkalmazásszintű kereső nincs; Windows rasteroldal.

**Módosítás:** Közös PDF search/state és natív szöveg/range-highlight adapter.

**Valódi bekötés előtt:** PDF-03 (P10).

- [ ] `pdf_04` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Keresett ékezetes kifejezés találatai bejárhatók; új buildnél stale találat eldobódik.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test pdf_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P10` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `PDF-04` implemented/tested állapotúnak.

### PDF-05 — Oldal/zoom/viewport megőrzés (P2, M)

**Kiindulás:** PDF-cserénél Mac autoScales visszaáll; view state nem tartós.

**Módosítás:** Generációk és inspectormódok között workspace-owned oldalmapping és viewport/zoom mentése.

**Valódi bekötés előtt:** PDF-03 (P10).

- [ ] `pdf_05` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Kisebb forrásmódosítás és panelváltás után ugyanazon tartalom közelében marad az olvasó.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test pdf_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P10` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `PDF-05` implemented/tested állapotúnak.

### PDF-06 — PDF outline és thumbnails (P2, M)

**Kiindulás:** Oldalléptetés és zoom létezik.

**Módosítás:** Könyvjelző/oldalbélyegkép navigáció, lusta generálással.

**Valódi bekötés előtt:** PDF-03 (P10).

- [ ] `pdf_06` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Hosszú dolgozatnál gyors navigáció és mért thumbnail-memóriakeret.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test pdf_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P10` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `PDF-06` implemented/tested állapotúnak.

### PDF-07 — Oldalra ugrás, módok és rotáció (P2, M)

**Kiindulás:** Mac singlePageContinuous, fix toolbar; Windows egy raszteroldal.

**Módosítás:** Oldalszámmező, fit-width/page, continuous/two-up és rotation a platformképességek szerint.

**Valódi bekötés előtt:** PDF-01 (P10).

- [ ] `pdf_07` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Vegyes lapméret és elforgatás nem töri a forward/inverse koordinátát.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test pdf_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P10` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `PDF-07` implemented/tested állapotúnak.

### PDF-08 — Windows PDF-szöveg és kijelölés (P1, L)

**Kiindulás:** Windows.Data.Pdf raszterrender forrásban; szövegréteg nincs.

**Módosítás:** Kvalifikált natív PDF-adapter szöveg/keresés/link/accessibility képességekkel.

**Valódi bekötés előtt:** OS-01 (P21), PDF-04 (P10).

- [ ] `pdf_08` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Valós Windows gépen ékezetes szöveg kijelölhető, kereshető és felolvasható.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test pdf_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P10` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `PDF-08` implemented/tested állapotúnak.

### PDF-09 — PDF diff (P2, M)

**Kiindulás:** Nincs generáció-összehasonlítás.

**Módosítás:** Oldal-/szöveg-/vizuális diff előző vagy kiválasztott generációval.

**Valódi bekötés előtt:** PDF-03 (P10), BLD-15 (P09).

- [ ] `pdf_09` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Tördelésváltás és tényleges tartalomváltozás megkülönböztethető; eredet jelölt.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test pdf_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P10` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `PDF-09` implemented/tested állapotúnak.

### PDF-10 — PDF export, print és külső megnyitás (P2, S)

**Kiindulás:** Beágyazott preview van, külön publikációs parancsok nincsenek.

**Módosítás:** Natív export/print/open-external a kiválasztott generációra.

**Valódi bekötés előtt:** PDF-03 (P10).

- [ ] `pdf_10` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Régi artifact exportja is egyértelműen azonosított; felhasználó választ célhelyet.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test pdf_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P10` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `PDF-10` implemented/tested állapotúnak.

### PDF-11 — Nagy/hibás PDF erőforrásvédelme (P1, M)

**Kiindulás:** Kis minták ellenőrzöttek; nagy/sérült corpus nincs.

**Módosítás:** Bounded/lazy oldalfeldolgozás, hibajelzés, PDF-stressz és parserizoláció vizsgálata.

**Valódi bekötés előtt:** QA-03 (P23), BLD-03 (P09).

- [ ] `pdf_11` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Ezeroldalas és sérült fixture mellett az editor használható, nincs korlátlan memória.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test pdf_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P10` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `PDF-11` implemented/tested állapotúnak.

### PDF-12 — Annotáció és képlet-/ábranézet (P2, L)

**Kiindulás:** A preview fordított dokumentumot mutat.

**Módosítás:** Opcionális annotációs sidecar és képlet/ábra hover, forrásfájl módosítása nélkül.

**Valódi bekötés előtt:** PDF-03 (P10), LANG-10 (P05).

- [ ] `pdf_12` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Új build után bizonytalan annotáció jelölt; LaTeX forrás és PDF eredet megőrzött.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test pdf_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P10` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `PDF-12` implemented/tested állapotúnak.

## Task Z: Bekötés és átadás

- [ ] `python3 scripts/validate_development_packages.py` és `cargo +1.99.1 test -p latexide-core --test pdf_service_contract --locked` sikeres; a tesztparancs itt célállapot, futtatását külön rögzíteni kell.
- [ ] P00 alkalmazza a manifestben felsorolt közös hookokat; stagingben egyesével fusson a korábbi Rust/native/IPC/MCP regresszió a megváltozott rétegnek megfelelően. Nincs automatikus Save vagy új agent-edit jóváhagyás.
- [ ] Mac natív workflow valódi fókusz-/IME-/undo-eseménnyel; Windows külön MSBuild/link/launch/funkcionális kör. Linux saját runner/felület nélkül unrun. Nem natív csomagnál ezeket az adaptergazda végzi.
- [ ] `docs/development/results/P10.md` tartalmazza a teljesített ID-ket és reprodukálható bizonyítékot. P00 ennek alapján frissíti a handoffot, ledgert és a releváns skill-reference-et; más csomag ne írja párhuzamosan ezeket.

## Másolható indítófeladat

> Hajtsd végre a `docs/superpowers/plans/2026-10-31-parallel/p10-pdf.md` tervet. Előbb olvasd az AGENTS.md-t, a handoffot, a projekt-skillt, a közös csomagszerződést és a BASELINE_DELTA-t. Csak P10 saját útvonalait módosítsd, a közös fájlokhoz integration-manifestet adj. Őrizd meg a közvetlen agent-edit + explicit Save + Save után is elérhető undo szerződést. A követelményfüggőséget és platformi bizonyítékot ellenőrizd; planned/unrun állapotot ne nevezd késznek.
