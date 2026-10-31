# P06 — Projektkeresés és szemantikus navigáció Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Egységes keresést, outline-t, vissza/előre történetet és a meglévő definícióugrás bővítését adja.

**Architecture:** Snapshothoz kötött találatok forrásankort tartalmaznak; megnyitáskor revízió és célidentitás újraellenőrzött. A most kész Cmd-click adapter megmarad, a szolgáltatás ambiguity/peek/history irányba bővül.

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

- R1: Megnyitás előtt megváltozó találat ne rossz szöveget jelöljön ki.
- R2: Két azonos nevű definíció esetén a felület mutassa az alternatívákat.
- R3: Back/forward ne módosítsa a forrást és ne veszítse el a korábbi viewportot.
- R4: Hibás vagy költséges regex explicit hiba/deadline legyen.
- R5: Keresés-csere preview után stale fájl esetén a teljes tranzakció álljon meg.

---

## Tulajdon és indulás

**Branch:** `feature/p06-navigation`. **Profil:** core.

**Kizárólagosan létrehozható/módosítható útvonalak:**

- `crates/latexide-core/src/services/navigation/`
- `crates/latexide-core/tests/navigation_service_contract.rs`
- `integration/requests/P06.json`
- `docs/development/results/P06.md`

**Teszt:** `crates/latexide-core/tests/navigation_service_contract.rs`. **Eredménynapló:** `docs/development/results/P06.md`.

**Olvasandó jelenlegi belépési pontok:** `crates/latexide-core/src/definition.rs`, `apps/macos/Sources/LaTeXIDEKit/Workspace/DefinitionNavigation.swift`, `docs/definition-navigation-verification.md`.

Indulás: P00 contract-v1 után, fagyasztott fixture-ökkel és fake portokkal; a többi csomag teljes elkészülte nem indulási feltétel.

Integrációs szomszédok: P02, P03, P04, P05, P07, P08, P09, P12. Ezek **nem csomagszintű topologikus sorrend**: az egyedi követelményfüggőségek maradnak irányadók. A kölcsönös portfüggést fixture és fokozatos bekötés oldja, nem két teljes csomag egymásra várása.

## Task 1: Önállóan ellenőrizhető szerződésszelet

**Create:** `crates/latexide-core/src/services/navigation/mod.rs`, `crates/latexide-core/tests/navigation_service_contract.rs`. **Consumes / produces:**

```rust
pub fn search(snapshot:&WorkspaceSnapshot, query:SearchQuery) -> Result<SearchPage,SearchError>;
// SearchQuery { text:String, regex:bool, case_sensitive:bool, cursor:Option<String>, max_results:u32 }; SearchPage { snapshot_id:String, hits:Vec<SearchHit>, next_cursor:Option<String> }; SearchHit { anchor:SourceAnchor, preview:String }; SearchError::{InvalidRegex,StaleCursor,Cancelled,Quota}
```

Az itt felsorolt DTO-mezőket P00 helyezi a `crates/latexide-contracts/src/` moduljaiba, serde serialize/deserialize és egyértelmű hiba-enummal. A szolgáltatás a saját modulban marad; ne hozz létre protocol→core→protocol függőségi kört. A port v1 helyi Rust-határ, nem automatikus új IPC-eszköz.

- [ ] **RED:** `crates/latexide-core/tests/navigation_service_contract.rs` alatt `contract_v1_first_slice` néven rögzítsd ezt az ellenőrzést: **Dirty chapter.tex találata revízióhoz kötött; új snapshot alatt régi lapozókurzor StaleCursor, nem kevert találatlista.**
- [ ] Futtasd: `cargo +1.99.1 test -p latexide-core --test navigation_service_contract --locked`. Először a hiányzó szolgáltatásra vagy állított viselkedésre FAIL legyen; környezet-/linkhibát ne könyvelj reprodukciónak.
- [ ] Valósítsd meg az első portot `crates/latexide-core/src/services/navigation/mod.rs` alatt; az alkalmazás-regisztrációt add át `integration/requests/P06.json` manifestben (schema, package_id, contract_version, requirement_ids, source_paths, hook_targets, test_commands, compatibility).
- [ ] Ugyanazzal a paranccsal legyen GREEN; tedd mellé R1–R5 célzott regresszióját a `review_focus_r1` … `review_focus_r5` tesztekbe. Az állítások a fenti öt elvárt viselkedést vizsgálják.
- [ ] Ellenőrizd a saját diffet, majd készíts kis commitot: `git add -- crates/latexide-core/src/services/navigation/mod.rs crates/latexide-core/tests/navigation_service_contract.rs integration/requests/P06.json` és `git commit -m "feat(p06): establish tested service slice"`. A dokumentált későbbi munkavégzéskor commitolj; ez a terv most nem futtatja ezeket.

## Követelményenkénti feladatsor

Minden alábbi sor külön tesztelhető részeredmény és saját kis commit. A teszt neve az ID kisbetűs, aláhúzásos alakja; az elfogadás mondata a kötelező assertion. A dependency provider mockja csak izolált teszthez elég; release-hez a valódi providerrel is teljesüljön.

### LANG-06 — Definícióra ugrás és peek (P1, M)

**Kiindulás:** A történeti audit óta saját makró/ref/cite Cmd-click és pontos dirty/disk célhely elkészült; scope, peek, ambiguity picker és történet továbbra hiány. Lásd BASELINE_DELTA.

**Módosítás:** Új lexikális navigáció integrációs kvalifikációja, majd peek, scope/ambiguity és history.

**Valódi bekötés előtt:** LANG-02 (P05), NAV-04 (P06).

- [ ] `lang_06` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Dirty másik fájl definíciójára ugrik; Unicode-tartomány, stale/limit hiba és visszanavigáció helyes.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test navigation_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P06` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `LANG-06` implemented/tested állapotúnak.

### NAV-01 — Projekt-tartalomkeresés (P1, L)

**Kiindulás:** Quick Open fájlnevet, natív Find egy fájlt keres.

**Módosítás:** Közös streamelt tartalomkeresés scope/ignore/cancel támogatással és dirty overlayjel.

**Valódi bekötés előtt:** WS-01 (P04), MCP-02 (P03).

- [ ] `nav_01` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Dirty szövegben is talál; találat revízióhoz kötött; nagy projekt megszakítható.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test navigation_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P06` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `NAV-01` implemented/tested állapotúnak.

### NAV-02 — Projekt-szintű keresés és csere (P1, L)

**Kiindulás:** Nincs projektcserélő szolgáltatás.

**Módosítás:** Regex/replace terv és atomi többfájlos alkalmazás; opcionális diff-áttekintés.

**Valódi bekötés előtt:** NAV-01 (P06), DOC-08 (P02).

- [ ] `nav_02` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Stale találat nem ír felül új szöveget; változás közösen visszavonható.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test navigation_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P06` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `NAV-02` implemented/tested állapotúnak.

### NAV-03 — Dokumentum-outline (P1, M)

**Kiindulás:** Nincs section/environment/label navigációs nézet.

**Módosítás:** Közös outline hierarchy és aktuális fejezet követése.

**Valódi bekötés előtt:** LANG-01 (P05), LANG-02 (P05).

- [ ] `nav_03` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Multifile projekt fejezetei és dirty címei frissülnek; kattintás helyes sorhoz ugrik.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test navigation_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P06` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `NAV-03` implemented/tested állapotúnak.

### NAV-04 — Előre/hátra navigációs előzmény (P1, M)

**Kiindulás:** Fájlonkénti viewport van, általános helytörténet nincs.

**Módosítás:** Workspace-owned Location history, fájlnév-/revízióváltozáshoz igazítva.

**Valódi bekötés előtt:** WS-06 (P08).

- [ ] `nav_04` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Definition→reference→vissza visszaadja a caret/viewportot tab-elrejtés után is.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test navigation_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P06` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `NAV-04` implemented/tested állapotúnak.

### NAV-05 — Search Everywhere és fuzzy Quick Open (P2, M)

**Kiindulás:** Substring fájlszűrés, első 100 eredmény és szűk parancslista van.

**Módosítás:** Fájl/szimbólum/parancs/beállítás egységes provider, fuzzy és MRU.

**Valódi bekötés előtt:** WS-06 (P08), LANG-06 (P06).

- [ ] `nav_05` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Nagy listán rangsorolt találat; billentyűzet, empty/error/loading állapot következetes.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test navigation_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P06` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `NAV-05` implemented/tested állapotúnak.

### NAV-06 — Bookmarks és TODO nézet (P2, S)

**Kiindulás:** Nincs közös könyvjelző-/TODO-szolgáltatás.

**Módosítás:** Tartós, szerkesztéskövető jelölők és scope-olt TODO index.

**Valódi bekötés előtt:** WS-01 (P04), DOC-09 (P02).

- [ ] `nav_06` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Törölt/átnevezett fájl jelölői kezelhetők; user/agent edit után helyük követhető.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test navigation_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P06` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `NAV-06` implemented/tested állapotúnak.

### NAV-07 — Szemantikus breadcrumb (P2, M)

**Kiindulás:** Jelenleg projekt/path breadcrumb van.

**Módosítás:** Section/environment/symbol breadcrumb és kiválasztás.

**Valódi bekötés előtt:** NAV-03 (P06).

- [ ] `nav_07` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Beágyazott környezetből pontos navigáció; nem csak fájlútvonalat mutat.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test navigation_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P06` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `NAV-07` implemented/tested állapotúnak.

### NAV-08 — Hibapontok és gyors navigáció (P1, M)

**Kiindulás:** Compiler file/line kattintás van, range és problémahierarchia részleges.

**Módosítás:** Normalizált helyek, következő/előző hiba és keyboard-focus visszaadás.

**Valódi bekötés előtt:** BLD-05 (P09), EDT-10 (P07).

- [ ] `nav_08` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Abszolút/relatív/ékezetes/Windows path helyes; külső fájl nem nyílik meg tévesen.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test navigation_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P06` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `NAV-08` implemented/tested állapotúnak.

### NAV-09 — Címke/citation kiválasztó kereséssel (P2, M)

**Kiindulás:** Completion kulcsot ad, külön részletes kereső nincs.

**Módosítás:** Bibliográfiai/metaadat kereső, ref preview és több kulcs beszúrása.

**Valódi bekötés előtt:** LANG-11 (P12).

- [ ] `nav_09` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Szerző/cím szerinti találat helyes BibTeX-kulcsot illeszt, meglévő listát megőrzi.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test navigation_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P06` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `NAV-09` implemented/tested állapotúnak.

### NAV-10 — Projektfa mint műveleti felület (P2, M)

**Kiindulás:** Fa és fájlmegnyitás van; létrehozás/rename/drag/drop nincs.

**Módosítás:** Fájlműveleti kontextusmenü, drag/drop és reveal a közös műveletekre kötve.

**Valódi bekötés előtt:** DOC-09 (P02), WS-01 (P04).

- [ ] `nav_10` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Fa művelete ugyanazt az undo/recoveryt használja, mint parancs és agent.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test navigation_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P06` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `NAV-10` implemented/tested állapotúnak.

### NAV-11 — Használati scope és mentett keresések (P2, S)

**Kiindulás:** Nincs scope modell.

**Módosítás:** Main graph/open files/folder/glob scope; megőrzött keresés és ignore-indoklás.

**Valódi bekötés előtt:** NAV-01 (P06), LANG-02 (P05).

- [ ] `nav_11` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Felhasználó látja, mi maradt ki; scopeváltás nem ad régi querytalálatot.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test navigation_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P06` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `NAV-11` implemented/tested állapotúnak.

## Task Z: Bekötés és átadás

- [ ] `python3 scripts/validate_development_packages.py` és `cargo +1.99.1 test -p latexide-core --test navigation_service_contract --locked` sikeres; a tesztparancs itt célállapot, futtatását külön rögzíteni kell.
- [ ] P00 alkalmazza a manifestben felsorolt közös hookokat; stagingben egyesével fusson a korábbi Rust/native/IPC/MCP regresszió a megváltozott rétegnek megfelelően. Nincs automatikus Save vagy új agent-edit jóváhagyás.
- [ ] Mac natív workflow valódi fókusz-/IME-/undo-eseménnyel; Windows külön MSBuild/link/launch/funkcionális kör. Linux saját runner/felület nélkül unrun. Nem natív csomagnál ezeket az adaptergazda végzi.
- [ ] `docs/development/results/P06.md` tartalmazza a teljesített ID-ket és reprodukálható bizonyítékot. P00 ennek alapján frissíti a handoffot, ledgert és a releváns skill-reference-et; más csomag ne írja párhuzamosan ezeket.

## Másolható indítófeladat

> Hajtsd végre a `docs/superpowers/plans/2026-10-31-parallel/p06-navigation.md` tervet. Előbb olvasd az AGENTS.md-t, a handoffot, a projekt-skillt, a közös csomagszerződést és a BASELINE_DELTA-t. Csak P06 saját útvonalait módosítsd, a közös fájlokhoz integration-manifestet adj. Őrizd meg a közvetlen agent-edit + explicit Save + Save után is elérhető undo szerződést. A követelményfüggőséget és platformi bizonyítékot ellenőrizd; planned/unrun állapotot ne nevezd késznek.
