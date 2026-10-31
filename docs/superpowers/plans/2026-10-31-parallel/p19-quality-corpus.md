# P19 — Fault injection, korpusz és protokollminőség Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Determinista hiba-/kiesési tesztekkel és reprezentatív LaTeX-korpusszal bizonyítja az adat- és protokollszerződéseket.

**Architecture:** Saját tesztprojektek és injektálható faultpontok, rögzített seedekkel. Tesztfuttató nem piszkálja a felhasználó élő appját vagy projektjét; minden eredmény a pontos forrás-SHA-hoz kötött.

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

- R1: Reprodukálható seed nélkül bukó fuzz eredmény nem lezárt regressziós teszt.
- R2: Kill/power-loss szimuláció után a valódi lemezállapotot és nyugtázott revíziót is ellenőrizni kell.
- R3: Korpuszhibát ne rejtse el automatikus fixture-átírás vagy baseline-frissítés.
- R4: Hosszú user/agent kevert session minden stale/error/undo átmenete naplózott legyen.
- R5: Teszt timeout megszakított eredmény, nem PASS vagy üres siker.

---

## Tulajdon és indulás

**Branch:** `feature/p19-quality-corpus`. **Profil:** core.

**Kizárólagosan létrehozható/módosítható útvonalak:**

- `tests/quality_corpus/`
- `tests/quality_corpus/test_harness.py`
- `integration/requests/P19.json`
- `docs/development/results/P19.md`

**Teszt:** `tests/quality_corpus/test_harness.py`. **Eredménynapló:** `docs/development/results/P19.md`.

**Olvasandó jelenlegi belépési pontok:** `tests/ipc_contract.py`, `tests/compiler_runtime_contract.py`, `evidence/ide-audit-2026-10-31/audit_probes.py`.

Indulás: már most elindítható környezet-/tesztfeltárással; a termékbekötéshez P00 contract-v1 kell.

Integrációs szomszédok: P01, P03. Ezek **nem csomagszintű topologikus sorrend**: az egyedi követelményfüggőségek maradnak irányadók. A kölcsönös portfüggést fixture és fokozatos bekötés oldja, nem két teljes csomag egymásra várása.

## Task 1: Önállóan ellenőrizhető szerződésszelet

**Create:** `tests/quality_corpus/run.py`, `tests/quality_corpus/test_harness.py`. **Consumes / produces:**

```rust
pub fn expected_fault_outcome(point:FaultPoint, acknowledged_revision:u64) -> FaultExpectation;
// FaultPoint::{BeforeJournalSync,AfterJournalSync,BeforeRename,AfterRename,MidFrame}; FaultExpectation { minimum_recovered_revision:u64, disk_may_change:bool, partial_edit_allowed:bool }
```

Az itt felsorolt DTO-mezőket P00 helyezi a `crates/latexide-contracts/src/` moduljaiba, serde serialize/deserialize és egyértelmű hiba-enummal. A szolgáltatás a saját modulban marad; ne hozz létre protocol→core→protocol függőségi kört. A port v1 helyi Rust-határ, nem automatikus új IPC-eszköz.

**Saját adapter belépési pontja:**

```python
def expected_fault_outcome(point: str, acknowledged_revision: int) -> dict: ...
def run_fault_case(case: dict, executable: Path) -> dict: ...
```

A fenti Rust-alak az interoperábilis szerződést rögzíti; a saját adapter az itt megadott nyelven készül. Az első assertiont az adapter saját tesztkerete ellenőrzi.

- [ ] **RED:** `tests/quality_corpus/test_harness.py` alatt `contract_v1_first_slice` néven rögzítsd ezt az ellenőrzést: **AfterJournalSync kiesés után legalább a nyugtázott revízió visszajön, részleges többfájlos edit soha nem elfogadott.**
- [ ] Futtasd: `python3 tests/quality_corpus/test_harness.py`. Először a hiányzó szolgáltatásra vagy állított viselkedésre FAIL legyen; környezet-/linkhibát ne könyvelj reprodukciónak.
- [ ] Valósítsd meg az első portot `tests/quality_corpus/run.py` alatt; az alkalmazás-regisztrációt add át `integration/requests/P19.json` manifestben (schema, package_id, contract_version, requirement_ids, source_paths, hook_targets, test_commands, compatibility).
- [ ] Ugyanazzal a paranccsal legyen GREEN; tedd mellé R1–R5 célzott regresszióját a `review_focus_r1` … `review_focus_r5` tesztekbe. Az állítások a fenti öt elvárt viselkedést vizsgálják.
- [ ] Ellenőrizd a saját diffet, majd készíts kis commitot: `git add -- tests/quality_corpus/run.py tests/quality_corpus/test_harness.py integration/requests/P19.json` és `git commit -m "feat(p19): establish tested service slice"`. A dokumentált későbbi munkavégzéskor commitolj; ez a terv most nem futtatja ezeket.

## Követelményenkénti feladatsor

Minden alábbi sor külön tesztelhető részeredmény és saját kis commit. A teszt neve az ID kisbetűs, aláhúzásos alakja; az elfogadás mondata a kötelező assertion. A dependency provider mockja csak izolált teszthez elég; release-hez a valódi providerrel is teljesüljön.

### QA-01 — Crash/fault injection és recovery bizonyíték (P0, L)

**Kiindulás:** Normál szerződéstesztek vannak; széles crash-korpusz nincs.

**Módosítás:** Saját fixture-ön kill/fault/disk-full/power-loss modell save/edit/build/index pontokon.

**Valódi bekötés előtt:** DOC-01 (P01).

- [ ] `qa_01` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Utolsó nyugtázott edit recoveryből megvan; a lemezen mindig teljes, azonosított változat marad.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `python3 tests/quality_corpus/test_harness.py` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P19` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `QA-01` implemented/tested állapotúnak.

### QA-02 — Reprezentatív LaTeX kompatibilitási korpusz (P1, L)

**Kiindulás:** Kis basic/multifile minták ellenőrzöttek.

**Módosítás:** Article/book/report/beamer/thesis, BibTeX/Biber, fontspec, tikz és problémás class korpusz.

**Valódi bekötés előtt:** nincs.

- [ ] `qa_02` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Minden támogatott recept/platform saját PASS/FAIL/unrun táblát kap, publikálható reprodukcióval.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `python3 tests/quality_corpus/test_harness.py` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P19` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `QA-02` implemented/tested állapotúnak.

### QA-05 — Hosszú vegyes user/agent session (P1, M)

**Kiindulás:** Kis own-fixture read/edit/undo/build kör létezik.

**Módosítás:** Többórás scripted soak, száz fájl és ismételt panel/projekt/build váltások.

**Valódi bekötés előtt:** DOC-02 (P01), MCP-02 (P03).

- [ ] `qa_05` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Nincs növekvő leak, elveszett draft vagy hibás revízió; minden hiba reprodukálható nyomot ad.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `python3 tests/quality_corpus/test_harness.py` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P19` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `QA-05` implemented/tested állapotúnak.

### QA-09 — Fuzz és property szerződések (P1, M)

**Kiindulás:** Célzott Unicode/parser/batch regressziók vannak.

**Módosítás:** Edit/undo/position/JSON/parser fuzz, seedelt property teszt és korlátos hibabudget.

**Valódi bekötés előtt:** nincs.

- [ ] `qa_09` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Panic/hang/minimal reproducer követhető; érvénytelen input után a mag használható.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `python3 tests/quality_corpus/test_harness.py` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P19` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `QA-09` implemented/tested állapotúnak.

## Task Z: Bekötés és átadás

- [ ] `python3 scripts/validate_development_packages.py` és `python3 tests/quality_corpus/test_harness.py` sikeres; a tesztparancs itt célállapot, futtatását külön rögzíteni kell.
- [ ] P00 alkalmazza a manifestben felsorolt közös hookokat; stagingben egyesével fusson a korábbi Rust/native/IPC/MCP regresszió a megváltozott rétegnek megfelelően. Nincs automatikus Save vagy új agent-edit jóváhagyás.
- [ ] Mac natív workflow valódi fókusz-/IME-/undo-eseménnyel; Windows külön MSBuild/link/launch/funkcionális kör. Linux saját runner/felület nélkül unrun. Nem natív csomagnál ezeket az adaptergazda végzi.
- [ ] `docs/development/results/P19.md` tartalmazza a teljesített ID-ket és reprodukálható bizonyítékot. P00 ennek alapján frissíti a handoffot, ledgert és a releváns skill-reference-et; más csomag ne írja párhuzamosan ezeket.

## Másolható indítófeladat

> Hajtsd végre a `docs/superpowers/plans/2026-10-31-parallel/p19-quality-corpus.md` tervet. Előbb olvasd az AGENTS.md-t, a handoffot, a projekt-skillt, a közös csomagszerződést és a BASELINE_DELTA-t. Csak P19 saját útvonalait módosítsd, a közös fájlokhoz integration-manifestet adj. Őrizd meg a közvetlen agent-edit + explicit Save + Save után is elérhető undo szerződést. A követelményfüggőséget és platformi bizonyítékot ellenőrizd; planned/unrun állapotot ne nevezd késznek.
