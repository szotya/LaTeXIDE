# P15 — LSP, plugin SDK és mellékfájlok Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Korlátos, verziózott bővítési határt és külső nyelvi szolgáltatást ad.

**Architecture:** Izolált providerfolyamat, capability-alku és regisztrált parancsok; a plugin nem kap kanonikus szövegírási kerülőutat. SDK fixture önállóan teszteli a provider és host kompatibilitását.

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

- R1: Összeomló vagy lassú plugin ne állítsa le a dokumentumruntimet.
- R2: Plugin API-major eltérés ne okozzon részben működő csatlakozást.
- R3: LSP régi dokumentumverzióra küldött editje csak stale hibát adhat.
- R4: Ismeretlen eredetű csomag ne kapjon automatikusan futtatási vagy hálózati jogot.
- R5: Hiányzó natív rendercapability esetén a plugin jelezzen unsupported állapotot.

---

## Tulajdon és indulás

**Branch:** `feature/p15-extensions`. **Profil:** core.

**Kizárólagosan létrehozható/módosítható útvonalak:**

- `crates/latexide-core/src/services/extensions/`
- `crates/latexide-core/tests/extension_host_contract.rs`
- `integration/requests/P15.json`
- `docs/development/results/P15.md`

**Teszt:** `crates/latexide-core/tests/extension_host_contract.rs`. **Eredménynapló:** `docs/development/results/P15.md`.

**Olvasandó jelenlegi belépési pontok:** `crates/latexide-mcp/src/main.rs`, `crates/latexide-protocol/src/lib.rs`, `docs/IDE_PRODUCT_DIRECTION.md`.

Indulás: P00 contract-v1 után, fagyasztott fixture-ökkel és fake portokkal; a többi csomag teljes elkészülte nem indulási feltétel.

Integrációs szomszédok: P02, P03, P08, P18, P21, P23. Ezek **nem csomagszintű topologikus sorrend**: az egyedi követelményfüggőségek maradnak irányadók. A kölcsönös portfüggést fixture és fokozatos bekötés oldja, nem két teljes csomag egymásra várása.

## Task 1: Önállóan ellenőrizhető szerződésszelet

**Create:** `crates/latexide-core/src/services/extensions/mod.rs`, `crates/latexide-core/tests/extension_host_contract.rs`. **Consumes / produces:**

```rust
pub fn negotiate_provider(host:ProviderCapabilities, plugin:ProviderManifest) -> Result<ProviderSession,ProviderError>;
// ProviderCapabilities { api_major:u32, commands:Vec<String> }; ProviderManifest { id:String, api_major:u32, requested_capabilities:Vec<String>, executable:String }; ProviderSession { id:String, granted_capabilities:Vec<String> }; ProviderError::{IncompatibleVersion,DeniedCapability,InvalidManifest,Untrusted}
```

Az itt felsorolt DTO-mezőket P00 helyezi a `crates/latexide-contracts/src/` moduljaiba, serde serialize/deserialize és egyértelmű hiba-enummal. A szolgáltatás a saját modulban marad; ne hozz létre protocol→core→protocol függőségi kört. A port v1 helyi Rust-határ, nem automatikus új IPC-eszköz.

- [ ] **RED:** `crates/latexide-core/tests/extension_host_contract.rs` alatt `contract_v1_first_slice` néven rögzítsd ezt az ellenőrzést: **Más API-major és nem engedélyezett capability elutasított; kompatibilis read-only provider nem kap edit jogot.**
- [ ] Futtasd: `cargo +1.99.1 test -p latexide-core --test extension_host_contract --locked`. Először a hiányzó szolgáltatásra vagy állított viselkedésre FAIL legyen; környezet-/linkhibát ne könyvelj reprodukciónak.
- [ ] Valósítsd meg az első portot `crates/latexide-core/src/services/extensions/mod.rs` alatt; az alkalmazás-regisztrációt add át `integration/requests/P15.json` manifestben (schema, package_id, contract_version, requirement_ids, source_paths, hook_targets, test_commands, compatibility).
- [ ] Ugyanazzal a paranccsal legyen GREEN; tedd mellé R1–R5 célzott regresszióját a `review_focus_r1` … `review_focus_r5` tesztekbe. Az állítások a fenti öt elvárt viselkedést vizsgálják.
- [ ] Ellenőrizd a saját diffet, majd készíts kis commitot: `git add -- crates/latexide-core/src/services/extensions/mod.rs crates/latexide-core/tests/extension_host_contract.rs integration/requests/P15.json` és `git commit -m "feat(p15): establish tested service slice"`. A dokumentált későbbi munkavégzéskor commitolj; ez a terv most nem futtatja ezeket.

## Követelményenkénti feladatsor

Minden alábbi sor külön tesztelhető részeredmény és saját kis commit. A teszt neve az ID kisbetűs, aláhúzásos alakja; az elfogadás mondata a kötelező assertion. A dependency provider mockja csak izolált teszthez elég; release-hez a valódi providerrel is teljesüljön.

### EXT-01 — Külső language-service host (P2, L)

**Kiindulás:** Nincs LSP host.

**Módosítás:** Rust tulajdonú subprocess supervisor és nyelvi provider interfész; editor csak adapter.

**Valódi bekötés előtt:** MCP-02 (P03), MCP-03 (P03).

- [ ] `ext_01` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Server crash/restart/cancel nem veszít dirty dokumentumot; nincs második hiteles textmodell.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test extension_host_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P15` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `EXT-01` implemented/tested állapotúnak.

### EXT-02 — Verziózott plugin API (P2, L)

**Kiindulás:** MCP tool bridge van; általános pluginrendszer nincs.

**Módosítás:** Először declarative theme/snippet/command/provider API, verziózott szerződéssel.

**Valódi bekötés előtt:** MCP-03 (P03), WS-06 (P08).

- [ ] `ext_02` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Hibás plugin kikapcsolható; kompatibilitás és API-életciklus dokumentált.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test extension_host_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P15` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `EXT-02` implemented/tested állapotúnak.

### EXT-03 — Plugin manifest és csomagkezelés (P2, M)

**Kiindulás:** Nincs telepített bővítmény-életciklus.

**Módosítás:** Manifest, verzió, hash, licenc, install/update/disable/uninstall helyi kezelése.

**Valódi bekötés előtt:** EXT-02 (P15), SEC-04 (P18).

- [ ] `ext_03` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Félbeszakadt frissítés rollbackelhető; ismeretlen plugin nem indul automatikusan.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test extension_host_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P15` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `EXT-03` implemented/tested állapotúnak.

### EXT-04 — Providerhatár és izoláció (P2, L)

**Kiindulás:** Codex/compiler külön subprocess, általános providerhatár nincs.

**Módosítás:** Natív UI-tól leválasztott szolgáltatás-host és minimális capability-szerződés.

**Valódi bekötés előtt:** MCP-02 (P03), SEC-02 (P18).

- [ ] `ext_04` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Egy provider túlterhelése/crash nem állítja le az editort és nem bővíti másik jogait.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test extension_host_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P15` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `EXT-04` implemented/tested állapotúnak.

### EXT-05 — Bővítménydiagnosztika és safe mode (P3, M)

**Kiindulás:** Nincs plugin profiler vagy safe startup.

**Módosítás:** Betöltési idő/hiba/RSS nézet, disable-all és safe mode.

**Valódi bekötés előtt:** EXT-03 (P15), QA-03 (P23).

- [ ] `ext_05` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Rossz bővítmény után a projekt tovább megnyitható, dirty recovery elérhető.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test extension_host_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P15` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `EXT-05` implemented/tested állapotúnak.

### EXT-06 — Extension test SDK és kompatibilitás (P3, M)

**Kiindulás:** Nincs fejlesztői SDK a termékhez.

**Módosítás:** Példák, mock host, conformance runner és verziózott migrációs útmutató.

**Valódi bekötés előtt:** EXT-02 (P15).

- [ ] `ext_06` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Külső fejlesztő checkoutfüggetlen példát le tud futtatni támogatott OS-en.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test extension_host_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P15` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `EXT-06` implemented/tested állapotúnak.

### EXT-07 — Katalógus és trust/eredet (P3, L)

**Kiindulás:** Nincs Marketplace vagy plugin discovery.

**Módosítás:** Először helyi ellenőrzött katalógus; később opcionális registry külön üzemeltetési tervvel.

**Valódi bekötés előtt:** EXT-03 (P15), SEC-04 (P18).

- [ ] `ext_07` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Eredet/licenc/kompatibilitás látszik; registryhiba nem blokkol helyi munkát.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test extension_host_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P15` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `EXT-07` implemented/tested állapotúnak.

### EXT-08 — Keresztplatformos bővítési szerződés (P2, M)

**Kiindulás:** Natív frontendek külön implementációk.

**Módosítás:** Közös contribution/command/resource modell és platformi capability-jelzés.

**Valódi bekötés előtt:** EXT-02 (P15), OS-01 (P21).

- [ ] `ext_08` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Mac-only provider nem látszik működő Windows/Linux funkciónak; hiány pontosan jelölt.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test extension_host_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P15` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `EXT-08` implemented/tested állapotúnak.

### DEV-08 — Kódolt research mellékfájlok (P2, M)

**Kiindulás:** Plain text megnyitható, nyelvfüggő programozási támogatás nincs.

**Módosítás:** Markdown/JSON/YAML/Python/R/Julia/shell nyelvi profil és LSP adapter capability szerint.

**Valódi bekötés előtt:** EXT-01 (P15), WS-04 (P08).

- [ ] `dev_08` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Egy dolgozat scripts/data/config fájljai helyes language ID-t és tartalomfüggő szolgáltatást kapnak.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test extension_host_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P15` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `DEV-08` implemented/tested állapotúnak.

### DEV-10 — Adatfájl és képelőnézet (P2, M)

**Kiindulás:** Minden megnyitás szövegként indulhat, bináris forrás elutasított.

**Módosítás:** Külön readonly CSV/JSON/kép/asset editor típusok, bounded previewval.

**Valódi bekötés előtt:** EXT-02 (P15), DOC-12 (P02).

- [ ] `dev_10` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Nagy/binary fájl nem szövegszerkesztőként hibázik; source módosítás tudatos marad.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test extension_host_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P15` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `DEV-10` implemented/tested állapotúnak.

## Task Z: Bekötés és átadás

- [ ] `python3 scripts/validate_development_packages.py` és `cargo +1.99.1 test -p latexide-core --test extension_host_contract --locked` sikeres; a tesztparancs itt célállapot, futtatását külön rögzíteni kell.
- [ ] P00 alkalmazza a manifestben felsorolt közös hookokat; stagingben egyesével fusson a korábbi Rust/native/IPC/MCP regresszió a megváltozott rétegnek megfelelően. Nincs automatikus Save vagy új agent-edit jóváhagyás.
- [ ] Mac natív workflow valódi fókusz-/IME-/undo-eseménnyel; Windows külön MSBuild/link/launch/funkcionális kör. Linux saját runner/felület nélkül unrun. Nem natív csomagnál ezeket az adaptergazda végzi.
- [ ] `docs/development/results/P15.md` tartalmazza a teljesített ID-ket és reprodukálható bizonyítékot. P00 ennek alapján frissíti a handoffot, ledgert és a releváns skill-reference-et; más csomag ne írja párhuzamosan ezeket.

## Másolható indítófeladat

> Hajtsd végre a `docs/superpowers/plans/2026-10-31-parallel/p15-extensions.md` tervet. Előbb olvasd az AGENTS.md-t, a handoffot, a projekt-skillt, a közös csomagszerződést és a BASELINE_DELTA-t. Csak P15 saját útvonalait módosítsd, a közös fájlokhoz integration-manifestet adj. Őrizd meg a közvetlen agent-edit + explicit Save + Save után is elérhető undo szerződést. A követelményfüggőséget és platformi bizonyítékot ellenőrizd; planned/unrun állapotot ne nevezd késznek.
