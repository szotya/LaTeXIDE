# P05 — Inkrementális LaTeX nyelvi motor és refaktorálás Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A meglévő lexikális kiegészítést explicit lefedettségű szerkezeti/scope modellel és biztonságos refaktorálással egészíti ki.

**Architecture:** Inkrementális parse és include-gráf a VFS-snapshotra épül; dinamikus TeX-konstrukció partial/unknown. Módosító művelet kizárólag P02 revíziózott tranzakcióját kérheti.

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

- R1: Dinamikus TeX/conditionals esetén a lefedettség legyen partial, ne hamis biztos állítás.
- R2: Ciklusos include és makrórekurzió álljon meg korlátosan.
- R3: Régi snapshotra indult elemzés ne írja felül az új revízió diagnosztikáját.
- R4: Komment/verbatim/BOM/CRLF/emoji tartományai maradjanak a kanonikus koordinátarendszerben.
- R5: Rename preview és alkalmazás között változó bármely fájl esetén a teljes edit legyen elutasított.

---

## Tulajdon és indulás

**Branch:** `feature/p05-language`. **Profil:** core.

**Kizárólagosan létrehozható/módosítható útvonalak:**

- `crates/latexide-core/src/services/language/`
- `crates/latexide-core/tests/language_service_contract.rs`
- `integration/requests/P05.json`
- `docs/development/results/P05.md`

**Teszt:** `crates/latexide-core/tests/language_service_contract.rs`. **Eredménynapló:** `docs/development/results/P05.md`.

**Olvasandó jelenlegi belépési pontok:** `crates/latexide-core/src/tex_scan.rs`, `crates/latexide-core/src/project_index.rs`, `crates/latexide-core/src/completion.rs`.

Indulás: P00 contract-v1 után, fagyasztott fixture-ökkel és fake portokkal; a többi csomag teljes elkészülte nem indulási feltétel.

Integrációs szomszédok: P02, P03, P04, P07, P08, P15, P19. Ezek **nem csomagszintű topologikus sorrend**: az egyedi követelményfüggőségek maradnak irányadók. A kölcsönös portfüggést fixture és fokozatos bekötés oldja, nem két teljes csomag egymásra várása.

## Task 1: Önállóan ellenőrizhető szerződésszelet

**Create:** `crates/latexide-core/src/services/language/mod.rs`, `crates/latexide-core/tests/language_service_contract.rs`. **Consumes / produces:**

```rust
pub fn analyze(snapshot:&WorkspaceSnapshot, root:String, budget:AnalysisBudget) -> Result<AnalysisResult,LanguageError>;
// AnalysisBudget { max_files:u32, max_bytes:u64, deadline_ms:u64 }; AnalysisResult { snapshot_id:String, symbols:Vec<ScopedSymbol>, diagnostics:Vec<SourceDiagnostic>, coverage:Coverage }; ScopedSymbol { name:String, definition:SourceAnchor, scope:String }; Coverage::{Complete,Partial,Unknown}; LanguageError::{Cancelled,StaleSnapshot,Quota}
```

Az itt felsorolt DTO-mezőket P00 helyezi a `crates/latexide-contracts/src/` moduljaiba, serde serialize/deserialize és egyértelmű hiba-enummal. A szolgáltatás a saját modulban marad; ne hozz létre protocol→core→protocol függőségi kört. A port v1 helyi Rust-határ, nem automatikus új IPC-eszköz.

- [ ] **RED:** `crates/latexide-core/tests/language_service_contract.rs` alatt `contract_v1_first_slice` néven rögzítsd ezt az ellenőrzést: **Include-scope fixture-ben az elérhető makró célhelye pontos, dinamikus include esetén coverage Partial és nincs kitalált teljes feloldás.**
- [ ] Futtasd: `cargo +1.99.1 test -p latexide-core --test language_service_contract --locked`. Először a hiányzó szolgáltatásra vagy állított viselkedésre FAIL legyen; környezet-/linkhibát ne könyvelj reprodukciónak.
- [ ] Valósítsd meg az első portot `crates/latexide-core/src/services/language/mod.rs` alatt; az alkalmazás-regisztrációt add át `integration/requests/P05.json` manifestben (schema, package_id, contract_version, requirement_ids, source_paths, hook_targets, test_commands, compatibility).
- [ ] Ugyanazzal a paranccsal legyen GREEN; tedd mellé R1–R5 célzott regresszióját a `review_focus_r1` … `review_focus_r5` tesztekbe. Az állítások a fenti öt elvárt viselkedést vizsgálják.
- [ ] Ellenőrizd a saját diffet, majd készíts kis commitot: `git add -- crates/latexide-core/src/services/language/mod.rs crates/latexide-core/tests/language_service_contract.rs integration/requests/P05.json` és `git commit -m "feat(p05): establish tested service slice"`. A dokumentált későbbi munkavégzéskor commitolj; ez a terv most nem futtatja ezeket.

## Követelményenkénti feladatsor

Minden alábbi sor külön tesztelhető részeredmény és saját kis commit. A teszt neve az ID kisbetűs, aláhúzásos alakja; az elfogadás mondata a kötelező assertion. A dependency provider mockja csak izolált teszthez elég; release-hez a valódi providerrel is teljesüljön.

### LANG-01 — Inkrementális dokumentumszerkezet (P1, L)

**Kiindulás:** Lexikális lexer és korlátos szimbólumindex van.

**Módosítás:** Toleráns syntax tree és stabil tartományok; hibaálló inkrementális frissítés.

**Valódi bekötés előtt:** MCP-02 (P03).

- [ ] `lang_01` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Beillesztés után csak érintett részek frissülnek; félkész TeX nem fagyasztja a felületet.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test language_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P05` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `LANG-01` implemented/tested állapotúnak.

### LANG-02 — Projekt függőségi gráf és scope (P1, L)

**Kiindulás:** Projektmappát indexel; nem követi a TeX include/bib/package szemantikát.

**Módosítás:** Főfájlhoz kötött input/include/bibliography gráf és explicit részlegesség.

**Valódi bekötés előtt:** LANG-01 (P05), WS-08 (P08).

- [ ] `lang_02` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Két főfájl azonos labelje nem keveredik; ciklus és hiányzó include diagnosztizálható.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test language_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P05` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `LANG-02` implemented/tested állapotúnak.

### LANG-03 — Csomag- és környezetkiegészítés (P1, M)

**Kiindulás:** Saját makró/ref/cite már működik; csomagkatalógus nincs.

**Módosítás:** Használt csomagok verziózott parancs-/környezetmetaadatai, dokumentáció és provenance.

**Valódi bekötés előtt:** LANG-02 (P05).

- [ ] `lang_03` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Csak releváns csomag jelöltjei, korrekt argumentumok; hiányzó adat látható.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test language_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P05` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `LANG-03` implemented/tested állapotúnak.

### LANG-04 — Makró-scope és feltételes definíciók (P1, M)

**Kiindulás:** Definíciók lexikálisak, determinisztikus prioritással.

**Módosítás:** Gráfhoz kötött definíciók, környezetek és bizonytalan feloldás jelölése.

**Valódi bekötés előtt:** LANG-02 (P05).

- [ ] `lang_04` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Nem használt fejezet definíciója nem nyer indokolatlanul; dinamikus makróra nincs hamis bizonyosság.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test language_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P05` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `LANG-04` implemented/tested állapotúnak.

### LANG-05 — Build előtti élő diagnosztika (P1, L)

**Kiindulás:** Jelenleg fordítódiagnosztikák vannak.

**Módosítás:** LaTeX/BibTeX strukturális ellenőrzések vagy kvalifikált language server; külön diagnosztikagazda.

**Valódi bekötés előtt:** LANG-01 (P05), EXT-01 (P15).

- [ ] `lang_05` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Nem lezárt környezet/címkehiba build nélkül látszik; stale eredmény eldobódik.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test language_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P05` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `LANG-05` implemented/tested állapotúnak.

### LANG-07 — Find usages és hivatkozási gráf (P1, M)

**Kiindulás:** Címkék és kulcsok gyűjtöttek, használati helyek nincsenek.

**Módosítás:** Ref/cite/makró használatok revíziózott indexe és csoportos találatnézet.

**Valódi bekötés előtt:** LANG-02 (P05).

- [ ] `lang_07` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Többfájlos használatok és komment/verbatim kizárás a dokumentált scope szerint működik.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test language_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P05` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `LANG-07` implemented/tested állapotúnak.

### LANG-08 — Biztonságos szimbólum-átnevezés (P1, L)

**Kiindulás:** Nincs rename szolgáltatás.

**Módosítás:** Label/cite/makró rename konfliktusellenőrzéssel és atomi WorkspaceEdittel.

**Valódi bekötés előtt:** LANG-07 (P05), DOC-08 (P02).

- [ ] `lang_08` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Ütköző kulcs vagy stale fájl esetén nincs részleges átírás; egy undo mindent visszaállít.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test language_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P05` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `LANG-08` implemented/tested állapotúnak.

### LANG-09 — Quick fix és intentions (P2, M)

**Kiindulás:** Completion van; Alt-Enter/code-action rendszer nincs.

**Módosítás:** Típusos kódműveletek indoklással és pontos revíziózott szerkesztésekkel.

**Valódi bekötés előtt:** LANG-05 (P05), DOC-08 (P02).

- [ ] `lang_09` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Missing label/environment javítása determinisztikus; nem érint más dokumentumot véletlenül.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test language_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P05` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `LANG-09` implemented/tested állapotúnak.

### LANG-10 — Hover és argumentumsúgó (P1, M)

**Kiindulás:** Popup egysoros detailt ad.

**Módosítás:** Csomag/parancs, referencia és citation hover; kurzorhoz kötött signature help.

**Valódi bekötés előtt:** LANG-03 (P05).

- [ ] `lang_10` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Nyitott argumentumnál megfelelő paraméter; providerhibánál a gépelés tovább működik.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test language_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P05` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `LANG-10` implemented/tested állapotúnak.

### LANG-12 — Fájlútvonal-kiegészítés (P2, M)

**Kiindulás:** Parancs/kulcs jelöltek vannak, include/graphics útvonaljelölt nincs.

**Módosítás:** Projektmanifest-alapú relatív path completion, képtípus és idézés kezeléssel.

**Valódi bekötés előtt:** WS-01 (P04).

- [ ] `lang_12` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Szóközös/ékezetes fájl beszúrható; gyökéren kívüli cél csak explicit támogatott policyvel.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test language_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P05` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `LANG-12` implemented/tested állapotúnak.

### LANG-13 — Indexelés háttérben és látható állapottal (P1, M)

**Kiindulás:** 128 fájl/8 MiB/8192 szimbólum/10000 bejegyzés limit, szinkron refresh.

**Módosítás:** Közös háttérindex, konfigurált keret, prioritás és workspace-generation token.

**Valódi bekötés előtt:** MCP-02 (P03), WS-01 (P04).

- [ ] `lang_13` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Korlátos részleges eredmény látszik; cold scan és forced refresh nem blokkolja a főszálat.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test language_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P05` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `LANG-13` implemented/tested állapotúnak.

### LANG-14 — Szemantikus színezés (P2, M)

**Kiindulás:** A meglévő színezés lexikális.

**Módosítás:** Feloldott címkék/makrók és hibák külön, opcionális szemantikus rétegben.

**Valódi bekötés előtt:** LANG-02 (P05).

- [ ] `lang_14` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Lexikális fallback azonnal marad; újraszínezés nem módosít text/revízió/undo állapotot.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test language_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P05` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `LANG-14` implemented/tested állapotúnak.

### LANG-15 — Refaktorálás: extract, inline, move (P2, M)

**Kiindulás:** Nincsenek többfájlos refaktorálások.

**Módosítás:** Kijelölésből input/makró, fejezetmozgatás és importfrissítés korlátos műveletekkel.

**Valódi bekötés előtt:** DOC-08 (P02), LANG-02 (P05).

- [ ] `lang_15` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Hatáslista és konfliktus látható; agent közvetlen alkalmazását nem váltja fel approval-kapu.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test language_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P05` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `LANG-15` implemented/tested állapotúnak.

### LANG-16 — Kiegészítés rangsorolás és saját katalógus (P2, M)

**Kiindulás:** Case-sensitive prefix, determinisztikus sorrend, legfeljebb 20 jelölt.

**Módosítás:** Fuzzy/relevancia és opcionális használati rangsor; user snippet/katalógus import.

**Valódi bekötés előtt:** LANG-03 (P05), EDT-04 (P07).

- [ ] `lang_16` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Ugyanaz az input reprodukálható; személyes rangsor törölhető, provenance megmarad.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test language_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P05` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `LANG-16` implemented/tested állapotúnak.

### LANG-17 — Nyelvi szolgáltatás képességalkuja (P2, M)

**Kiindulás:** Saját API létezik; általános LSP kliens nincs.

**Módosítás:** Egy közös LSP adapter, verzió-/UTF-16-/cancel-/restart-szerződéssel; Texlab proof-of-concept.

**Valódi bekötés előtt:** EXT-01 (P15), DOC-08 (P02).

- [ ] `lang_17` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **didOpen/didChange dirty Rust tartalmat küld; WorkspaceEdit ugyanazon undo-ba kerül.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test language_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P05` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `LANG-17` implemented/tested állapotúnak.

### LANG-18 — Parser és szemantika korpusz (P1, M)

**Kiindulás:** 21 projektcompletion-kontraktus és rossz-input őrteszt van.

**Módosítás:** Valós class/style/bib/makrókorpusz, differential teszt és determinisztikus hibabudget.

**Valódi bekötés előtt:** QA-02 (P19).

- [ ] `lang_18` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Dokumentált lefedettség és false-positive arány; túlkomplex TeX-nél explicit részleges állapot.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test language_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P05` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `LANG-18` implemented/tested állapotúnak.

## Task Z: Bekötés és átadás

- [ ] `python3 scripts/validate_development_packages.py` és `cargo +1.99.1 test -p latexide-core --test language_service_contract --locked` sikeres; a tesztparancs itt célállapot, futtatását külön rögzíteni kell.
- [ ] P00 alkalmazza a manifestben felsorolt közös hookokat; stagingben egyesével fusson a korábbi Rust/native/IPC/MCP regresszió a megváltozott rétegnek megfelelően. Nincs automatikus Save vagy új agent-edit jóváhagyás.
- [ ] Mac natív workflow valódi fókusz-/IME-/undo-eseménnyel; Windows külön MSBuild/link/launch/funkcionális kör. Linux saját runner/felület nélkül unrun. Nem natív csomagnál ezeket az adaptergazda végzi.
- [ ] `docs/development/results/P05.md` tartalmazza a teljesített ID-ket és reprodukálható bizonyítékot. P00 ennek alapján frissíti a handoffot, ledgert és a releváns skill-reference-et; más csomag ne írja párhuzamosan ezeket.

## Másolható indítófeladat

> Hajtsd végre a `docs/superpowers/plans/2026-10-31-parallel/p05-language.md` tervet. Előbb olvasd az AGENTS.md-t, a handoffot, a projekt-skillt, a közös csomagszerződést és a BASELINE_DELTA-t. Csak P05 saját útvonalait módosítsd, a közös fájlokhoz integration-manifestet adj. Őrizd meg a közvetlen agent-edit + explicit Save + Save után is elérhető undo szerződést. A követelményfüggőséget és platformi bizonyítékot ellenőrizd; planned/unrun állapotot ne nevezd késznek.
