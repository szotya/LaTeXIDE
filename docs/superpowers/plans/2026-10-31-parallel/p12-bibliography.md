# P12 — BibTeX adatmodell és kutatói import Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Strukturált, dirty bufferrel következetes bibliográfiát és ellenőrizhető külső importot ad.

**Architecture:** Lossless BibTeX/BibLaTeX parse és hibaállapot; import előbb szerkeszthető rekordot ad, az alkalmazás P02 tranzakciója. Hálózat és titokhatár P18 policyhoz kötött.

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

- R1: Hibás vagy félbegépelt BibTeX megőrizze az eredeti szöveget és hibapozíciót adjon.
- R2: Duplikált kulcs feloldása egyezzen a completion/definition jelenlegi szerződésével.
- R3: DOI/arXiv/Zotero válasz ne váljon automatikusan mentett vagy futtatott tartalommá.
- R4: LaTeX escape és nested brace ne vesszen el normalizáláskor.
- R5: Hálózati timeout/offline mód és provider rate limit ne blokkolja a helyi szerkesztést.

---

## Tulajdon és indulás

**Branch:** `feature/p12-bibliography`. **Profil:** core.

**Kizárólagosan létrehozható/módosítható útvonalak:**

- `crates/latexide-core/src/services/bibliography/`
- `crates/latexide-core/tests/bibliography_contract.rs`
- `integration/requests/P12.json`
- `docs/development/results/P12.md`

**Teszt:** `crates/latexide-core/tests/bibliography_contract.rs`. **Eredménynapló:** `docs/development/results/P12.md`.

**Olvasandó jelenlegi belépési pontok:** `crates/latexide-core/src/project_index.rs`, `crates/latexide-core/src/tex_scan.rs`.

Indulás: P00 contract-v1 után, fagyasztott fixture-ökkel és fake portokkal; a többi csomag teljes elkészülte nem indulási feltétel.

Integrációs szomszédok: P05, P18. Ezek **nem csomagszintű topologikus sorrend**: az egyedi követelményfüggőségek maradnak irányadók. A kölcsönös portfüggést fixture és fokozatos bekötés oldja, nem két teljes csomag egymásra várása.

## Task 1: Önállóan ellenőrizhető szerződésszelet

**Create:** `crates/latexide-core/src/services/bibliography/mod.rs`, `crates/latexide-core/tests/bibliography_contract.rs`. **Consumes / produces:**

```rust
pub fn inspect_bibliography(source:&str) -> BibliographyReport;
// BibliographyReport { entries:Vec<BibEntry>, diagnostics:Vec<SourceDiagnostic>, duplicate_keys:Vec<String> }; BibEntry { key:String, entry_type:String, fields:Vec<BibField>, range:Utf16Range }; BibField { name:String, raw_value:String }
```

Az itt felsorolt DTO-mezőket P00 helyezi a `crates/latexide-contracts/src/` moduljaiba, serde serialize/deserialize és egyértelmű hiba-enummal. A szolgáltatás a saját modulban marad; ne hozz létre protocol→core→protocol függőségi kört. A port v1 helyi Rust-határ, nem automatikus új IPC-eszköz.

- [ ] **RED:** `crates/latexide-core/tests/bibliography_contract.rs` alatt `contract_v1_first_slice` néven rögzítsd ezt az ellenőrzést: **Két azonos BibTeX-kulcs explicit duplikáció; megjegyzés és brace-nested mező lossless újraszerializálható.**
- [ ] Futtasd: `cargo +1.99.1 test -p latexide-core --test bibliography_contract --locked`. Először a hiányzó szolgáltatásra vagy állított viselkedésre FAIL legyen; környezet-/linkhibát ne könyvelj reprodukciónak.
- [ ] Valósítsd meg az első portot `crates/latexide-core/src/services/bibliography/mod.rs` alatt; az alkalmazás-regisztrációt add át `integration/requests/P12.json` manifestben (schema, package_id, contract_version, requirement_ids, source_paths, hook_targets, test_commands, compatibility).
- [ ] Ugyanazzal a paranccsal legyen GREEN; tedd mellé R1–R5 célzott regresszióját a `review_focus_r1` … `review_focus_r5` tesztekbe. Az állítások a fenti öt elvárt viselkedést vizsgálják.
- [ ] Ellenőrizd a saját diffet, majd készíts kis commitot: `git add -- crates/latexide-core/src/services/bibliography/mod.rs crates/latexide-core/tests/bibliography_contract.rs integration/requests/P12.json` és `git commit -m "feat(p12): establish tested service slice"`. A dokumentált későbbi munkavégzéskor commitolj; ez a terv most nem futtatja ezeket.

## Követelményenkénti feladatsor

Minden alábbi sor külön tesztelhető részeredmény és saját kis commit. A teszt neve az ID kisbetűs, aláhúzásos alakja; az elfogadás mondata a kötelező assertion. A dependency provider mockja csak izolált teszthez elég; release-hez a valódi providerrel is teljesüljön.

### LANG-11 — BibTeX adatmodell és ellenőrzés (P1, L)

**Kiindulás:** Entry-kulcsok lexikális indexe működik; teljes bibliográfiai feloldás nincs.

**Módosítás:** Mezők, string/crossref/xdata, duplikáció és biblatex-séma kezelése.

**Valódi bekötés előtt:** LANG-01 (P05).

- [ ] `lang_11` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Dupla kulcs, örökölt mező és hibás entry célzottan jelezhető; eredeti forrás megőrzött.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test bibliography_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P12` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `LANG-11` implemented/tested állapotúnak.

### RES-03 — Bibliográfiai részletek és duplikáció (P1, M)

**Kiindulás:** Kulcskiegészítés van, bibliográfiai editor nincs.

**Módosítás:** Entry detail, required fields és duplicate-key/content ellenőrző nézet.

**Valódi bekötés előtt:** LANG-11 (P12).

- [ ] `res_03` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Hiányos vagy duplikált adat célzottan javítható, forrás és formázás megőrzött.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test bibliography_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P12` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `RES-03` implemented/tested állapotúnak.

### RES-04 — DOI/arXiv/Zotero import (P2, M)

**Kiindulás:** Nincs kutatói importintegráció.

**Módosítás:** Opcionális provider, cache, provenance és reviewzható BibTeX-diff.

**Valódi bekötés előtt:** RES-03 (P12), SEC-06 (P18).

- [ ] `res_04` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Hálózati hiba nem módosít fájlt; importált adat javítható, kulcsütközés kezelhető.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test bibliography_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P12` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `RES-04` implemented/tested állapotúnak.

## Task Z: Bekötés és átadás

- [ ] `python3 scripts/validate_development_packages.py` és `cargo +1.99.1 test -p latexide-core --test bibliography_contract --locked` sikeres; a tesztparancs itt célállapot, futtatását külön rögzíteni kell.
- [ ] P00 alkalmazza a manifestben felsorolt közös hookokat; stagingben egyesével fusson a korábbi Rust/native/IPC/MCP regresszió a megváltozott rétegnek megfelelően. Nincs automatikus Save vagy új agent-edit jóváhagyás.
- [ ] Mac natív workflow valódi fókusz-/IME-/undo-eseménnyel; Windows külön MSBuild/link/launch/funkcionális kör. Linux saját runner/felület nélkül unrun. Nem natív csomagnál ezeket az adaptergazda végzi.
- [ ] `docs/development/results/P12.md` tartalmazza a teljesített ID-ket és reprodukálható bizonyítékot. P00 ennek alapján frissíti a handoffot, ledgert és a releváns skill-reference-et; más csomag ne írja párhuzamosan ezeket.

## Másolható indítófeladat

> Hajtsd végre a `docs/superpowers/plans/2026-10-31-parallel/p12-bibliography.md` tervet. Előbb olvasd az AGENTS.md-t, a handoffot, a projekt-skillt, a közös csomagszerződést és a BASELINE_DELTA-t. Csak P12 saját útvonalait módosítsd, a közös fájlokhoz integration-manifestet adj. Őrizd meg a közvetlen agent-edit + explicit Save + Save után is elérhető undo szerződést. A követelményfüggőséget és platformi bizonyítékot ellenőrizd; planned/unrun állapotot ne nevezd késznek.
