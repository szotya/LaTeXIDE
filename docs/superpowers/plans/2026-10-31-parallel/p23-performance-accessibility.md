# P23 — Teljesítmény, hozzáférhetőség és használhatóság Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Összehasonlítható teljesítmény- és valós billentyűzetes/képernyőolvasós workflow-bizonyítékot ad.

**Architecture:** Release/debug külön mérés; azonos fixture és workload a LaTeXIDE/JetBrains/VS Code összevetéséhez. Natív AX és UI skálázási mátrix, rögzített gép/OS/termékverzió és hiányzó adatokkal.

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

- R1: Warm/cold cache, debug/release és eltérő hardver ne kerüljön egy rangsorba.
- R2: Képernyőolvasó tényleges fókusz- és névbeolvasását ne helyettesítse attribútum-létezés.
- R3: Kis ablak/nagy betű/más DPI mellett minden fő workflow elérhető maradjon.
- R4: Hiányzó vagy timeout mérés NA legyen, ne nulla és ne gyorsasági előny.
- R5: Felhasználói munkafolyamat hibaaránya és adatvesztése fontosabb mint egyetlen mikrobenchmark átlaga.

---

## Tulajdon és indulás

**Branch:** `feature/p23-performance-accessibility`. **Profil:** core.

**Kizárólagosan létrehozható/módosítható útvonalak:**

- `tools/benchmarks/`
- `tools/benchmarks/test_report.py`
- `integration/requests/P23.json`
- `docs/development/results/P23.md`

**Teszt:** `tools/benchmarks/test_report.py`. **Eredménynapló:** `docs/development/results/P23.md`.

**Olvasandó jelenlegi belépési pontok:** `docs/IDE_AUDIT_2026-10-31.md`, `docs/IDE_PRODUCT_DIRECTION.md`, `apps/macos/Tests/LaTeXIDEKitTests`.

Indulás: már most elindítható környezet-/tesztfeltárással; a termékbekötéshez P00 contract-v1 kell.

Integrációs szomszédok: P11, P19. Ezek **nem csomagszintű topologikus sorrend**: az egyedi követelményfüggőségek maradnak irányadók. A kölcsönös portfüggést fixture és fokozatos bekötés oldja, nem két teljes csomag egymásra várása.

## Task 1: Önállóan ellenőrizhető szerződésszelet

**Create:** `tools/benchmarks/report.py`, `tools/benchmarks/test_report.py`. **Consumes / produces:**

```rust
pub fn summarize_samples(samples:&[TimingSample]) -> Result<TimingSummary,MeasurementError>;
// TimingSample { duration_us:u64, release:bool, fixture_sha:String }; TimingSummary { count:u32, median_us:u64, p95_us:u64 }; MeasurementError::{Empty,MixedBuildMode,MixedFixture}
```

Az itt felsorolt DTO-mezőket P00 helyezi a `crates/latexide-contracts/src/` moduljaiba, serde serialize/deserialize és egyértelmű hiba-enummal. A szolgáltatás a saját modulban marad; ne hozz létre protocol→core→protocol függőségi kört. A port v1 helyi Rust-határ, nem automatikus új IPC-eszköz.

**Saját adapter belépési pontja:**

```python
def summarize_samples(samples: list[dict]) -> dict: ...
```

A fenti Rust-alak az interoperábilis szerződést rögzíti; a saját adapter az itt megadott nyelven készül. Az első assertiont az adapter saját tesztkerete ellenőrzi.

- [ ] **RED:** `tools/benchmarks/test_report.py` alatt `contract_v1_first_slice` néven rögzítsd ezt az ellenőrzést: **Debug/release vagy eltérő fixture mintái MixedBuildMode/MixedFixture hibát adnak, nem közös szép átlagot.**
- [ ] Futtasd: `python3 tools/benchmarks/test_report.py`. Először a hiányzó szolgáltatásra vagy állított viselkedésre FAIL legyen; környezet-/linkhibát ne könyvelj reprodukciónak.
- [ ] Valósítsd meg az első portot `tools/benchmarks/report.py` alatt; az alkalmazás-regisztrációt add át `integration/requests/P23.json` manifestben (schema, package_id, contract_version, requirement_ids, source_paths, hook_targets, test_commands, compatibility).
- [ ] Ugyanazzal a paranccsal legyen GREEN; tedd mellé R1–R5 célzott regresszióját a `review_focus_r1` … `review_focus_r5` tesztekbe. Az állítások a fenti öt elvárt viselkedést vizsgálják.
- [ ] Ellenőrizd a saját diffet, majd készíts kis commitot: `git add -- tools/benchmarks/report.py tools/benchmarks/test_report.py integration/requests/P23.json` és `git commit -m "feat(p23): establish tested service slice"`. A dokumentált későbbi munkavégzéskor commitolj; ez a terv most nem futtatja ezeket.

## Követelményenkénti feladatsor

Minden alábbi sor külön tesztelhető részeredmény és saját kis commit. A teszt neve az ID kisbetűs, aláhúzásos alakja; az elfogadás mondata a kötelező assertion. A dependency provider mockja csak izolált teszthez elég; release-hez a valódi providerrel is teljesüljön.

### QA-03 — Input/RSS/CPU/lemez baseline (P1, M)

**Kiindulás:** Van szűk debug lexer/index mérés és régi 170 MiB snapshot.

**Módosítás:** Release benchmark cold/warm/startup/typing/scroll/PDF/build/agent és processzfánkénti erőforrásmérés.

**Valódi bekötés előtt:** nincs.

- [ ] `qa_03` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Nyers adatok, hardver/corpus/verzió és percentilisek rögzítettek; regressziós budget meghatározott.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `python3 tools/benchmarks/test_report.py` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P23` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `QA-03` implemented/tested állapotúnak.

### QA-04 — Összehasonlítható versenytársbenchmark (P1, M)

**Kiindulás:** JetBrains/VS Code futás nincs mérve ebben a felmérésben.

**Módosítás:** Azonos projekt, plugin, engine, cache és hardver kontrollált benchmarkprotokoll.

**Valódi bekötés előtt:** QA-03 (P23), QA-02 (P19).

- [ ] `qa_04` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Külön startup/idle/edit/build és összes child RSS; nincs nyelvválasztásból levezetett sebességállítás.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `python3 tools/benchmarks/test_report.py` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P23` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `QA-04` implemented/tested állapotúnak.

### QA-06 — Képernyőolvasó és billentyűzet kvalifikáció (P1, L)

**Kiindulás:** Kontraszt és szűk fókusz/shortcut tesztek PASS; teljes AX audit nincs.

**Módosítás:** VoiceOver/Narrator és később Orca, keyboard-only, HU/US/CJK és nagyítási tesztforgatókönyv.

**Valódi bekötés előtt:** nincs.

- [ ] `qa_06` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Alapworkflow egér nélkül végigvihető; completion és agentállapot felolvasható.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `python3 tools/benchmarks/test_report.py` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P23` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `QA-06` implemented/tested állapotúnak.

### QA-07 — Reszponzív natív vizuális mátrix (P1, M)

**Kiindulás:** Kontrollált render néhány megjelenésen, szűk live bizonyíték van.

**Módosítás:** Kis/nagy ablak, light/dark/contrast, OS-theme change, DPI/monitor, nagy betű és lokalizáció.

**Valódi bekötés előtt:** nincs.

- [ ] `qa_07` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Nincs clipping vagy elérhetetlen művelet; live és off-screen render bizonyíték külön jelölt.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `python3 tools/benchmarks/test_report.py` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P23` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `QA-07` implemented/tested állapotúnak.

### QA-15 — Felhasználói workflow és usability mérés (P2, M)

**Kiindulás:** Technikai fixture-k vannak; valós hallgatói feladatmérés nincs.

**Módosítás:** Megfigyelt új projekt→írás→hiba→bib→PDF→agent→undo→beadás munkafolyamat.

**Valódi bekötés előtt:** RES-02 (P11), RES-08 (P11).

- [ ] `qa_15` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Feladatvégzés, hibák és elakadások mértek; konkrét UX javításokhoz rendeltek.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `python3 tools/benchmarks/test_report.py` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P23` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `QA-15` implemented/tested állapotúnak.

## Task Z: Bekötés és átadás

- [ ] `python3 scripts/validate_development_packages.py` és `python3 tools/benchmarks/test_report.py` sikeres; a tesztparancs itt célállapot, futtatását külön rögzíteni kell.
- [ ] P00 alkalmazza a manifestben felsorolt közös hookokat; stagingben egyesével fusson a korábbi Rust/native/IPC/MCP regresszió a megváltozott rétegnek megfelelően. Nincs automatikus Save vagy új agent-edit jóváhagyás.
- [ ] Mac natív workflow valódi fókusz-/IME-/undo-eseménnyel; Windows külön MSBuild/link/launch/funkcionális kör. Linux saját runner/felület nélkül unrun. Nem natív csomagnál ezeket az adaptergazda végzi.
- [ ] `docs/development/results/P23.md` tartalmazza a teljesített ID-ket és reprodukálható bizonyítékot. P00 ennek alapján frissíti a handoffot, ledgert és a releváns skill-reference-et; más csomag ne írja párhuzamosan ezeket.

## Másolható indítófeladat

> Hajtsd végre a `docs/superpowers/plans/2026-10-31-parallel/p23-performance-accessibility.md` tervet. Előbb olvasd az AGENTS.md-t, a handoffot, a projekt-skillt, a közös csomagszerződést és a BASELINE_DELTA-t. Csak P23 saját útvonalait módosítsd, a közös fájlokhoz integration-manifestet adj. Őrizd meg a közvetlen agent-edit + explicit Save + Save után is elérhető undo szerződést. A követelményfüggőséget és platformi bizonyítékot ellenőrizd; planned/unrun állapotot ne nevezd késznek.
