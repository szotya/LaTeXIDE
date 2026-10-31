# P00 — Szerződések és integrációgazda Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Versionált DTO-kat, izolált tesztfixture-öket és egyetlen bekötési folyamatot ad minden párhuzamos csomagnak.

**Architecture:** Új, függőségi kört nem alkotó latexide-contracts crate; csak P00 módosít közös regisztrációt, lockfile-t, protokoll-dispatchot és handoffot. Minden más csomag saját mappából ad adaptert és integration-manifestet.

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

- R1: Contract-változás közben nyitott régi kliens explicit verzióhibát kapjon, ne félreértelmezett választ.
- R2: Párhuzamos kérelmek ugyanazt a közös hookot kérik: determinisztikus egyesítés és regresszió kell.
- R3: Audit óta megvalósult képesség ne legyen ismét hiányzónak jelölve.
- R4: Hiányzó Windows/Linux futás maradjon unrun akkor is, ha a közös Rust ellenőrzés zöld.
- R5: A dokumentációgenerátor változatlan bemenetre byte-azonos fájlokat adjon.

---

## Tulajdon és indulás

**Branch:** `feature/p00-integration`. **Profil:** core.

**Kizárólagosan létrehozható/módosítható útvonalak:**

- `crates/latexide-contracts/`
- `integration/registry/`
- `tests/contracts/`
- `docs/development/contracts/`
- `integration/requests/P00.json`
- `docs/development/results/P00.md`

**P00 kizárólagos közös bekötési útvonalai** (csak fogadott manifest és regresszió mellett; a többi csomagnak olvasható):

- `Cargo.toml`
- `Cargo.lock`
- `rust-toolchain.toml`
- `crates/*/Cargo.toml`
- `crates/*/src/lib.rs`
- `crates/latexide-core/src/{workspace,document,project_index,tex_scan,completion,definition}.rs`
- `crates/latexide-protocol/src/ipc.rs`
- `crates/latexide-ffi/src/{runtime,session}.rs`
- `crates/latexide-mcp/src/main.rs`
- `include/latexide_core.h`
- `apps/macos/Package.swift`
- `apps/macos/Sources/LaTeXIDEKit/{WorkspaceStore,WorkspaceView,NativeRuntime,NativeEditor}.swift`
- `apps/macos/Sources/LaTeXIDEKit/Editor/SourceTextView.swift`
- `apps/windows/{MainWindow*,NativeCore*,LaTeXIDE.vcxproj,packages.config,NuGet.Native.*}`
- `scripts/{build_macos.sh,build_windows.ps1}`
- `.github/workflows/`
- `docs/{DEVELOPER_HANDOFF,implementation-progress}.md`
- `skills/latexide-platform-development/`
- `docs/development/package-specs.json`
- `docs/development/packages.json`
- `docs/development/README.md`
- `docs/development/requirement-map.csv`
- `docs/superpowers/specs/2026-10-31-parallel-development-design.md`
- `docs/superpowers/plans/2026-10-31-parallel/`

**Teszt:** `crates/latexide-contracts/tests/compatibility.rs`. **Eredménynapló:** `docs/development/results/P00.md`.

**Olvasandó jelenlegi belépési pontok:** `Cargo.toml`, `crates/latexide-protocol/src/lib.rs`, `docs/DEVELOPER_HANDOFF.md`.

Indulás: már most elindítható környezet-/tesztfeltárással; a termékbekötéshez P00 contract-v1 kell.

Integrációs szomszédok: P03. Ezek **nem csomagszintű topologikus sorrend**: az egyedi követelményfüggőségek maradnak irányadók. A kölcsönös portfüggést fixture és fokozatos bekötés oldja, nem két teljes csomag egymásra várása.

## Task 1: Önállóan ellenőrizhető szerződésszelet

**Create:** `crates/latexide-contracts/src/lib.rs`, `crates/latexide-contracts/tests/compatibility.rs`. **Consumes / produces:**

```rust
pub fn validate_manifest(manifest: IntegrationManifest, contract: ContractVersion, ownership: &OwnershipRegistry) -> Result<IntegrationOrder, ContractError>;
// IntegrationManifest { schema:u32, package_id:String, contract_version:u32, requirement_ids:Vec<String>, source_paths:Vec<String>, hook_targets:Vec<String>, test_commands:Vec<String>, compatibility:String }
// ContractVersion { major:u32, minor:u32 }; OwnershipRegistry { paths:Vec<OwnedPath>, requirements:Vec<OwnedRequirement> }; OwnedPath { package_id:String, path:String }; OwnedRequirement { package_id:String, requirement_id:String, prerequisites:Vec<String> }; IntegrationOrder { requirement_ids:Vec<String> }; ContractError::{UnknownRequirement,ForeignPath,IncompatibleVersion,CyclicRequirement}
```

Az itt felsorolt DTO-mezőket P00 helyezi a `crates/latexide-contracts/src/` moduljaiba, serde serialize/deserialize és egyértelmű hiba-enummal. A szolgáltatás a saját modulban marad; ne hozz létre protocol→core→protocol függőségi kört. A port v1 helyi Rust-határ, nem automatikus új IPC-eszköz.

- [ ] **RED:** `crates/latexide-contracts/tests/compatibility.rs` alatt `contract_v1_first_slice` néven rögzítsd ezt az ellenőrzést: **Idegen csomag fájlútvonalát tartalmazó manifest ForeignPath hibával elutasított, a v1 fixture oda-vissza veszteségmentes.**
- [ ] Futtasd: `cargo +1.99.1 test -p latexide-contracts --test compatibility --locked`. Először a hiányzó szolgáltatásra vagy állított viselkedésre FAIL legyen; környezet-/linkhibát ne könyvelj reprodukciónak.
- [ ] Valósítsd meg az első portot `crates/latexide-contracts/src/lib.rs` alatt; az alkalmazás-regisztrációt add át `integration/requests/P00.json` manifestben (schema, package_id, contract_version, requirement_ids, source_paths, hook_targets, test_commands, compatibility).
- [ ] Ugyanazzal a paranccsal legyen GREEN; tedd mellé R1–R5 célzott regresszióját a `review_focus_r1` … `review_focus_r5` tesztekbe. Az állítások a fenti öt elvárt viselkedést vizsgálják.
- [ ] Ellenőrizd a saját diffet, majd készíts kis commitot: `git add -- crates/latexide-contracts/src/lib.rs crates/latexide-contracts/tests/compatibility.rs integration/requests/P00.json` és `git commit -m "feat(p00): establish tested service slice"`. A dokumentált későbbi munkavégzéskor commitolj; ez a terv most nem futtatja ezeket.

## Követelményenkénti feladatsor

Minden alábbi sor külön tesztelhető részeredmény és saját kis commit. A teszt neve az ID kisbetűs, aláhúzásos alakja; az elfogadás mondata a kötelező assertion. A dependency provider mockja csak izolált teszthez elég; release-hez a valódi providerrel is teljesüljön.

### QA-08 — Ismételhető helyi ellenőrző kapu és CI (P1, M)

**Kiindulás:** Több kézi script/teszt van; repo/CI pipeline nincs létrehozva.

**Módosítás:** Egy verify parancs manifesttel; később tényleges Git/runner elérhetőségnél Mac/Windows/Linux CI.

**Valódi bekötés előtt:** MCP-03 (P03).

- [ ] `qa_08` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Exit code, skipped/unrun és artifact hash géppel feldolgozható; timeout soha nem PASS.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-contracts --test compatibility --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P00` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `QA-08` implemented/tested állapotúnak.

### QA-14 — Dokumentáció és capability-leltár frissessége (P1, M)

**Kiindulás:** Handoff/ledger/skill karbantartott; audit közben új increment zárult.

**Módosítás:** Teszt/forrásmanifestből állapotellenőrzés, törött link és source-only jelölés automatizálása.

**Valódi bekötés előtt:** QA-08 (P00).

- [ ] `qa_14` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Aktuális forrás, binary, teszt és leírás nem kap összemosott kész állapotot.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-contracts --test compatibility --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P00` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `QA-14` implemented/tested állapotúnak.

### OS-10 — Hordozható handoff és fejlesztői skill (P1, S)

**Kiindulás:** Projekt-skill és references él; runtime/platform határok dokumentáltak.

**Módosítás:** Minden új közös szerződés után referencia és parancs/eredmény/korlát frissítése.

**Valódi bekötés előtt:** QA-14 (P00).

- [ ] `os_10` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Új contributor relatív útvonalakból elindul; planned/check/build/run állapotokat megkülönbözteti.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-contracts --test compatibility --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P00` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `OS-10` implemented/tested állapotúnak.

## Task Z: Bekötés és átadás

- [ ] `python3 scripts/validate_development_packages.py` és `cargo +1.99.1 test -p latexide-contracts --test compatibility --locked` sikeres; a tesztparancs itt célállapot, futtatását külön rögzíteni kell.
- [ ] P00 alkalmazza a manifestben felsorolt közös hookokat; stagingben egyesével fusson a korábbi Rust/native/IPC/MCP regresszió a megváltozott rétegnek megfelelően. Nincs automatikus Save vagy új agent-edit jóváhagyás.
- [ ] Mac natív workflow valódi fókusz-/IME-/undo-eseménnyel; Windows külön MSBuild/link/launch/funkcionális kör. Linux saját runner/felület nélkül unrun. Nem natív csomagnál ezeket az adaptergazda végzi.
- [ ] `docs/development/results/P00.md` tartalmazza a teljesített ID-ket és reprodukálható bizonyítékot. P00 ennek alapján frissíti a handoffot, ledgert és a releváns skill-reference-et; más csomag ne írja párhuzamosan ezeket.

## Másolható indítófeladat

> Hajtsd végre a `docs/superpowers/plans/2026-10-31-parallel/p00-integration.md` tervet. Előbb olvasd az AGENTS.md-t, a handoffot, a projekt-skillt, a közös csomagszerződést és a BASELINE_DELTA-t. Csak P00 saját útvonalait módosítsd, a közös fájlokhoz integration-manifestet adj. Őrizd meg a közvetlen agent-edit + explicit Save + Save után is elérhető undo szerződést. A követelményfüggőséget és platformi bizonyítékot ellenőrizd; planned/unrun állapotot ne nevezd késznek.
