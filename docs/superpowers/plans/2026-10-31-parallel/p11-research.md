# P11 — Írás, sablonok és publikációs export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Sablontól a beadásig reprodukálható kutatói/egyetemi dokumentumfolyamatot ad.

**Architecture:** A sablon és preflight compiler-/forrásmanifesthez kötött; export allowlist alapján új könyvtárba történik. Research pipeline a taskportot használja, a fókuszprofil a settingsportot.

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

- R1: Sablon megnyitása ne írja felül a létező felhasználói projektet.
- R2: Titkot vagy sessiondescriptort tartalmazó export ne készüljön el csendben.
- R3: Beadáshoz szükséges ábra/bib/custom class hiánya konkrét preflight hibát adjon.
- R4: Magyar ékezet, szóközös útvonal és főfájlnév pontjai maradjanak működők.
- R5: Notebook/Quarto/latexdiff pipeline csak megfelelő trust és toolverzió mellett induljon.

---

## Tulajdon és indulás

**Branch:** `feature/p11-research`. **Profil:** core.

**Kizárólagosan létrehozható/módosítható útvonalak:**

- `crates/latexide-core/src/services/research/`
- `crates/latexide-core/tests/research_contract.rs`
- `integration/requests/P11.json`
- `docs/development/results/P11.md`

**Teszt:** `crates/latexide-core/tests/research_contract.rs`. **Eredménynapló:** `docs/development/results/P11.md`.

**Olvasandó jelenlegi belépési pontok:** `tests/fixtures`, `tools/texlive/package-roots.json`, `docs/IDE_PRODUCT_DIRECTION.md`.

Indulás: P00 contract-v1 után, fagyasztott fixture-ökkel és fake portokkal; a többi csomag teljes elkészülte nem indulási feltétel.

Integrációs szomszédok: P01, P02, P05, P07, P08, P09, P17, P18. Ezek **nem csomagszintű topologikus sorrend**: az egyedi követelményfüggőségek maradnak irányadók. A kölcsönös portfüggést fixture és fokozatos bekötés oldja, nem két teljes csomag egymásra várása.

## Task 1: Önállóan ellenőrizhető szerződésszelet

**Create:** `crates/latexide-core/src/services/research/mod.rs`, `crates/latexide-core/tests/research_contract.rs`. **Consumes / produces:**

```rust
pub fn plan_export(snapshot:&WorkspaceSnapshot, policy:ExportPolicy) -> Result<ExportManifest,ExportError>;
// ExportPolicy { include_sources:bool, include_pdf:bool, allow_paths:Vec<String> }; ExportManifest { snapshot_id:String, files:Vec<String>, warnings:Vec<String> }; ExportError::{SecretCandidate,MissingArtifact,OutsideRoot,StaleBuild}
```

Az itt felsorolt DTO-mezőket P00 helyezi a `crates/latexide-contracts/src/` moduljaiba, serde serialize/deserialize és egyértelmű hiba-enummal. A szolgáltatás a saját modulban marad; ne hozz létre protocol→core→protocol függőségi kört. A port v1 helyi Rust-határ, nem automatikus új IPC-eszköz.

- [ ] **RED:** `crates/latexide-core/tests/research_contract.rs` alatt `contract_v1_first_slice` néven rögzítsd ezt az ellenőrzést: **Beadási csomag tartalmazza a szükséges tex/bib/kép/PDF fájlokat, .env és IDE-sessionadat nélkül; stale PDF blokkol.**
- [ ] Futtasd: `cargo +1.99.1 test -p latexide-core --test research_contract --locked`. Először a hiányzó szolgáltatásra vagy állított viselkedésre FAIL legyen; környezet-/linkhibát ne könyvelj reprodukciónak.
- [ ] Valósítsd meg az első portot `crates/latexide-core/src/services/research/mod.rs` alatt; az alkalmazás-regisztrációt add át `integration/requests/P11.json` manifestben (schema, package_id, contract_version, requirement_ids, source_paths, hook_targets, test_commands, compatibility).
- [ ] Ugyanazzal a paranccsal legyen GREEN; tedd mellé R1–R5 célzott regresszióját a `review_focus_r1` … `review_focus_r5` tesztekbe. Az állítások a fenti öt elvárt viselkedést vizsgálják.
- [ ] Ellenőrizd a saját diffet, majd készíts kis commitot: `git add -- crates/latexide-core/src/services/research/mod.rs crates/latexide-core/tests/research_contract.rs integration/requests/P11.json` és `git commit -m "feat(p11): establish tested service slice"`. A dokumentált későbbi munkavégzéskor commitolj; ez a terv most nem futtatja ezeket.

## Követelményenkénti feladatsor

Minden alábbi sor külön tesztelhető részeredmény és saját kis commit. A teszt neve az ID kisbetűs, aláhúzásos alakja; az elfogadás mondata a kötelező assertion. A dependency provider mockja csak izolált teszthez elég; release-hez a valódi providerrel is teljesüljön.

### RES-01 — Szó-/karakter-/szakaszszám (P1, M)

**Kiindulás:** Nincs kutatói mérőszolgáltatás.

**Módosítás:** TeX-tudatos számlálás total/section/selection scope-pal, magyar szabályok dokumentálásával.

**Valódi bekötés előtt:** LANG-02 (P05).

- [ ] `res_01` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Komment, képlet és bibliográfia kezelése deklarált; kézi fixture-eredményekkel ellenőrzött.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test research_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P11` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `RES-01` implemented/tested állapotúnak.

### RES-02 — Valódi projekt- és egyetemi sablonok (P1, L)

**Kiindulás:** Basic/multifile fixture létezik; template browser nincs.

**Módosítás:** Verziózott thesis/article/beamer sablonkatalógus forrás/licenc/engine metaadatokkal.

**Valódi bekötés előtt:** DOC-06 (P02), BLD-09 (P09).

- [ ] `res_02` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Új projekt azonnal offline fordul; intézményi követelmények konkrét verzióhoz kötöttek.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test research_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P11` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `RES-02` implemented/tested állapotúnak.

### RES-05 — Ábra- és táblázat munkafolyamat (P2, M)

**Kiindulás:** includegraphics snippet létezik; assetnézet nincs.

**Módosítás:** Kép-preview, relatív import, caption/label és táblázatsnippet/parancsok.

**Valódi bekötés előtt:** LANG-12 (P05), DOC-09 (P02).

- [ ] `res_05` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Átmozgatott asset hivatkozása javul; nagy kép nem blokkolja a szerkesztőt.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test research_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P11` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `RES-05` implemented/tested állapotúnak.

### RES-06 — Képlet- és környezetsegéd (P2, M)

**Kiindulás:** Néhány builtin math parancs van.

**Módosítás:** Kereshető szimbólumkatalógus, környezetváltás és kijelölés köré csomagolás.

**Valódi bekötés előtt:** EDT-04 (P07), EDT-12 (P07).

- [ ] `res_06` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Már meglévő delimiter/caret megőrzött, egy undo; nem kell modellhívás.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test research_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P11` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `RES-06` implemented/tested állapotúnak.

### RES-07 — Revíziók összehasonlítása és latexdiff (P2, M)

**Kiindulás:** Nincs dokumentumszintű változáscsomag.

**Módosítás:** Két forrásállapotból korlátozott latexdiff/annotált PDF pipeline.

**Valódi bekötés előtt:** BLD-14 (P09), DOC-05 (P01).

- [ ] `res_07` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **A diff külön generáció, nem írja felül az eredeti forrást vagy publikációs artifactot.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test research_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P11` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `RES-07` implemented/tested állapotúnak.

### RES-08 — Beadási/exportcsomag (P1, M)

**Kiindulás:** Csak helyi projekt és build-output van.

**Módosítás:** Forrás+bib+asset+manifest ZIP és arXiv/journal profil szerinti ellenőrzés.

**Valódi bekötés előtt:** BLD-09 (P09), SEC-06 (P18).

- [ ] `res_08` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Csomag tiszta könyvtárban újrafordul; cache, sessiontoken és titok nem kerül bele.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test research_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P11` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `RES-08` implemented/tested állapotúnak.

### RES-09 — Notebook/Quarto/Rnw kutatói pipeline (P2, L)

**Kiindulás:** Nincs notebook vagy literate build.

**Módosítás:** Választható external-tool profil Python/R/Julia/Quarto kimenettel és input provenance-szal.

**Valódi bekötés előtt:** DEV-03 (P17), BLD-14 (P09).

- [ ] `res_09` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Adatból ábra/táblázat és LaTeX export ismételhető; futtatási környezet azonosított.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test research_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P11` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `RES-09` implemented/tested állapotúnak.

### RES-10 — Fókuszált írás és kutatói profil (P2, M)

**Kiindulás:** Panelek elrejthetők, profilegység nincs.

**Módosítás:** Writing/review/developer profil ugyanazon workspace állapotai fölött.

**Valódi bekötés előtt:** WS-05 (P08).

- [ ] `res_10` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Profilváltás megőrzi a draftot, caret/viewportot és dirty fájlokat.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test research_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P11` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `RES-10` implemented/tested állapotúnak.

### RES-11 — Glossary, acronyms és nomenclature (P2, M)

**Kiindulás:** Nincs dedikált nyelvi modell vagy UI.

**Módosítás:** Támogatott csomagok kulcsai, beillesztés, használatok és buildrecipe.

**Valódi bekötés előtt:** LANG-03 (P05), BLD-07 (P09).

- [ ] `res_11` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Gls-kulcs és indexgenerálás offline corpusban helyes; unsupported macro jelölt.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test research_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P11` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `RES-11` implemented/tested állapotúnak.

### RES-12 — Publikációs preflight (P2, L)

**Kiindulás:** Nincs beadási ellenőrző workflow.

**Módosítás:** Hivatkozás/ábra/font/layout követelmények verziózott, célprofilos vizsgálata.

**Valódi bekötés előtt:** RES-08 (P11), BLD-05 (P09).

- [ ] `res_12` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Minden eredmény input/build/profilhoz kötött; automatikusan nem állít megfelelőséget.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test research_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P11` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `RES-12` implemented/tested állapotúnak.

## Task Z: Bekötés és átadás

- [ ] `python3 scripts/validate_development_packages.py` és `cargo +1.99.1 test -p latexide-core --test research_contract --locked` sikeres; a tesztparancs itt célállapot, futtatását külön rögzíteni kell.
- [ ] P00 alkalmazza a manifestben felsorolt közös hookokat; stagingben egyesével fusson a korábbi Rust/native/IPC/MCP regresszió a megváltozott rétegnek megfelelően. Nincs automatikus Save vagy új agent-edit jóváhagyás.
- [ ] Mac natív workflow valódi fókusz-/IME-/undo-eseménnyel; Windows külön MSBuild/link/launch/funkcionális kör. Linux saját runner/felület nélkül unrun. Nem natív csomagnál ezeket az adaptergazda végzi.
- [ ] `docs/development/results/P11.md` tartalmazza a teljesített ID-ket és reprodukálható bizonyítékot. P00 ennek alapján frissíti a handoffot, ledgert és a releváns skill-reference-et; más csomag ne írja párhuzamosan ezeket.

## Másolható indítófeladat

> Hajtsd végre a `docs/superpowers/plans/2026-10-31-parallel/p11-research.md` tervet. Előbb olvasd az AGENTS.md-t, a handoffot, a projekt-skillt, a közös csomagszerződést és a BASELINE_DELTA-t. Csak P11 saját útvonalait módosítsd, a közös fájlokhoz integration-manifestet adj. Őrizd meg a közvetlen agent-edit + explicit Save + Save után is elérhető undo szerződést. A követelményfüggőséget és platformi bizonyítékot ellenőrizd; planned/unrun állapotot ne nevezd késznek.
