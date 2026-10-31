# P21 — Windows 11 x64 natív minősítés és paritás Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Valódi Windows build/link/indítás és Intel/AMD workflow-eredményt ad, majd pótolja a natív paritást.

**Architecture:** Először a jelenlegi WinUI forrás tiszta Windows gépen fordul; csak utána képességport a közös portokra. A Rust MSVC check kiinduló bizonyíték, nem natív appfuttatás.

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

- R1: Windows natív dependency restore/link hibáját ne helyettesítsük Mac cross-checkkel.
- R2: ConPTY/Job Object/ACL valós Windows viselkedés külön tesztet kapjon.
- R3: Intel és AMD x64 gép eredménye külön scope és hardware-metaadat legyen.
- R4: IME, Ctrl-click, fókusz, mentés/undo és PDF-szöveg működése natív eseménnyel igazolt legyen.
- R5: Szóközös/Unicode/hosszú/UNC útvonal ne okozzon adatvesztést vagy hangtalan hibát.

---

## Tulajdon és indulás

**Branch:** `feature/p21-windows`. **Profil:** core.

**Kizárólagosan létrehozható/módosítható útvonalak:**

- `apps/windows/Features/PlatformParity/`
- `tests/windows_native/qualification.ps1`
- `integration/requests/P21.json`
- `docs/development/results/P21.md`

**Teszt:** `tests/windows_native/qualification.ps1`. **Eredménynapló:** `docs/development/results/P21.md`.

**Olvasandó jelenlegi belépési pontok:** `apps/windows/LaTeXIDE.vcxproj`, `scripts/build_windows.ps1`, `docs/windows-validation.md`.

Indulás: már most elindítható környezet-/tesztfeltárással; a termékbekötéshez P00 contract-v1 kell.

Integrációs szomszédok: P08, P10, P14, P23. Ezek **nem csomagszintű topologikus sorrend**: az egyedi követelményfüggőségek maradnak irányadók. A kölcsönös portfüggést fixture és fokozatos bekötés oldja, nem két teljes csomag egymásra várása.

## Task 1: Önállóan ellenőrizhető szerződésszelet

**Create:** `apps/windows/Features/PlatformParity/Qualification.cpp`, `tests/windows_native/qualification.ps1`. **Consumes / produces:**

```rust
pub fn evaluate_windows_evidence(e:WindowsEvidence) -> Result<WindowsQualification,PlatformError>;
// WindowsEvidence { source_sha:String, msbuild:bool, linked:bool, launched:bool, workflow_passed:bool, cpu_vendor:String }; WindowsQualification { source_sha:String, cpu_vendor:String }; PlatformError::{BuildFailed,LinkFailed,LaunchFailed,UnrunWorkflow,UnsupportedArchitecture}
```

Az itt felsorolt DTO-mezőket P00 helyezi a `crates/latexide-contracts/src/` moduljaiba, serde serialize/deserialize és egyértelmű hiba-enummal. A szolgáltatás a saját modulban marad; ne hozz létre protocol→core→protocol függőségi kört. A port v1 helyi Rust-határ, nem automatikus új IPC-eszköz.

**Saját adapter belépési pontja:**

```powershell
function Test-WindowsQualification { param([hashtable]$Evidence) }
# Qualification.cpp: WindowsQualification evaluate_windows_evidence(const WindowsEvidence& evidence);
```

A fenti Rust-alak az interoperábilis szerződést rögzíti; a saját adapter az itt megadott nyelven készül. Az első assertiont az adapter saját tesztkerete ellenőrzi.

- [ ] **RED:** `tests/windows_native/qualification.ps1` alatt `contract_v1_first_slice` néven rögzítsd ezt az ellenőrzést: **msbuild=true, linked=true, launched=true, workflow_passed=false esetén UnrunWorkflow; csak MSVC check nem elegendő natív minősítéshez.**
- [ ] Futtasd: `pwsh -File tests/windows_native/qualification.ps1`. Először a hiányzó szolgáltatásra vagy állított viselkedésre FAIL legyen; környezet-/linkhibát ne könyvelj reprodukciónak.
- [ ] Valósítsd meg az első portot `apps/windows/Features/PlatformParity/Qualification.cpp` alatt; az alkalmazás-regisztrációt add át `integration/requests/P21.json` manifestben (schema, package_id, contract_version, requirement_ids, source_paths, hook_targets, test_commands, compatibility).
- [ ] Ugyanazzal a paranccsal legyen GREEN; tedd mellé R1–R5 célzott regresszióját a `review_focus_r1` … `review_focus_r5` tesztekbe. Az állítások a fenti öt elvárt viselkedést vizsgálják.
- [ ] Ellenőrizd a saját diffet, majd készíts kis commitot: `git add -- apps/windows/Features/PlatformParity/Qualification.cpp tests/windows_native/qualification.ps1 integration/requests/P21.json` és `git commit -m "feat(p21): establish tested service slice"`. A dokumentált későbbi munkavégzéskor commitolj; ez a terv most nem futtatja ezeket.

## Követelményenkénti feladatsor

Minden alábbi sor külön tesztelhető részeredmény és saját kis commit. A teszt neve az ID kisbetűs, aláhúzásos alakja; az elfogadás mondata a kötelező assertion. A dependency provider mockja csak izolált teszthez elég; release-hez a valódi providerrel is teljesüljön.

### OS-01 — Tényleges Windows build és futás (P0, L)

**Kiindulás:** WinUI forrás, XML/C++ helper és Rust cross-check; MSBuild/link/runtime unrun.

**Módosítás:** Windows 11 x64 runner/gép és teljes restore/build/launch/workflow kvalifikáció.

**Valódi bekötés előtt:** nincs.

- [ ] `os_01` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Valódi x64 appon edit/save/IME/agent undo/build/PDF ciklus, nem Mac cross-check.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `pwsh -File tests/windows_native/qualification.ps1` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P21` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `OS-01` implemented/tested állapotúnak.

### OS-02 — Windows completion és natív asszisztens (P1, L)

**Kiindulás:** Közös Rust API van; a popup/Codex panel Mac implementáció.

**Módosítás:** WinUI popup, logikai-key/fókusz, providerprocess és draft/session adapter.

**Valódi bekötés előtt:** OS-01 (P21), AI-02 (P14).

- [ ] `os_02` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Saját makró/ref/cite, stale fallback, prompt undo és stop tényleges Windows gépen PASS.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `pwsh -File tests/windows_native/qualification.ps1` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P21` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `OS-02` implemented/tested állapotúnak.

### OS-03 — Windows UI- és PDF-paritás (P1, M)

**Kiindulás:** Quick Open/commands/PDF forrásban szűkebb; panelek perzisztenciája eltérhet.

**Módosítás:** Közös command/settings/layout szerződés és natív kereshető PDF képesség.

**Valódi bekötés előtt:** OS-01 (P21), WS-04 (P08), PDF-08 (P10).

- [ ] `os_03` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Funkciómátrix szerint mérhető paritás; eltérő funkció unavailable, nem megtévesztő ikon.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `pwsh -File tests/windows_native/qualification.ps1` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P21` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `OS-03` implemented/tested állapotúnak.

### OS-09 — Intel/AMD Windows és eszközmátrix (P1, M)

**Kiindulás:** Windows cél x64 Intel/AMD, célgépes adat nincs.

**Módosítás:** Legalább reprezentatív Intel/AMD, DPI/IME/PDF/process-tree és accessibility próba.

**Valódi bekötés előtt:** OS-01 (P21), QA-06 (P23), QA-07 (P23).

- [ ] `os_09` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Architecture/driver/OS eredmények külön rögzítettek; Windows ARM64 és x86 nem kerül támogatottnak.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `pwsh -File tests/windows_native/qualification.ps1` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P21` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `OS-09` implemented/tested állapotúnak.

## Task Z: Bekötés és átadás

- [ ] `python3 scripts/validate_development_packages.py` és `pwsh -File tests/windows_native/qualification.ps1` sikeres; a tesztparancs itt célállapot, futtatását külön rögzíteni kell.
- [ ] P00 alkalmazza a manifestben felsorolt közös hookokat; stagingben egyesével fusson a korábbi Rust/native/IPC/MCP regresszió a megváltozott rétegnek megfelelően. Nincs automatikus Save vagy új agent-edit jóváhagyás.
- [ ] Mac natív workflow valódi fókusz-/IME-/undo-eseménnyel; Windows külön MSBuild/link/launch/funkcionális kör. Linux saját runner/felület nélkül unrun. Nem natív csomagnál ezeket az adaptergazda végzi.
- [ ] `docs/development/results/P21.md` tartalmazza a teljesített ID-ket és reprodukálható bizonyítékot. P00 ennek alapján frissíti a handoffot, ledgert és a releváns skill-reference-et; más csomag ne írja párhuzamosan ezeket.

## Másolható indítófeladat

> Hajtsd végre a `docs/superpowers/plans/2026-10-31-parallel/p21-windows.md` tervet. Előbb olvasd az AGENTS.md-t, a handoffot, a projekt-skillt, a közös csomagszerződést és a BASELINE_DELTA-t. Csak P21 saját útvonalait módosítsd, a közös fájlokhoz integration-manifestet adj. Őrizd meg a közvetlen agent-edit + explicit Save + Save után is elérhető undo szerződést. A követelményfüggőséget és platformi bizonyítékot ellenőrizd; planned/unrun állapotot ne nevezd késznek.
