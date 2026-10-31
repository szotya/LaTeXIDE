# U02 — Python / PyCharm / DataSpell Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Külön választható technológiai programot épít: venv/conda, Python typing/debug/pytest, notebook kernel és dataframe inspect.

**Architecture:** P15 plugin SDK és a közös nyelvi/task/debug/test portok fölött izolált adapter. Első kapu saját capability-mátrix és rögzített toolkorpusz, utána slice-onkénti implementáció; ez nem automatikus vállalás teljes JetBrains-termékklónra.

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

- R1: A Python / PyCharm / DataSpell eszközverzió és projektmodell eltérése explicit kompatibilitási eredményt adjon.
- R2: Nem megbízható projekt ne indítson automatikusan scriptet, plugint, deploymentet vagy csomagtelepítést.
- R3: Stale dokumentumra érkező provider edit csak a közös revíziózott tranzakción keresztül alkalmazható.
- R4: Provider/környezet kiesése közben dirty munka, fókusz és visszavonás maradjon meg.
- R5: A teljes vertikumparitás csak tételes capability-mátrix és tényleges platformkorpusz után állítható; a smoke adapter nem elég.

---

## Tulajdon és indulás

**Branch:** `feature/u02-python-data`. **Profil:** optional.

**Kizárólagosan létrehozható/módosítható útvonalak:**

- `extensions/verticals/u02-python-data/`
- `integration/requests/U02.json`
- `docs/development/results/U02.md`

**Teszt:** `extensions/verticals/u02-python-data/tests/capability_contract.rs`. **Eredménynapló:** `docs/development/results/U02.md`.

**Olvasandó jelenlegi belépési pontok:** `docs/development/contracts`, `docs/ide-audit-2026-10-31/comparison.json`, `docs/ide-audit-2026-10-31/sources.json`.

Indulás: P00 contract-v1 után, fagyasztott fixture-ökkel és fake portokkal; a többi csomag teljes elkészülte nem indulási feltétel.

Integrációs szomszédok: P15, P17. Ezek **nem csomagszintű topologikus sorrend**: az egyedi követelményfüggőségek maradnak irányadók. A kölcsönös portfüggést fixture és fokozatos bekötés oldja, nem két teljes csomag egymásra várása.

## Task 0: Vertikumszerződés és önálló tesztcsomag

- [ ] Hozd létre `extensions/verticals/u02-python-data/CAPABILITIES.md` fájlt a fenti Goal technológiai területeire: capability-ID, projektprofil, toolverzió, natív platform, elvárt workflow és teszt. A három állapot: required/optional/unsupported; unsupported tétel mellett nincs teljes-paritás állítás.
- [ ] `extensions/verticals/u02-python-data/Cargo.toml`: package `latexide-u02-python-data`, version `0.1.0`, edition `2024`, rust-version `1.99`, külön üres `[workspace]` és `latexide-contracts = { path = "../../../crates/latexide-contracts" }` dependency. Így külön tesztelhető, nem módosít közös workspace-t.
- [ ] P00 SDK-v1 után `cargo +1.99.1 generate-lockfile --manifest-path extensions/verticals/u02-python-data/Cargo.toml`; commitold a saját lockfile-t, majd a terv további `--locked` parancsait használd.
- [ ] Legalább két eltérő projekt-/toolverzióprofilra rögzíts kisméretű, licencelt fixture-t; a provider hiányát MissingCapability/UnrunPlatform állítás vizsgálja. Ez a smoke-kapu még nem a teljes vertikum.

## Task 1: Önállóan ellenőrizhető szerződésszelet

**Create:** `extensions/verticals/u02-python-data/src/lib.rs`, `extensions/verticals/u02-python-data/tests/capability_contract.rs`. **Consumes / produces:**

```rust
pub fn qualify_vertical(manifest:VerticalManifest, results:Vec<CapabilityResult>) -> Result<VerticalQualification,VerticalError>;
// VerticalManifest { id:String, sdk_major:u32, tool_versions:Vec<String>, required_capabilities:Vec<String> }; CapabilityResult { capability:String, source_sha:String, platform:String, passed:bool }; VerticalQualification { id:String, verified_capabilities:Vec<String> }; VerticalError::{IncompatibleSdk,MissingCapability,UnrunPlatform,SourceMismatch}
// Scope: venv/conda, Python typing/debug/pytest, notebook kernel és dataframe inspect
```

Az itt felsorolt DTO-mezőket P00 helyezi a `crates/latexide-contracts/src/` moduljaiba, serde serialize/deserialize és egyértelmű hiba-enummal. A szolgáltatás a saját modulban marad; ne hozz létre protocol→core→protocol függőségi kört. A port v1 helyi Rust-határ, nem automatikus új IPC-eszköz.

- [ ] **RED:** `extensions/verticals/u02-python-data/tests/capability_contract.rs` alatt `contract_v1_first_slice` néven rögzítsd ezt az ellenőrzést: **Két eltérő venv azonos importját a kiválasztott interpreter oldja, notebook output kernelhez kötött; hiányzó capability esetén MissingCapability, nincs teljes-paritás jelölés.**
- [ ] Futtasd: `cargo +1.99.1 test --manifest-path extensions/verticals/u02-python-data/Cargo.toml --test capability_contract --locked`. Először a hiányzó szolgáltatásra vagy állított viselkedésre FAIL legyen; környezet-/linkhibát ne könyvelj reprodukciónak.
- [ ] Valósítsd meg az első portot `extensions/verticals/u02-python-data/src/lib.rs` alatt; az alkalmazás-regisztrációt add át `integration/requests/U02.json` manifestben (schema, package_id, contract_version, requirement_ids, source_paths, hook_targets, test_commands, compatibility).
- [ ] Ugyanazzal a paranccsal legyen GREEN; tedd mellé R1–R5 célzott regresszióját a `review_focus_r1` … `review_focus_r5` tesztekbe. Az állítások a fenti öt elvárt viselkedést vizsgálják.
- [ ] Ellenőrizd a saját diffet, majd készíts kis commitot: `git add -- extensions/verticals/u02-python-data/src/lib.rs extensions/verticals/u02-python-data/tests/capability_contract.rs integration/requests/U02.json` és `git commit -m "feat(u02): establish tested service slice"`. A dokumentált későbbi munkavégzéskor commitolj; ez a terv most nem futtatja ezeket.

## Követelményenkénti feladatsor

Minden alábbi sor külön tesztelhető részeredmény és saját kis commit. A teszt neve az ID kisbetűs, aláhúzásos alakja; az elfogadás mondata a kötelező assertion. A dependency provider mockja csak izolált teszthez elég; release-hez a valódi providerrel is teljesüljön.

### UNI-02 — PyCharm/DataSpell: Python és data science (P4, XL)

**Kiindulás:** Python interpreter, notebook kernel, dataframe és ML workflow nincs.

**Módosítás:** Python/venv/packaging/Jupyter/dataframe/plot/test/debug provider és remote kernelmodell.

**Valódi bekötés előtt:** EXT-02 (P15), DEV-04 (P17), DEV-05 (P17).

- [ ] `uni_02` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Notebook eredmény/runtime eredete helyes; Python és adatkutatói workflow teljes saját korpuszon működik.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test --manifest-path extensions/verticals/u02-python-data/Cargo.toml --test capability_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `U02` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `UNI-02` implemented/tested állapotúnak.

## Task Z: Bekötés és átadás

- [ ] `python3 scripts/validate_development_packages.py` és `cargo +1.99.1 test --manifest-path extensions/verticals/u02-python-data/Cargo.toml --test capability_contract --locked` sikeres; a tesztparancs itt célállapot, futtatását külön rögzíteni kell.
- [ ] P00 alkalmazza a manifestben felsorolt közös hookokat; stagingben egyesével fusson a korábbi Rust/native/IPC/MCP regresszió a megváltozott rétegnek megfelelően. Nincs automatikus Save vagy új agent-edit jóváhagyás.
- [ ] Mac natív workflow valódi fókusz-/IME-/undo-eseménnyel; Windows külön MSBuild/link/launch/funkcionális kör. Linux saját runner/felület nélkül unrun. Nem natív csomagnál ezeket az adaptergazda végzi.
- [ ] `docs/development/results/U02.md` tartalmazza a teljesített ID-ket és reprodukálható bizonyítékot. P00 ennek alapján frissíti a handoffot, ledgert és a releváns skill-reference-et; más csomag ne írja párhuzamosan ezeket.

## Másolható indítófeladat

> Hajtsd végre a `docs/superpowers/plans/2026-10-31-parallel/u02-python-data.md` tervet. Előbb olvasd az AGENTS.md-t, a handoffot, a projekt-skillt, a közös csomagszerződést és a BASELINE_DELTA-t. Csak U02 saját útvonalait módosítsd, a közös fájlokhoz integration-manifestet adj. Őrizd meg a közvetlen agent-edit + explicit Save + Save után is elérhető undo szerződést. A követelményfüggőséget és platformi bizonyítékot ellenőrizd; planned/unrun állapotot ne nevezd késznek.
