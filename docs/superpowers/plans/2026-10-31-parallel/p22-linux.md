# P22 — Linux platformdöntés és natív port Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bizonyíték alapján választ Linux UI-t és külön bootstrap/runtime/desktop workflow-t épít.

**Architecture:** Döntési spike GTK/Qt és csomagolási következményekkel, majd XDG-runtime/Unix IPC és compilerport. Nincs előre késznek nevezett Linux-felület; a Mac /private/tmp konvenciója nem kerül át.

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

- R1: Hiányzó vagy nem biztonságos XDG_RUNTIME_DIR explicit hibát és dokumentált megoldást adjon.
- R2: Wayland és X11 fókusz/IME/PDF viselkedés külön igazolandó.
- R3: Választott UI toolkit licenc/csomagméret/RAM döntése mérésen alapuljon.
- R4: Sandbox és process-tree cancel Linuxon külön működjön.
- R5: Linux Rust unit PASS ne legyen natív desktop támogatásként kommunikálva.

---

## Tulajdon és indulás

**Branch:** `feature/p22-linux`. **Profil:** core.

**Kizárólagosan létrehozható/módosítható útvonalak:**

- `apps/linux/platform_probe/`
- `tests/linux_native/test_runtime_dir.py`
- `integration/requests/P22.json`
- `docs/development/results/P22.md`
- `apps/linux/platform_probe/Cargo.toml`

**Teszt:** `tests/linux_native/test_runtime_dir.py`. **Eredménynapló:** `docs/development/results/P22.md`.

**Olvasandó jelenlegi belépési pontok:** `crates/latexide-ffi/src/session.rs`, `skills/latexide-platform-development/references/linux.md`.

Indulás: P00 contract-v1 után, fagyasztott fixture-ökkel és fake portokkal; a többi csomag teljes elkészülte nem indulási feltétel.

Integrációs szomszédok: P03, P15. Ezek **nem csomagszintű topologikus sorrend**: az egyedi követelményfüggőségek maradnak irányadók. A kölcsönös portfüggést fixture és fokozatos bekötés oldja, nem két teljes csomag egymásra várása.

## Task 1: Önállóan ellenőrizhető szerződésszelet

**Create:** `apps/linux/platform_probe/main.rs`, `tests/linux_native/test_runtime_dir.py`. **Consumes / produces:**

```rust
pub fn validate_runtime_directory(path:String, owner_uid:u32, mode:u32, current_uid:u32) -> Result<String,PlatformError>;
// PlatformError::{WrongOwner,UnsafePermissions,MissingRuntimeDirectory,UnsupportedDesktop}
```

Az itt felsorolt DTO-mezőket P00 helyezi a `crates/latexide-contracts/src/` moduljaiba, serde serialize/deserialize és egyértelmű hiba-enummal. A szolgáltatás a saját modulban marad; ne hozz létre protocol→core→protocol függőségi kört. A port v1 helyi Rust-határ, nem automatikus új IPC-eszköz.

**Saját adapter belépési pontja:**

```rust
pub fn validate_runtime_directory(path:String, owner_uid:u32, mode:u32, current_uid:u32) -> Result<String,PlatformError>;
// Python runtime-dir test launches the compiled probe in a disposable directory.
```

A fenti Rust-alak az interoperábilis szerződést rögzíti; a saját adapter az itt megadott nyelven készül. Az első assertiont az adapter saját tesztkerete ellenőrzi.

- [ ] **RED:** `tests/linux_native/test_runtime_dir.py` alatt `contract_v1_first_slice` néven rögzítsd ezt az ellenőrzést: **Más tulajdonosú vagy group/world elérhető runtime-dir elutasított; nincs automatikus /private/tmp fallback.**
- [ ] Futtasd: `python3 tests/linux_native/test_runtime_dir.py`. Először a hiányzó szolgáltatásra vagy állított viselkedésre FAIL legyen; környezet-/linkhibát ne könyvelj reprodukciónak.
- [ ] Valósítsd meg az első portot `apps/linux/platform_probe/main.rs` alatt; az alkalmazás-regisztrációt add át `integration/requests/P22.json` manifestben (schema, package_id, contract_version, requirement_ids, source_paths, hook_targets, test_commands, compatibility).
- [ ] Ugyanazzal a paranccsal legyen GREEN; tedd mellé R1–R5 célzott regresszióját a `review_focus_r1` … `review_focus_r5` tesztekbe. Az állítások a fenti öt elvárt viselkedést vizsgálják.
- [ ] Ellenőrizd a saját diffet, majd készíts kis commitot: `git add -- apps/linux/platform_probe/main.rs tests/linux_native/test_runtime_dir.py integration/requests/P22.json` és `git commit -m "feat(p22): establish tested service slice"`. A dokumentált későbbi munkavégzéskor commitolj; ez a terv most nem futtatja ezeket.

## Követelményenkénti feladatsor

Minden alábbi sor külön tesztelhető részeredmény és saját kis commit. A teszt neve az ID kisbetűs, aláhúzásos alakja; az elfogadás mondata a kötelező assertion. A dependency provider mockja csak izolált teszthez elég; release-hez a valódi providerrel is teljesüljön.

### OS-04 — Linux platformdöntés és bootstrap (P3, L)

**Kiindulás:** Nincs natív UI, compiler/runtime kvalifikáció; /private/tmp minden Unixra érvényes.

**Módosítás:** Támogatott disztribúció/architektúra és GTK/Qt döntés; XDG runtime és verified Linux bootstrap.

**Valódi bekötés előtt:** MCP-03 (P03).

- [ ] `os_04` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Linuxon saját Rust/link/IPC/build bizonyíték; Mac payload/útvonal nincs újrahasználva.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `python3 tests/linux_native/test_runtime_dir.py` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P22` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `OS-04` implemented/tested állapotúnak.

### OS-05 — Linux natív workspace (P3, XL)

**Kiindulás:** Nincs Linux frontend.

**Módosítás:** A kiválasztott toolkit natív editor/PDF/clipboard/IME/AX/agent adapterei.

**Valódi bekötés előtt:** OS-04 (P22), EXT-08 (P15).

- [ ] `os_05` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Teljes saját edit→agent→undo→save→build→PDF workflow a kijelölt Linuxon.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `python3 tests/linux_native/test_runtime_dir.py` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P22` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `OS-05` implemented/tested állapotúnak.

## Task Z: Bekötés és átadás

- [ ] `python3 scripts/validate_development_packages.py` és `python3 tests/linux_native/test_runtime_dir.py` sikeres; a tesztparancs itt célállapot, futtatását külön rögzíteni kell.
- [ ] P00 alkalmazza a manifestben felsorolt közös hookokat; stagingben egyesével fusson a korábbi Rust/native/IPC/MCP regresszió a megváltozott rétegnek megfelelően. Nincs automatikus Save vagy új agent-edit jóváhagyás.
- [ ] Mac natív workflow valódi fókusz-/IME-/undo-eseménnyel; Windows külön MSBuild/link/launch/funkcionális kör. Linux saját runner/felület nélkül unrun. Nem natív csomagnál ezeket az adaptergazda végzi.
- [ ] `docs/development/results/P22.md` tartalmazza a teljesített ID-ket és reprodukálható bizonyítékot. P00 ennek alapján frissíti a handoffot, ledgert és a releváns skill-reference-et; más csomag ne írja párhuzamosan ezeket.

## Másolható indítófeladat

> Hajtsd végre a `docs/superpowers/plans/2026-10-31-parallel/p22-linux.md` tervet. Előbb olvasd az AGENTS.md-t, a handoffot, a projekt-skillt, a közös csomagszerződést és a BASELINE_DELTA-t. Csak P22 saját útvonalait módosítsd, a közös fájlokhoz integration-manifestet adj. Őrizd meg a közvetlen agent-edit + explicit Save + Save után is elérhető undo szerződést. A követelményfüggőséget és platformi bizonyítékot ellenőrizd; planned/unrun állapotot ne nevezd késznek.
