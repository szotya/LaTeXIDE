# P20 — Kiadási minősítés, migráció és csomagolás Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reprodukálható, aláírt és migrációval kipróbált kiadási folyamatot hoz létre.

**Architecture:** Pontos candidate SHA és toolmanifest alapján külön build/test/sign/notarize/runtime kapuk. A support bundle redaktált, a frissítés hiba esetén visszaállítható; a mostani ad-hoc bundle nem production release.

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

- R1: Egy régebbi commit zöld bizonyítéka ne minősítsen újabb candidate-et.
- R2: Aláírás/notarizáció hibája ne legyen csendesen ad-hoc sikerre visszaminősítve.
- R3: Frissítés közbeni dirty munka és recovery-séma visszaállítható maradjon.
- R4: Support bundle csak redaktált adatot és a felhasználó által választott csatolmányt vigyen.
- R5: macOS minimumverzió és Windows x64 natív runtime a kiadási mátrixban külön igazolt legyen.

---

## Tulajdon és indulás

**Branch:** `feature/p20-release`. **Profil:** core.

**Kizárólagosan létrehozható/módosítható útvonalak:**

- `tools/release/`
- `tools/release/test_qualification.py`
- `integration/requests/P20.json`
- `docs/development/results/P20.md`

**Teszt:** `tools/release/test_qualification.py`. **Eredménynapló:** `docs/development/results/P20.md`.

**Olvasandó jelenlegi belépési pontok:** `scripts/build_macos.sh`, `scripts/build_windows.ps1`, `tools/toolchain-lock.json`.

Indulás: P00 contract-v1 után, fagyasztott fixture-ökkel és fake portokkal; a többi csomag teljes elkészülte nem indulási feltétel.

Integrációs szomszédok: P00, P01, P03, P08, P18, P21. Ezek **nem csomagszintű topologikus sorrend**: az egyedi követelményfüggőségek maradnak irányadók. A kölcsönös portfüggést fixture és fokozatos bekötés oldja, nem két teljes csomag egymásra várása.

## Task 1: Önállóan ellenőrizhető szerződésszelet

**Create:** `tools/release/qualify.py`, `tools/release/test_qualification.py`. **Consumes / produces:**

```rust
pub fn qualify_release(evidence:ReleaseEvidence) -> Result<ReleaseDecision,ReleaseError>;
// ReleaseEvidence { source_sha:String, required_gates:Vec<String>, passed_gates:Vec<String>, artifact_sha256:String }; ReleaseDecision { source_sha:String, artifact_sha256:String }; ReleaseError::{MissingGate,SourceMismatch,UnsignedArtifact,UnrunPlatform}
```

Az itt felsorolt DTO-mezőket P00 helyezi a `crates/latexide-contracts/src/` moduljaiba, serde serialize/deserialize és egyértelmű hiba-enummal. A szolgáltatás a saját modulban marad; ne hozz létre protocol→core→protocol függőségi kört. A port v1 helyi Rust-határ, nem automatikus új IPC-eszköz.

**Saját adapter belépési pontja:**

```python
def qualify_release(evidence: dict) -> dict: ...
```

A fenti Rust-alak az interoperábilis szerződést rögzíti; a saját adapter az itt megadott nyelven készül. Az első assertiont az adapter saját tesztkerete ellenőrzi.

- [ ] **RED:** `tools/release/test_qualification.py` alatt `contract_v1_first_slice` néven rögzítsd ezt az ellenőrzést: **Hiányzó Windows runtime gate vagy eltérő forrás-SHA esetén nincs ReleaseDecision, konkrét hiba van.**
- [ ] Futtasd: `python3 tools/release/test_qualification.py`. Először a hiányzó szolgáltatásra vagy állított viselkedésre FAIL legyen; környezet-/linkhibát ne könyvelj reprodukciónak.
- [ ] Valósítsd meg az első portot `tools/release/qualify.py` alatt; az alkalmazás-regisztrációt add át `integration/requests/P20.json` manifestben (schema, package_id, contract_version, requirement_ids, source_paths, hook_targets, test_commands, compatibility).
- [ ] Ugyanazzal a paranccsal legyen GREEN; tedd mellé R1–R5 célzott regresszióját a `review_focus_r1` … `review_focus_r5` tesztekbe. Az állítások a fenti öt elvárt viselkedést vizsgálják.
- [ ] Ellenőrizd a saját diffet, majd készíts kis commitot: `git add -- tools/release/qualify.py tools/release/test_qualification.py integration/requests/P20.json` és `git commit -m "feat(p20): establish tested service slice"`. A dokumentált későbbi munkavégzéskor commitolj; ez a terv most nem futtatja ezeket.

## Követelményenkénti feladatsor

Minden alábbi sor külön tesztelhető részeredmény és saját kis commit. A teszt neve az ID kisbetűs, aláhúzásos alakja; az elfogadás mondata a kötelező assertion. A dependency provider mockja csak izolált teszthez elég; release-hez a valódi providerrel is teljesüljön.

### QA-10 — Kiadási build és csomagminősítés (P0, L)

**Kiindulás:** Mac debug/ad-hoc csomag van; Swift linker debug Rust archívumot köt.

**Módosítás:** Valódi release Cargo+Swift profil, staging, provenance és install/upgrade/rollback teszt.

**Valódi bekötés előtt:** nincs.

- [ ] `qa_10` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Kiadott bináris valóban release, arm64, teljes erőforrásokkal; forrás/hash és profile összerendelt.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `python3 tools/release/test_qualification.py` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P20` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `QA-10` implemented/tested állapotúnak.

### QA-11 — Developer ID/notarization és Windows aláírás (P1, M)

**Kiindulás:** Helyi ad-hoc aláírás van; kiadási identitás és installer nincs kvalifikálva.

**Módosítás:** Kiadáshoz aláírás/notarization/installer és offline ellenőrizhető eredet, külön release műveletként.

**Valódi bekötés előtt:** QA-10 (P20), OS-01 (P21).

- [ ] `qa_11` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Tiszta támogatott gépen megnyílik/települ; tanúsítvány és Gatekeeper/Windows eredmény rögzített.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `python3 tools/release/test_qualification.py` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P20` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `QA-11` implemented/tested állapotúnak.

### QA-12 — Frissítés és adat-migráció (P1, M)

**Kiindulás:** Helyi rollback appmásolatok vannak; termékupdater nincs.

**Módosítás:** Aláírt update, atomikus csere, config/history migráció és rollback.

**Valódi bekötés előtt:** QA-10 (P20), DOC-01 (P01), WS-04 (P08).

- [ ] `qa_12` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Félbeszakadt frissítés után az előző vagy új app indul; dirty recovery kompatibilis.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `python3 tools/release/test_qualification.py` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P20` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `QA-12` implemented/tested állapotúnak.

### QA-13 — Diagnosztika és support bundle (P2, M)

**Kiindulás:** Buildlog és evidence létezik, felhasználói support export nincs.

**Módosítás:** Helyi opt-in redaktált diagnostic report verzióval, capabilityvel és hibakóddal.

**Valódi bekötés előtt:** SEC-06 (P18), MCP-03 (P03).

- [ ] `qa_13` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Reprodukálható hibaelemek szerepelnek, dokumentumtartalom/token alapból kimarad.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `python3 tools/release/test_qualification.py` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P20` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `QA-13` implemented/tested állapotúnak.

### QA-16 — Kiadás utáni regressziós és támogatási szabály (P1, M)

**Kiindulás:** Nincs késztermék-verziózási/támogatási ciklus.

**Módosítás:** Támogatott OS/toolchain/provider mátrix, release checklist és visszajelzésből reprodukció.

**Valódi bekötés előtt:** QA-10 (P20), QA-08 (P00).

- [ ] `qa_16` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Egy release állítása minden aktív platformon saját bizonyítékra mutat; visszavonható hibás kiadás.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `python3 tools/release/test_qualification.py` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P20` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `QA-16` implemented/tested állapotúnak.

### OS-06 — macOS 14+ és Apple Silicon mátrix (P1, M)

**Kiindulás:** A cél macOS 14+ arm64, szűk fejlesztőgépes bizonyítékkal.

**Módosítás:** Minimum és aktuális támogatott OS, M-sorozat, sandbox/permission és installer vizsgálat.

**Valódi bekötés előtt:** QA-10 (P20).

- [ ] `os_06` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Minden hirdetett OS saját install/launch/workflow eredményt kap; Intel nem új követelmény.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `python3 tools/release/test_qualification.py` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P20` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `OS-06` implemented/tested állapotúnak.

## Task Z: Bekötés és átadás

- [ ] `python3 scripts/validate_development_packages.py` és `python3 tools/release/test_qualification.py` sikeres; a tesztparancs itt célállapot, futtatását külön rögzíteni kell.
- [ ] P00 alkalmazza a manifestben felsorolt közös hookokat; stagingben egyesével fusson a korábbi Rust/native/IPC/MCP regresszió a megváltozott rétegnek megfelelően. Nincs automatikus Save vagy új agent-edit jóváhagyás.
- [ ] Mac natív workflow valódi fókusz-/IME-/undo-eseménnyel; Windows külön MSBuild/link/launch/funkcionális kör. Linux saját runner/felület nélkül unrun. Nem natív csomagnál ezeket az adaptergazda végzi.
- [ ] `docs/development/results/P20.md` tartalmazza a teljesített ID-ket és reprodukálható bizonyítékot. P00 ennek alapján frissíti a handoffot, ledgert és a releváns skill-reference-et; más csomag ne írja párhuzamosan ezeket.

## Másolható indítófeladat

> Hajtsd végre a `docs/superpowers/plans/2026-10-31-parallel/p20-release.md` tervet. Előbb olvasd az AGENTS.md-t, a handoffot, a projekt-skillt, a közös csomagszerződést és a BASELINE_DELTA-t. Csak P20 saját útvonalait módosítsd, a közös fájlokhoz integration-manifestet adj. Őrizd meg a közvetlen agent-edit + explicit Save + Save után is elérhető undo szerződést. A követelményfüggőséget és platformi bizonyítékot ellenőrizd; planned/unrun állapotot ne nevezd késznek.
