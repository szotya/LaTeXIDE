# P16 — Integrált terminál és shellprofilok Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Valódi PTY/ConPTY terminált ad korlátozott, átlátható profilokkal.

**Architecture:** Külön terminálfolyamat és bounded scrollback, natív renderadapterrel. macOS PTY és Windows ConPTY külön backend; shell életciklus a trustpolicy és taskcancel szerződését használja.

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

- R1: Nagymennyiségű kimenet korlátos scrollback mellett se fagyassza a szerkesztőt.
- R2: Átméretezés és Unicode/combining karakter ne rontsa a cellaállapotot.
- R3: Shell gyermekfolyamat maradéktalanul megszakítható legyen mindkét OS-en.
- R4: Bracketed paste és veszélyes escape-szekvencia ne módosítson IDE-beállítást ellenőrizetlenül.
- R5: Projektkonfigurációból induló shell csak trust mellett kaphat folyamatindítási jogot.

---

## Tulajdon és indulás

**Branch:** `feature/p16-terminal`. **Profil:** core.

**Kizárólagosan létrehozható/módosítható útvonalak:**

- `crates/latexide-core/src/services/terminal/`
- `crates/latexide-core/tests/terminal_contract.rs`
- `integration/requests/P16.json`
- `docs/development/results/P16.md`

**Teszt:** `crates/latexide-core/tests/terminal_contract.rs`. **Eredménynapló:** `docs/development/results/P16.md`.

**Olvasandó jelenlegi belépési pontok:** `crates/latexide-build/src/lib.rs`, `apps/windows/MainWindow.xaml.cpp`.

Indulás: P00 contract-v1 után, fagyasztott fixture-ökkel és fake portokkal; a többi csomag teljes elkészülte nem indulási feltétel.

Integrációs szomszédok: P15, P18. Ezek **nem csomagszintű topologikus sorrend**: az egyedi követelményfüggőségek maradnak irányadók. A kölcsönös portfüggést fixture és fokozatos bekötés oldja, nem két teljes csomag egymásra várása.

## Task 1: Önállóan ellenőrizhető szerződésszelet

**Create:** `crates/latexide-core/src/services/terminal/mod.rs`, `crates/latexide-core/tests/terminal_contract.rs`. **Consumes / produces:**

```rust
pub fn validate_terminal_profile(profile:TerminalProfile, trust:TrustDecision) -> Result<TerminalLaunch,TerminalError>;
// TerminalProfile { executable:String, args:Vec<String>, cwd:String, env_keys:Vec<String> }; TerminalLaunch { executable:String, args:Vec<String>, cwd:String }; TrustDecision::{Trusted,Untrusted}; TerminalError::{Untrusted,OutsideRoot,InvalidExecutable,DeniedEnvironment}
```

Az itt felsorolt DTO-mezőket P00 helyezi a `crates/latexide-contracts/src/` moduljaiba, serde serialize/deserialize és egyértelmű hiba-enummal. A szolgáltatás a saját modulban marad; ne hozz létre protocol→core→protocol függőségi kört. A port v1 helyi Rust-határ, nem automatikus új IPC-eszköz.

- [ ] **RED:** `crates/latexide-core/tests/terminal_contract.rs` alatt `contract_v1_first_slice` néven rögzítsd ezt az ellenőrzést: **Untrusted projekt profilja nem indul; szóközös shellút argumentumtömbként változatlan, credential-env nem öröklődik.**
- [ ] Futtasd: `cargo +1.99.1 test -p latexide-core --test terminal_contract --locked`. Először a hiányzó szolgáltatásra vagy állított viselkedésre FAIL legyen; környezet-/linkhibát ne könyvelj reprodukciónak.
- [ ] Valósítsd meg az első portot `crates/latexide-core/src/services/terminal/mod.rs` alatt; az alkalmazás-regisztrációt add át `integration/requests/P16.json` manifestben (schema, package_id, contract_version, requirement_ids, source_paths, hook_targets, test_commands, compatibility).
- [ ] Ugyanazzal a paranccsal legyen GREEN; tedd mellé R1–R5 célzott regresszióját a `review_focus_r1` … `review_focus_r5` tesztekbe. Az állítások a fenti öt elvárt viselkedést vizsgálják.
- [ ] Ellenőrizd a saját diffet, majd készíts kis commitot: `git add -- crates/latexide-core/src/services/terminal/mod.rs crates/latexide-core/tests/terminal_contract.rs integration/requests/P16.json` és `git commit -m "feat(p16): establish tested service slice"`. A dokumentált későbbi munkavégzéskor commitolj; ez a terv most nem futtatja ezeket.

## Követelményenkénti feladatsor

Minden alábbi sor külön tesztelhető részeredmény és saját kis commit. A teszt neve az ID kisbetűs, aláhúzásos alakja; az elfogadás mondata a kötelező assertion. A dependency provider mockja csak izolált teszthez elég; release-hez a valódi providerrel is teljesüljön.

### DEV-01 — Valódi integrált terminál (P2, L)

**Kiindulás:** Compiler output panel van, PTY/ConPTY shell nincs.

**Módosítás:** Natív termináladapter PTY/ConPTY háttérrel, Unicode/resize/scrollback/links szerződéssel.

**Valódi bekötés előtt:** SEC-01 (P18), EXT-04 (P15).

- [ ] `dev_01` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Interaktív shell működik; billentyűket fókusztulajdonos kapja, child kilépés kezelhető.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test terminal_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P16` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `DEV-01` implemented/tested állapotúnak.

### DEV-02 — Terminálprofilok és shellintegráció (P2, M)

**Kiindulás:** Nincs shell a termékben.

**Módosítás:** Választható shell/profil, cwd/command detection és megtartási keret.

**Valódi bekötés előtt:** DEV-01 (P16).

- [ ] `dev_02` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Projektváltás és terminál-újranyitás nem használ hibás munkamappát; nincs globális shellrc átírás.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test terminal_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P16` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `DEV-02` implemented/tested állapotúnak.

## Task Z: Bekötés és átadás

- [ ] `python3 scripts/validate_development_packages.py` és `cargo +1.99.1 test -p latexide-core --test terminal_contract --locked` sikeres; a tesztparancs itt célállapot, futtatását külön rögzíteni kell.
- [ ] P00 alkalmazza a manifestben felsorolt közös hookokat; stagingben egyesével fusson a korábbi Rust/native/IPC/MCP regresszió a megváltozott rétegnek megfelelően. Nincs automatikus Save vagy új agent-edit jóváhagyás.
- [ ] Mac natív workflow valódi fókusz-/IME-/undo-eseménnyel; Windows külön MSBuild/link/launch/funkcionális kör. Linux saját runner/felület nélkül unrun. Nem natív csomagnál ezeket az adaptergazda végzi.
- [ ] `docs/development/results/P16.md` tartalmazza a teljesített ID-ket és reprodukálható bizonyítékot. P00 ennek alapján frissíti a handoffot, ledgert és a releváns skill-reference-et; más csomag ne írja párhuzamosan ezeket.

## Másolható indítófeladat

> Hajtsd végre a `docs/superpowers/plans/2026-10-31-parallel/p16-terminal.md` tervet. Előbb olvasd az AGENTS.md-t, a handoffot, a projekt-skillt, a közös csomagszerződést és a BASELINE_DELTA-t. Csak P16 saját útvonalait módosítsd, a közös fájlokhoz integration-manifestet adj. Őrizd meg a közvetlen agent-edit + explicit Save + Save után is elérhető undo szerződést. A követelményfüggőséget és platformi bizonyítékot ellenőrizd; planned/unrun állapotot ne nevezd késznek.
