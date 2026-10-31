# P01 — Recovery, bufferéletciklus és helyi előzmény Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Folyamatkiesés után helyreállítja a nyugtázott, el nem mentett munkát, és biztonságosan kezeli a megnyitott puffereket.

**Architecture:** A kanonikus Rust állapot változásai recovery-checkpointot és korlátos naplót kapnak. A tiszta puffer eldobható, a dirty állapot és undo nem; natív restore döntés a Rust-tartalom fölött jelenik meg.

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

- R1: Sérült utolsó naplórekordnál az utolsó teljes nyugtázott revízió álljon helyre.
- R2: Recovery közben más tartalommal létező lemezfájl ne legyen automatikusan felülírva.
- R3: Száz tiszta fájl bejárása ne veszítsen dirty tartalmat vagy undo-előzményt.
- R4: Tele lemez vagy jogosultsághiba ne jelenjen meg sikeres checkpointként.
- R5: Több IDE-session ugyanarra a projektre ne írjon közös recovery-naplóba ellenőrizetlenül.

---

## Tulajdon és indulás

**Branch:** `feature/p01-recovery`. **Profil:** core.

**Kizárólagosan létrehozható/módosítható útvonalak:**

- `crates/latexide-core/src/services/recovery/`
- `crates/latexide-core/tests/recovery_contract.rs`
- `integration/requests/P01.json`
- `docs/development/results/P01.md`

**Teszt:** `crates/latexide-core/tests/recovery_contract.rs`. **Eredménynapló:** `docs/development/results/P01.md`.

**Olvasandó jelenlegi belépési pontok:** `crates/latexide-core/src/workspace.rs`, `crates/latexide-ffi/src/runtime.rs`.

Indulás: P00 contract-v1 után, fagyasztott fixture-ökkel és fake portokkal; a többi csomag teljes elkészülte nem indulási feltétel.

Integrációs szomszédok: P02. Ezek **nem csomagszintű topologikus sorrend**: az egyedi követelményfüggőségek maradnak irányadók. A kölcsönös portfüggést fixture és fokozatos bekötés oldja, nem két teljes csomag egymásra várása.

## Task 1: Önállóan ellenőrizhető szerződésszelet

**Create:** `crates/latexide-core/src/services/recovery/mod.rs`, `crates/latexide-core/tests/recovery_contract.rs`. **Consumes / produces:**

```rust
pub fn recover(checkpoint: Checkpoint, journal: &[JournalEntry], disk: &[DiskIdentity]) -> Result<RecoverySet, RecoveryError>;
// Checkpoint { schema:u32, sequence:u64, documents:Vec<RecoveredDocument> }; JournalEntry { sequence:u64, document_id:String, revision:u64, utf8:Vec<u8>, checksum:String }
// DiskIdentity { path:String, content_sha256:String }; RecoveredDocument { document_id:String, revision:u64, utf8:Vec<u8>, dirty:bool }; RecoverySet { documents:Vec<RecoveredDocument>, last_sequence:u64 }; RecoveryError::{Corrupt,Quota,UnsupportedSchema}
```

Az itt felsorolt DTO-mezőket P00 helyezi a `crates/latexide-contracts/src/` moduljaiba, serde serialize/deserialize és egyértelmű hiba-enummal. A szolgáltatás a saját modulban marad; ne hozz létre protocol→core→protocol függőségi kört. A port v1 helyi Rust-határ, nem automatikus új IPC-eszköz.

- [ ] **RED:** `crates/latexide-core/tests/recovery_contract.rs` alatt `contract_v1_first_slice` néven rögzítsd ezt az ellenőrzést: **r7 nyugtázása utáni megszakításból r7 dirty tartalom visszajön; csonka r8 napló nem ír a lemezre.**
- [ ] Futtasd: `cargo +1.99.1 test -p latexide-core --test recovery_contract --locked`. Először a hiányzó szolgáltatásra vagy állított viselkedésre FAIL legyen; környezet-/linkhibát ne könyvelj reprodukciónak.
- [ ] Valósítsd meg az első portot `crates/latexide-core/src/services/recovery/mod.rs` alatt; az alkalmazás-regisztrációt add át `integration/requests/P01.json` manifestben (schema, package_id, contract_version, requirement_ids, source_paths, hook_targets, test_commands, compatibility).
- [ ] Ugyanazzal a paranccsal legyen GREEN; tedd mellé R1–R5 célzott regresszióját a `review_focus_r1` … `review_focus_r5` tesztekbe. Az állítások a fenti öt elvárt viselkedést vizsgálják.
- [ ] Ellenőrizd a saját diffet, majd készíts kis commitot: `git add -- crates/latexide-core/src/services/recovery/mod.rs crates/latexide-core/tests/recovery_contract.rs integration/requests/P01.json` és `git commit -m "feat(p01): establish tested service slice"`. A dokumentált későbbi munkavégzéskor commitolj; ez a terv most nem futtatja ezeket.

## Követelményenkénti feladatsor

Minden alábbi sor külön tesztelhető részeredmény és saját kis commit. A teszt neve az ID kisbetűs, aláhúzásos alakja; az elfogadás mondata a kötelező assertion. A dependency provider mockja csak izolált teszthez elég; release-hez a valódi providerrel is teljesüljön.

### DOC-01 — Nem mentett munka helyreállítása (P0, L)

**Kiindulás:** A runtime újranyitáskor a lemezt tölti; nincs recovery, OBS-03.

**Módosítás:** Verziózott, méretkorlátos recovery napló és checkpoint; újraindításkor visszaállítási felület.

**Valódi bekötés előtt:** nincs.

- [ ] `doc_01` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Folyamatkiesés után visszajön az utolsó nyugtázott tartalom; sérült napló nem írja felül a fájlt.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test recovery_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P01` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `DOC-01` implemented/tested állapotúnak.

### DOC-02 — Valódi dokumentumzárás és memória-keret (P1, M)

**Kiindulás:** Tíz megőrzött dokumentum után a további olvasás is elutasított, OBS-01.

**Módosítás:** Tab, buffer és history-élettartam különválasztása; biztonságos clean-evikció és dirty-védelem.

**Valódi bekötés előtt:** DOC-01 (P01).

- [ ] `doc_02` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Száz fájl bejárható; elrejtett dirty dokumentum és undo nem vész el; limit látható.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test recovery_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P01` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `DOC-02` implemented/tested állapotúnak.

### DOC-03 — Agentolvasás editorhely fogyasztása nélkül (P1, M)

**Kiindulás:** read_document szükség esetén megnyit és lefoglal egy helyet.

**Módosítás:** Korlátos, verziózott háttérolvasás és bufferlease; dirty buffer mindig elsőbbséget kap.

**Valódi bekötés előtt:** DOC-02 (P01).

- [ ] `doc_03` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Agent száz fájlt olvas szerkesztőlimit nélkül; dirty tartalmat és stabil chunkrevíziót kap.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test recovery_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P01` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `DOC-03` implemented/tested állapotúnak.

### DOC-05 — Tartós helyi fájlelőzmény (P1, M)

**Kiindulás:** Delta undo memóriában van, 16 MiB/256 bejegyzés kerettel.

**Módosítás:** Recoverytől elkülönült időbélyeges helyi előzmény, diff és megőrzési beállítás.

**Valódi bekötés előtt:** DOC-01 (P01).

- [ ] `doc_05` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Mentett és törölt fájl kiválasztott revíziója visszaállítható; keret és törlés ellenőrzött.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test recovery_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P01` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `DOC-05` implemented/tested állapotúnak.

### DOC-07 — Külső változások és konfliktusfeloldás (P1, M)

**Kiindulás:** Buffer külső módosítás után régi/clean; mentés védetten hibázik, OBS-04.

**Módosítás:** Watcher és külön disk-baseline; reload/keep/merge felület, változásjelzés.

**Valódi bekötés előtt:** DOC-01 (P01).

- [ ] `doc_07` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Clean fájl frissülhet; dirty konfliktusnál mindkét változat megmarad és választható.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test recovery_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P01` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `DOC-07` implemented/tested állapotúnak.

### DOC-13 — Scratch és untitled dokumentum (P2, M)

**Kiindulás:** Projektgyökér és már létező fájl szükséges.

**Módosítás:** Memóriában induló dokumentum explicit Save As-szal és recoveryvel.

**Valódi bekötés előtt:** DOC-01 (P01), DOC-06 (P02).

- [ ] `doc_13` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Új névtelen szöveg újraindítás után is elérhető; mentés nem veszít előzményt.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test recovery_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P01` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `DOC-13` implemented/tested állapotúnak.

### DOC-14 — Mentés és build hibafolyamata (P1, M)

**Kiindulás:** Natív build mentést hív; hibák részben közös failure mezőbe kerülnek.

**Módosítás:** Típusos save-result, fájlonkénti hibajelzés; build csak sikeres választott policy után.

**Valódi bekötés előtt:** DOC-07 (P01).

- [ ] `doc_14` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Egy írásvédett vagy konfliktusos fájlnál látható az ok és nem indul félreérthető build.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test recovery_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P01` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `DOC-14` implemented/tested állapotúnak.

### DOC-15 — Automatikus mentés szabályozása (P2, M)

**Kiindulás:** Kifejezett mentés és build előtti Save All van.

**Módosítás:** Választható user autosave; agent explicit-save szerződés marad, recovery külön működik.

**Valódi bekötés előtt:** DOC-01 (P01), DOC-14 (P01).

- [ ] `doc_15` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Ki-/bekapcsolás és fókuszváltás következetes; agentedit önmagában nem válik lemezes mentéssé.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test recovery_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P01` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `DOC-15` implemented/tested állapotúnak.

### DOC-16 — Munkamenet- és fájltulajdonlás (P1, M)

**Kiindulás:** Egy workspace állapota védett; több apppéldány disk-konfliktussal találkozik.

**Módosítás:** Másik példány felismerése és olvasható conflict/ownership jelzés, kényszerített felülírás nélkül.

**Valódi bekötés előtt:** DOC-07 (P01).

- [ ] `doc_16` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Két saját apppéldány egy fájlon nem veszít módosítást; crash után nincs örök zárolás.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test recovery_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P01` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `DOC-16` implemented/tested állapotúnak.

### WS-02 — Munkamenet visszaállítása (P1, M)

**Kiindulás:** Megjelenési preferenciák tartósak; tab/caret/history csak memóriában.

**Módosítás:** Projekt/workspace session storage a nyitott/hidden nézetekhez és recovery-hivatkozáshoz.

**Valódi bekötés előtt:** DOC-01 (P01), DOC-02 (P01).

- [ ] `ws_02` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Normál restart után projekt, tabok, caret, viewport és el nem küldött munka visszajön.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test recovery_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P01` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `WS-02` implemented/tested állapotúnak.

## Task Z: Bekötés és átadás

- [ ] `python3 scripts/validate_development_packages.py` és `cargo +1.99.1 test -p latexide-core --test recovery_contract --locked` sikeres; a tesztparancs itt célállapot, futtatását külön rögzíteni kell.
- [ ] P00 alkalmazza a manifestben felsorolt közös hookokat; stagingben egyesével fusson a korábbi Rust/native/IPC/MCP regresszió a megváltozott rétegnek megfelelően. Nincs automatikus Save vagy új agent-edit jóváhagyás.
- [ ] Mac natív workflow valódi fókusz-/IME-/undo-eseménnyel; Windows külön MSBuild/link/launch/funkcionális kör. Linux saját runner/felület nélkül unrun. Nem natív csomagnál ezeket az adaptergazda végzi.
- [ ] `docs/development/results/P01.md` tartalmazza a teljesített ID-ket és reprodukálható bizonyítékot. P00 ennek alapján frissíti a handoffot, ledgert és a releváns skill-reference-et; más csomag ne írja párhuzamosan ezeket.

## Másolható indítófeladat

> Hajtsd végre a `docs/superpowers/plans/2026-10-31-parallel/p01-recovery.md` tervet. Előbb olvasd az AGENTS.md-t, a handoffot, a projekt-skillt, a közös csomagszerződést és a BASELINE_DELTA-t. Csak P01 saját útvonalait módosítsd, a közös fájlokhoz integration-manifestet adj. Őrizd meg a közvetlen agent-edit + explicit Save + Save után is elérhető undo szerződést. A követelményfüggőséget és platformi bizonyítékot ellenőrizd; planned/unrun állapotot ne nevezd késznek.
