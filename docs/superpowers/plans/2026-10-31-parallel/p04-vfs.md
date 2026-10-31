# P04 — VFS, fájlfigyelés és projektmanifest Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Friss, kanonikus projektképet és dirty-overlayt ad keresésnek, nyelvi szolgáltatásnak és buildnek.

**Architecture:** Egy Rust VFS a lemezmanifest és a kanonikus dirty dokumentumok rendezett metszete. OS-watcher csak invalidál; overflow után teljes reconcile, hash/identitás ellenőrzés szükséges.

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

- R1: Watcher eseményvesztés után teljes reconcile állítsa helyre a projektképet.
- R2: Kis-/nagybetűben vagy Unicode-normalizációban eltérő fájlok ne legyenek néma aliasok.
- R3: Symlink és gyökérből kilépő út explicit hozzáférési döntést kapjon.
- R4: Dirty buffer tartalma legyen elsődleges a diszkhez képest.
- R5: Törölt majd azonos néven újra létrehozott fájl új identitása minden fogyasztóhoz jusson el.

---

## Tulajdon és indulás

**Branch:** `feature/p04-vfs`. **Profil:** core.

**Kizárólagosan létrehozható/módosítható útvonalak:**

- `crates/latexide-core/src/services/vfs/`
- `crates/latexide-core/tests/vfs_contract.rs`
- `integration/requests/P04.json`
- `docs/development/results/P04.md`

**Teszt:** `crates/latexide-core/tests/vfs_contract.rs`. **Eredménynapló:** `docs/development/results/P04.md`.

**Olvasandó jelenlegi belépési pontok:** `crates/latexide-core/src/project_index.rs`, `crates/latexide-core/src/workspace.rs`.

Indulás: P00 contract-v1 után, fagyasztott fixture-ökkel és fake portokkal; a többi csomag teljes elkészülte nem indulási feltétel.

Integrációs szomszédok: P01, P02, P03. Ezek **nem csomagszintű topologikus sorrend**: az egyedi követelményfüggőségek maradnak irányadók. A kölcsönös portfüggést fixture és fokozatos bekötés oldja, nem két teljes csomag egymásra várása.

## Task 1: Önállóan ellenőrizhető szerződésszelet

**Create:** `crates/latexide-core/src/services/vfs/mod.rs`, `crates/latexide-core/tests/vfs_contract.rs`. **Consumes / produces:**

```rust
pub fn reconcile(previous:&VfsSnapshot, disk:Vec<VfsEntry>, overlays:Vec<DirtyOverlay>) -> Result<VfsSnapshot,VfsError>;
// VfsEntry { path:String, content_sha256:String, size:u64 }; DirtyOverlay { path:String, revision:u64, content_sha256:String }; VfsSnapshot { epoch:u64, entries:Vec<VfsEntry> }; VfsError::{OutsideRoot,IdentityConflict,Quota}
```

Az itt felsorolt DTO-mezőket P00 helyezi a `crates/latexide-contracts/src/` moduljaiba, serde serialize/deserialize és egyértelmű hiba-enummal. A szolgáltatás a saját modulban marad; ne hozz létre protocol→core→protocol függőségi kört. A port v1 helyi Rust-határ, nem automatikus új IPC-eszköz.

- [ ] **RED:** `crates/latexide-core/tests/vfs_contract.rs` alatt `contract_v1_first_slice` néven rögzítsd ezt az ellenőrzést: **Változatlan méret/mtime mellett módosult chapter.tex új hash-t és epochot ad, dirty overlayt nem ír felül.**
- [ ] Futtasd: `cargo +1.99.1 test -p latexide-core --test vfs_contract --locked`. Először a hiányzó szolgáltatásra vagy állított viselkedésre FAIL legyen; környezet-/linkhibát ne könyvelj reprodukciónak.
- [ ] Valósítsd meg az első portot `crates/latexide-core/src/services/vfs/mod.rs` alatt; az alkalmazás-regisztrációt add át `integration/requests/P04.json` manifestben (schema, package_id, contract_version, requirement_ids, source_paths, hook_targets, test_commands, compatibility).
- [ ] Ugyanazzal a paranccsal legyen GREEN; tedd mellé R1–R5 célzott regresszióját a `review_focus_r1` … `review_focus_r5` tesztekbe. Az állítások a fenti öt elvárt viselkedést vizsgálják.
- [ ] Ellenőrizd a saját diffet, majd készíts kis commitot: `git add -- crates/latexide-core/src/services/vfs/mod.rs crates/latexide-core/tests/vfs_contract.rs integration/requests/P04.json` és `git commit -m "feat(p04): establish tested service slice"`. A dokumentált későbbi munkavégzéskor commitolj; ez a terv most nem futtatja ezeket.

## Követelményenkénti feladatsor

Minden alábbi sor külön tesztelhető részeredmény és saját kis commit. A teszt neve az ID kisbetűs, aláhúzásos alakja; az elfogadás mondata a kötelező assertion. A dependency provider mockja csak izolált teszthez elég; release-hez a valódi providerrel is teljesüljön.

### NAV-12 — Projektmanifest és tree élő frissessége (P1, M)

**Kiindulás:** Fájllista projektmegnyitáskor készül; index saját külön discoveryt használ.

**Módosítás:** Manifest diff események és a két bejáró összevonása.

**Valódi bekötés előtt:** WS-01 (P04), DOC-07 (P01).

- [ ] `nav_12` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Külső create/delete/rename a fában, keresésben és completionben ugyanúgy megjelenik.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test vfs_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P04` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `NAV-12` implemented/tested állapotúnak.

### WS-01 — Közös VFS és projektmanifest (P1, L)

**Kiindulás:** list_files és completion discovery eltérő bejáró és korlát.

**Módosítás:** Egy fájlazonosság-/manifest-szolgáltatás, ignore, file-events és lapozott állapot.

**Valódi bekötés előtt:** MCP-02 (P03).

- [ ] `ws_01` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Fa, keresés, index és build azonos fájlokat és változásokat lát; bounded bejárás.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test vfs_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P04` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `WS-01` implemented/tested állapotúnak.

### WS-10 — Több gyökér és több projekt (P3, L)

**Kiindulás:** Runtime egy gyökérhez kötött.

**Módosítás:** Workspace több elkülönített projekt-runtime-mal; explicit scope és jogosultsági határ.

**Valódi bekötés előtt:** WS-01 (P04), MCP-03 (P03).

- [ ] `ws_10` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Azonos fájlnév két rootban nem ütközik; agent/build mindig azonosított projekthez kötött.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test vfs_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P04` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `WS-10` implemented/tested állapotúnak.

### OS-07 — Fájlrendszer sajátosságok (P1, M)

**Kiindulás:** Unicode/space minták vannak; case/APFS/NTFS/reparse/volume mátrix nem teljes.

**Módosítás:** Fájlazonosság és case/Unicode-normalizálás, hálózati/eltűnő volume tesztek.

**Valódi bekötés előtt:** DOC-07 (P01), DOC-10 (P02).

- [ ] `os_07` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Névütközés vagy eltűnt T9 nem veszít dirty munkát és nem ír másik fájlba.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test vfs_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P04` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `OS-07` implemented/tested állapotúnak.

## Task Z: Bekötés és átadás

- [ ] `python3 scripts/validate_development_packages.py` és `cargo +1.99.1 test -p latexide-core --test vfs_contract --locked` sikeres; a tesztparancs itt célállapot, futtatását külön rögzíteni kell.
- [ ] P00 alkalmazza a manifestben felsorolt közös hookokat; stagingben egyesével fusson a korábbi Rust/native/IPC/MCP regresszió a megváltozott rétegnek megfelelően. Nincs automatikus Save vagy új agent-edit jóváhagyás.
- [ ] Mac natív workflow valódi fókusz-/IME-/undo-eseménnyel; Windows külön MSBuild/link/launch/funkcionális kör. Linux saját runner/felület nélkül unrun. Nem natív csomagnál ezeket az adaptergazda végzi.
- [ ] `docs/development/results/P04.md` tartalmazza a teljesített ID-ket és reprodukálható bizonyítékot. P00 ennek alapján frissíti a handoffot, ledgert és a releváns skill-reference-et; más csomag ne írja párhuzamosan ezeket.

## Másolható indítófeladat

> Hajtsd végre a `docs/superpowers/plans/2026-10-31-parallel/p04-vfs.md` tervet. Előbb olvasd az AGENTS.md-t, a handoffot, a projekt-skillt, a közös csomagszerződést és a BASELINE_DELTA-t. Csak P04 saját útvonalait módosítsd, a közös fájlokhoz integration-manifestet adj. Őrizd meg a közvetlen agent-edit + explicit Save + Save után is elérhető undo szerződést. A követelményfüggőséget és platformi bizonyítékot ellenőrizd; planned/unrun állapotot ne nevezd késznek.
