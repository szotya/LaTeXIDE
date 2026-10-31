# P17 — Task, teszt, debugger és fejlesztői eszközök Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bővíthető run/task/test/debug workflow-t épít a research mellékkód és a választható nyelvi vertikumok számára.

**Architecture:** Egy operation-state modellhez task, test és DAP adapterek kapcsolódnak. A környezetet explicit allowlist és trust határolja; eredmény adott forrás-/toolverzióhoz kötött.

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

- R1: Tesztfolyamat exit0, de hibás report ne legyen hamis Passed.
- R2: Debugger disconnect és taskcancel minden háttérműveletet lezárjon.
- R3: Forrásváltozás után coverage/benchmark eredmény stale jelzést kapjon.
- R4: HTTP és konténer tool credentialje ne kerüljön projektfájlba vagy naplóba.
- R5: Futó task közben megváltozó run configuration ne változtassa a már indult folyamat paramétereit.

---

## Tulajdon és indulás

**Branch:** `feature/p17-developer-tools`. **Profil:** core.

**Kizárólagosan létrehozható/módosítható útvonalak:**

- `crates/latexide-core/src/services/developer_tools/`
- `crates/latexide-core/tests/developer_tools_contract.rs`
- `integration/requests/P17.json`
- `docs/development/results/P17.md`

**Teszt:** `crates/latexide-core/tests/developer_tools_contract.rs`. **Eredménynapló:** `docs/development/results/P17.md`.

**Olvasandó jelenlegi belépési pontok:** `crates/latexide-build/src/lib.rs`, `crates/latexide-protocol/src/lib.rs`.

Indulás: P00 contract-v1 után, fagyasztott fixture-ökkel és fake portokkal; a többi csomag teljes elkészülte nem indulási feltétel.

Integrációs szomszédok: P15, P18. Ezek **nem csomagszintű topologikus sorrend**: az egyedi követelményfüggőségek maradnak irányadók. A kölcsönös portfüggést fixture és fokozatos bekötés oldja, nem két teljes csomag egymásra várása.

## Task 1: Önállóan ellenőrizhető szerződésszelet

**Create:** `crates/latexide-core/src/services/developer_tools/mod.rs`, `crates/latexide-core/tests/developer_tools_contract.rs`. **Consumes / produces:**

```rust
pub fn transition_task(state:TaskState, event:TaskEvent) -> Result<TaskState,TaskError>;
// TaskState::{Queued,Running,Passed,Failed,Cancelled,TimedOut}; TaskEvent::{Started,Exited(i32),Cancelled,Deadline}; TaskError::{InvalidTransition,Untrusted,InvalidConfiguration}
```

Az itt felsorolt DTO-mezőket P00 helyezi a `crates/latexide-contracts/src/` moduljaiba, serde serialize/deserialize és egyértelmű hiba-enummal. A szolgáltatás a saját modulban marad; ne hozz létre protocol→core→protocol függőségi kört. A port v1 helyi Rust-határ, nem automatikus új IPC-eszköz.

- [ ] **RED:** `crates/latexide-core/tests/developer_tools_contract.rs` alatt `contract_v1_first_slice` néven rögzítsd ezt az ellenőrzést: **Running→Exited(1) Failed, Running→Deadline TimedOut; végállapot után Started InvalidTransition.**
- [ ] Futtasd: `cargo +1.99.1 test -p latexide-core --test developer_tools_contract --locked`. Először a hiányzó szolgáltatásra vagy állított viselkedésre FAIL legyen; környezet-/linkhibát ne könyvelj reprodukciónak.
- [ ] Valósítsd meg az első portot `crates/latexide-core/src/services/developer_tools/mod.rs` alatt; az alkalmazás-regisztrációt add át `integration/requests/P17.json` manifestben (schema, package_id, contract_version, requirement_ids, source_paths, hook_targets, test_commands, compatibility).
- [ ] Ugyanazzal a paranccsal legyen GREEN; tedd mellé R1–R5 célzott regresszióját a `review_focus_r1` … `review_focus_r5` tesztekbe. Az állítások a fenti öt elvárt viselkedést vizsgálják.
- [ ] Ellenőrizd a saját diffet, majd készíts kis commitot: `git add -- crates/latexide-core/src/services/developer_tools/mod.rs crates/latexide-core/tests/developer_tools_contract.rs integration/requests/P17.json` és `git commit -m "feat(p17): establish tested service slice"`. A dokumentált későbbi munkavégzéskor commitolj; ez a terv most nem futtatja ezeket.

## Követelményenkénti feladatsor

Minden alábbi sor külön tesztelhető részeredmény és saját kis commit. A teszt neve az ID kisbetűs, aláhúzásos alakja; az elfogadás mondata a kötelező assertion. A dependency provider mockja csak izolált teszthez elég; release-hez a valódi providerrel is teljesüljön.

### DEV-03 — Általános task/run configuration (P2, L)

**Kiindulás:** Csak compiler-specifikus build supervisor van.

**Módosítás:** Deklaratív argv/cwd/env/input/output taskmodell függőségekkel, timeout/cancel és progress állapottal.

**Valódi bekötés előtt:** SEC-01 (P18), EXT-04 (P15).

- [ ] `dev_03` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Python script/formatter/test task saját eredményt ad; shellszöveg nem kötelező.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test developer_tools_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P17` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `DEV-03` implemented/tested állapotúnak.

### DEV-04 — Debug Adapter Protocol kliens (P3, L)

**Kiindulás:** Nincs debugger.

**Módosítás:** Natív breakpoint/call stack/variables/watch/debug console DAP szolgáltatásra kötve.

**Valódi bekötés előtt:** DEV-03 (P17), EXT-01 (P15).

- [ ] `dev_04` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Támogatott adapterrel step/stop/restart és stale-source mapping ténylegesen működik.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test developer_tools_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P17` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `DEV-04` implemented/tested állapotúnak.

### DEV-05 — Test explorer és eredményadapter (P2, L)

**Kiindulás:** A projekt saját tesztjei vannak; a termékben test runner nincs.

**Módosítás:** Frameworkfüggetlen discovery/run/result/cancel provider és natív tesztfa.

**Valódi bekötés előtt:** DEV-03 (P17), EXT-02 (P15).

- [ ] `dev_05` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Valódi tesztek futnak, skipped/fail/pass külön; futásból forrásba lehet ugrani.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test developer_tools_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P17` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `DEV-05` implemented/tested állapotúnak.

### DEV-06 — Coverage és benchmark nézet (P3, M)

**Kiindulás:** Nincs termékbeli coverage/profiling.

**Módosítás:** LCOV/Cobertura/adapter coverage és benchmark artifact kezelés.

**Valódi bekötés előtt:** DEV-05 (P17).

- [ ] `dev_06` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Csak illeszkedő source-revízióra fest; benchmarknál környezet és bizonytalanság jelölt.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test developer_tools_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P17` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `DEV-06` implemented/tested állapotúnak.

### DEV-07 — HTTP/REST kliens (P3, M)

**Kiindulás:** Nincs .http/.rest művelet.

**Módosítás:** Opcionális HTTP provider változókkal, eredmény/history nézettel és titokkezeléssel.

**Valódi bekötés előtt:** DEV-03 (P17), SEC-06 (P18).

- [ ] `dev_07` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Valódi status/header/body látható; secret nincs verziózott fájlban vagy exportban.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test developer_tools_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P17` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `DEV-07` implemented/tested állapotúnak.

### DEV-09 — Konténer és futtatókörnyezet nézet (P3, L)

**Kiindulás:** Nincs Docker/SDK/environment menedzsment.

**Módosítás:** Opcionális helyi container/venv/toolchain provider; állapot és futtatási eredet látható.

**Valódi bekötés előtt:** DEV-03 (P17), EXT-02 (P15).

- [ ] `dev_09` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Választott környezetben fut a task; hiányzó runtime nem látszik connected állapotnak.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test developer_tools_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P17` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `DEV-09` implemented/tested állapotúnak.

## Task Z: Bekötés és átadás

- [ ] `python3 scripts/validate_development_packages.py` és `cargo +1.99.1 test -p latexide-core --test developer_tools_contract --locked` sikeres; a tesztparancs itt célállapot, futtatását külön rögzíteni kell.
- [ ] P00 alkalmazza a manifestben felsorolt közös hookokat; stagingben egyesével fusson a korábbi Rust/native/IPC/MCP regresszió a megváltozott rétegnek megfelelően. Nincs automatikus Save vagy új agent-edit jóváhagyás.
- [ ] Mac natív workflow valódi fókusz-/IME-/undo-eseménnyel; Windows külön MSBuild/link/launch/funkcionális kör. Linux saját runner/felület nélkül unrun. Nem natív csomagnál ezeket az adaptergazda végzi.
- [ ] `docs/development/results/P17.md` tartalmazza a teljesített ID-ket és reprodukálható bizonyítékot. P00 ennek alapján frissíti a handoffot, ledgert és a releváns skill-reference-et; más csomag ne írja párhuzamosan ezeket.

## Másolható indítófeladat

> Hajtsd végre a `docs/superpowers/plans/2026-10-31-parallel/p17-developer-tools.md` tervet. Előbb olvasd az AGENTS.md-t, a handoffot, a projekt-skillt, a közös csomagszerződést és a BASELINE_DELTA-t. Csak P17 saját útvonalait módosítsd, a közös fájlokhoz integration-manifestet adj. Őrizd meg a közvetlen agent-edit + explicit Save + Save után is elérhető undo szerződést. A követelményfüggőséget és platformi bizonyítékot ellenőrizd; planned/unrun állapotot ne nevezd késznek.
