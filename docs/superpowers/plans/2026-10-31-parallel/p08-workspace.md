# P08 — Beállítások, parancsok és munkaterület Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Egy koherens, tartósan személyre szabható natív munkaterületet épít egységes parancs- és fókuszmodellel.

**Architecture:** Verziózott Rust settings és command-state adatokra épülő natív workspace. Profil és többablakos állapot dokumentumazonosítót hivatkozik, nem duplikált szövegtárolót.

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

- R1: Sérült vagy újabb sémájú settings ne semmisítse meg a régi értékeket.
- R2: Asszisztens el nem küldött draftja panel-/profil-/ablakváltáskor maradjon meg.
- R3: Command shortcut csak a fókuszt birtokló célra hasson.
- R4: Több monitorról eltűnő ablak visszahozható legyen az elérhető képernyőre.
- R5: Projektbeállítás ne tudjon jóváhagyatlan tetszőleges parancsot futtatni.

---

## Tulajdon és indulás

**Branch:** `feature/p08-workspace`. **Profil:** core.

**Kizárólagosan létrehozható/módosítható útvonalak:**

- `crates/latexide-core/src/services/workspace_settings/`
- `crates/latexide-core/tests/workspace_settings_contract.rs`
- `integration/requests/P08.json`
- `docs/development/results/P08.md`

**Teszt:** `crates/latexide-core/tests/workspace_settings_contract.rs`. **Eredménynapló:** `docs/development/results/P08.md`.

**Olvasandó jelenlegi belépési pontok:** `apps/macos/Sources/LaTeXIDEKit/WorkspaceStore.swift`, `apps/macos/Sources/LaTeXIDEKit/WorkspaceView.swift`, `shared/ide-theme`.

Indulás: P00 contract-v1 után, fagyasztott fixture-ökkel és fake portokkal; a többi csomag teljes elkészülte nem indulási feltétel.

Integrációs szomszédok: P01, P02, P03, P06, P11, P23. Ezek **nem csomagszintű topologikus sorrend**: az egyedi követelményfüggőségek maradnak irányadók. A kölcsönös portfüggést fixture és fokozatos bekötés oldja, nem két teljes csomag egymásra várása.

## Task 1: Önállóan ellenőrizhető szerződésszelet

**Create:** `crates/latexide-core/src/services/workspace_settings/mod.rs`, `crates/latexide-core/tests/workspace_settings_contract.rs`. **Consumes / produces:**

```rust
pub fn resolve_settings(user:SettingsLayer, project:SettingsLayer, profile:SettingsLayer) -> Result<EffectiveSettings,SettingsError>;
// SettingsLayer { schema:u32, values:Vec<SettingValue> }; SettingValue { key:String, value:String }; EffectiveSettings { schema:u32, values:Vec<SettingValue>, provenance:Vec<String> }; SettingsError::{UnsupportedSchema,InvalidValue,MigrationFailed}
```

Az itt felsorolt DTO-mezőket P00 helyezi a `crates/latexide-contracts/src/` moduljaiba, serde serialize/deserialize és egyértelmű hiba-enummal. A szolgáltatás a saját modulban marad; ne hozz létre protocol→core→protocol függőségi kört. A port v1 helyi Rust-határ, nem automatikus új IPC-eszköz.

- [ ] **RED:** `crates/latexide-core/tests/workspace_settings_contract.rs` alatt `contract_v1_first_slice` néven rögzítsd ezt az ellenőrzést: **User→project→profile felülírás determinisztikus, ismeretlen séma hibája megőrzi az eredeti settingsfájlt.**
- [ ] Futtasd: `cargo +1.99.1 test -p latexide-core --test workspace_settings_contract --locked`. Először a hiányzó szolgáltatásra vagy állított viselkedésre FAIL legyen; környezet-/linkhibát ne könyvelj reprodukciónak.
- [ ] Valósítsd meg az első portot `crates/latexide-core/src/services/workspace_settings/mod.rs` alatt; az alkalmazás-regisztrációt add át `integration/requests/P08.json` manifestben (schema, package_id, contract_version, requirement_ids, source_paths, hook_targets, test_commands, compatibility).
- [ ] Ugyanazzal a paranccsal legyen GREEN; tedd mellé R1–R5 célzott regresszióját a `review_focus_r1` … `review_focus_r5` tesztekbe. Az állítások a fenti öt elvárt viselkedést vizsgálják.
- [ ] Ellenőrizd a saját diffet, majd készíts kis commitot: `git add -- crates/latexide-core/src/services/workspace_settings/mod.rs crates/latexide-core/tests/workspace_settings_contract.rs integration/requests/P08.json` és `git commit -m "feat(p08): establish tested service slice"`. A dokumentált későbbi munkavégzéskor commitolj; ez a terv most nem futtatja ezeket.

## Követelményenkénti feladatsor

Minden alábbi sor külön tesztelhető részeredmény és saját kis commit. A teszt neve az ID kisbetűs, aláhúzásos alakja; az elfogadás mondata a kötelező assertion. A dependency provider mockja csak izolált teszthez elég; release-hez a valódi providerrel is teljesüljön.

### WS-03 — Több szerkesztőcsoport és ablak (P2, L)

**Kiindulás:** Egy aktív dokumentum és egy editor; PDF/Assistant közös inspector.

**Módosítás:** Közös DocumentID-ra mutató több EditorInstanceID és persistált split/layout modell.

**Valódi bekötés előtt:** WS-02 (P01), DOC-02 (P01).

- [ ] `ws_03` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Két nézet ugyanazt a Rust tartalmat látja, saját caret/scroll állapottal.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test workspace_settings_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P08` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `WS-03` implemented/tested állapotúnak.

### WS-04 — Verziózott beállítási rendszer (P1, M)

**Kiindulás:** UserDefaults és külön Windows preferences; több globális érték van.

**Módosítás:** Közös settings schema, user/workspace/nyelvi precedencia és migráció.

**Valódi bekötés előtt:** MCP-03 (P03).

- [ ] `ws_04` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Hibás/újabb konfiguráció nem írja felül a jó állapotot; platformon eltérő érték jelölt.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test workspace_settings_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P08` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `WS-04` implemented/tested állapotúnak.

### WS-05 — Writing/review/developer profilok (P2, M)

**Kiindulás:** Színek/panelek kapcsolhatók, profilmodell nincs.

**Módosítás:** Layout+settings profilok ugyanazon workspace fölött, import/exporttal.

**Valódi bekötés előtt:** WS-04 (P08), WS-02 (P01).

- [ ] `ws_05` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Profilváltás nem törli a draftot, dirty szöveget vagy folyamatos agentturnt.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test workspace_settings_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P08` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `WS-05` implemented/tested állapotúnak.

### WS-06 — Egységes command registry (P1, M)

**Kiindulás:** Mac menü és Quick Open külön hardcoded akciólisták.

**Módosítás:** Stabil command ID, cím, enablement, fókuszscope és billentyűbinding közös modellben.

**Valódi bekötés előtt:** nincs.

- [ ] `ws_06` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Menü/paletta/toolbar ugyanazt a parancsot és tiltott okot adja; nincs halott akció.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test workspace_settings_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P08` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `WS-06` implemented/tested állapotúnak.

### WS-07 — Billentyűparancs testreszabás (P2, M)

**Kiindulás:** Beégetett shortcutok és fókuszvédelem van.

**Módosítás:** Persistált keymap, context/when szabály és ütközésnézet, kiosztásfüggetlen logika.

**Valódi bekötés előtt:** WS-06 (P08), QA-06 (P23).

- [ ] `ws_07` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **HU/US/AltGr alatt is helyes; prompt natív undo-ját globális binding nem lopja el.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test workspace_settings_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P08` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `WS-07` implemented/tested állapotúnak.

### WS-08 — Projektbeállítás és fődokumentum tárolása (P1, M)

**Kiindulás:** Főfájl alapértelmezett/felületen változik; .latexide output hely van.

**Módosítás:** Verziózott projektmetaadat és user-local állapot külön; relatív, hordozható utak.

**Valódi bekötés előtt:** WS-04 (P08).

- [ ] `ws_08` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Projekt másik gépen is helyes főfájlt/profilt választ; helyi abszolút path nem lesz kötelező.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test workspace_settings_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P08` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `WS-08` implemented/tested állapotúnak.

### WS-09 — Recent projects és welcome workflow (P2, M)

**Kiindulás:** Csak projektmappa megnyitása szerepel.

**Módosítás:** Közelmúlt, sablonból létrehozás, hibás/elérhetetlen path kezelése.

**Valódi bekötés előtt:** DOC-06 (P02), RES-02 (P11).

- [ ] `ws_09` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Hiányzó T9/volume nem fagyaszt; tiszta projekt létrehozása egyértelmű lépésekből áll.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test workspace_settings_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P08` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `WS-09` implemented/tested állapotúnak.

### WS-11 — Fókuszmodell az összes nézethez (P1, M)

**Kiindulás:** Undo fókusztulajdonlás tesztelt; nincs teljes tab/focus navigációs modell.

**Módosítás:** Panelkapcsolás, Escape/Tab és elrejtés fókuszszerződése minden új nézetnél.

**Valódi bekötés előtt:** WS-06 (P08), QA-06 (P23).

- [ ] `ws_11` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Billentyűzettel teljes alapworkflow végigvihető; eltűnt view nem marad responder.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test workspace_settings_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P08` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `WS-11` implemented/tested állapotúnak.

### WS-12 — Kis ablak, több monitor és skálázás (P1, M)

**Kiindulás:** Kontrollált nagyablakos render és méretezhető panelek vannak.

**Módosítás:** Méret/DPI/safe-area mátrix, minimumok és visszaállítható elrendezés.

**Valódi bekötés előtt:** QA-07 (P23).

- [ ] `ws_12` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Kis laptopon nem takaródik el mentés/build/hiba; monitorcsere nem veszíti az ablakot.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test workspace_settings_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P08` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `WS-12` implemented/tested állapotúnak.

### WS-13 — Állapot- és értesítési központ (P2, M)

**Kiindulás:** Státuszsor és egy failure mező működik.

**Módosítás:** Operation-scoped loading/error/retry/cancel értesítések, history és nem modális jelzések.

**Valódi bekötés előtt:** MCP-02 (P03), WS-06 (P08).

- [ ] `ws_13` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Egyszerre build+index+agent állapota nem írja felül egymást vagy hamisít completed állapotot.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test workspace_settings_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P08` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `WS-13` implemented/tested állapotúnak.

### WS-14 — Paletták és egyéni theme export (P2, M)

**Kiindulás:** Négy család és light/dark, System/fix mód tesztelt.

**Módosítás:** Validált user theme, font/layout export és reset; platformazonos szemantikus role-ok.

**Valódi bekötés előtt:** WS-04 (P08), QA-06 (P23).

- [ ] `ws_14` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Kontrasztellenőrzés és hibás theme fallback; app újratelepítés nem törli a választást.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test workspace_settings_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P08` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `WS-14` implemented/tested állapotúnak.

### OS-08 — OS fájltípus és launcher integráció (P2, M)

**Kiindulás:** Mac .app és fejlesztői --project van; teljes open-file/CLI workflow nincs.

**Módosítás:** Open with, drag project/file, recent documents és latexide path:line launcher.

**Valódi bekötés előtt:** WS-09 (P08), NAV-04 (P06).

- [ ] `os_08` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Már futó példányban helyes projekt/hely nyílik, dirty projektváltás védett.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test workspace_settings_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P08` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `OS-08` implemented/tested állapotúnak.

## Task Z: Bekötés és átadás

- [ ] `python3 scripts/validate_development_packages.py` és `cargo +1.99.1 test -p latexide-core --test workspace_settings_contract --locked` sikeres; a tesztparancs itt célállapot, futtatását külön rögzíteni kell.
- [ ] P00 alkalmazza a manifestben felsorolt közös hookokat; stagingben egyesével fusson a korábbi Rust/native/IPC/MCP regresszió a megváltozott rétegnek megfelelően. Nincs automatikus Save vagy új agent-edit jóváhagyás.
- [ ] Mac natív workflow valódi fókusz-/IME-/undo-eseménnyel; Windows külön MSBuild/link/launch/funkcionális kör. Linux saját runner/felület nélkül unrun. Nem natív csomagnál ezeket az adaptergazda végzi.
- [ ] `docs/development/results/P08.md` tartalmazza a teljesített ID-ket és reprodukálható bizonyítékot. P00 ennek alapján frissíti a handoffot, ledgert és a releváns skill-reference-et; más csomag ne írja párhuzamosan ezeket.

## Másolható indítófeladat

> Hajtsd végre a `docs/superpowers/plans/2026-10-31-parallel/p08-workspace.md` tervet. Előbb olvasd az AGENTS.md-t, a handoffot, a projekt-skillt, a közös csomagszerződést és a BASELINE_DELTA-t. Csak P08 saját útvonalait módosítsd, a közös fájlokhoz integration-manifestet adj. Őrizd meg a közvetlen agent-edit + explicit Save + Save után is elérhető undo szerződést. A követelményfüggőséget és platformi bizonyítékot ellenőrizd; planned/unrun állapotot ne nevezd késznek.
