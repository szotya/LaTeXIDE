# P02 — Atomi fájlműveletek és szerkesztési tranzakciók Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Egységes, visszavonható többfájlos szerkesztést, fájlműveleteket és tartós mentést ad.

**Architecture:** Tranzakciótervező előbb minden revíziót/útvonalat validál, majd egy egységben alkalmaz. A lemezmentés külön commitfázis; natív Save As és gépelési undo-csoport ugyanarra a Rust-magra támaszkodik.

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

- R1: Második fájl validálási hibája ne hagyja az elsőt részben módosítva.
- R2: Emoji közepére mutató UTF-16 range és átfedő edit legyen explicit hiba.
- R3: Mentés után az undo maradjon működő és újra menthető.
- R4: Szóközös, Unicode vagy symlinkkel kicserélt cél útvonala ne vezessen idegen fájl felülírásához.
- R5: Hosszú gépelés undo-csoportja ne törölje észrevétlenül az előző logikai szerkesztés visszavonhatóságát.

---

## Tulajdon és indulás

**Branch:** `feature/p02-transactions`. **Profil:** core.

**Kizárólagosan létrehozható/módosítható útvonalak:**

- `crates/latexide-core/src/services/transactions/`
- `crates/latexide-core/tests/transaction_contract.rs`
- `integration/requests/P02.json`
- `docs/development/results/P02.md`

**Teszt:** `crates/latexide-core/tests/transaction_contract.rs`. **Eredménynapló:** `docs/development/results/P02.md`.

**Olvasandó jelenlegi belépési pontok:** `crates/latexide-core/src/document.rs`, `crates/latexide-core/src/workspace.rs`, `tests/ffi_contract.c`.

Indulás: P00 contract-v1 után, fagyasztott fixture-ökkel és fake portokkal; a többi csomag teljes elkészülte nem indulási feltétel.

Integrációs szomszédok: P01, P03, P19, P23. Ezek **nem csomagszintű topologikus sorrend**: az egyedi követelményfüggőségek maradnak irányadók. A kölcsönös portfüggést fixture és fokozatos bekötés oldja, nem két teljes csomag egymásra várása.

## Task 1: Önállóan ellenőrizhető szerződésszelet

**Create:** `crates/latexide-core/src/services/transactions/mod.rs`, `crates/latexide-core/tests/transaction_contract.rs`. **Consumes / produces:**

```rust
pub fn prepare_transaction(snapshot: &WorkspaceSnapshot, edits: Vec<RevisionedEdit>) -> Result<PreparedTransaction, TransactionError>;
// RevisionedEdit { document_id:String, expected_revision:u64, start_utf16:u32, length_utf16:u32, replacement:String }; PreparedTransaction { transaction_id:String, edits:Vec<RevisionedEdit>, base_snapshot:String }; TransactionError::{StaleRevision,InvalidRange,Overlap,OutsideRoot,Quota}
```

Az itt felsorolt DTO-mezőket P00 helyezi a `crates/latexide-contracts/src/` moduljaiba, serde serialize/deserialize és egyértelmű hiba-enummal. A szolgáltatás a saját modulban marad; ne hozz létre protocol→core→protocol függőségi kört. A port v1 helyi Rust-határ, nem automatikus új IPC-eszköz.

- [ ] **RED:** `crates/latexide-core/tests/transaction_contract.rs` alatt `contract_v1_first_slice` néven rögzítsd ezt az ellenőrzést: **Két fájlból a második stale revíziója az egész batch-et elutasítja; az első tartalma és undo-ja változatlan.**
- [ ] Futtasd: `cargo +1.99.1 test -p latexide-core --test transaction_contract --locked`. Először a hiányzó szolgáltatásra vagy állított viselkedésre FAIL legyen; környezet-/linkhibát ne könyvelj reprodukciónak.
- [ ] Valósítsd meg az első portot `crates/latexide-core/src/services/transactions/mod.rs` alatt; az alkalmazás-regisztrációt add át `integration/requests/P02.json` manifestben (schema, package_id, contract_version, requirement_ids, source_paths, hook_targets, test_commands, compatibility).
- [ ] Ugyanazzal a paranccsal legyen GREEN; tedd mellé R1–R5 célzott regresszióját a `review_focus_r1` … `review_focus_r5` tesztekbe. Az állítások a fenti öt elvárt viselkedést vizsgálják.
- [ ] Ellenőrizd a saját diffet, majd készíts kis commitot: `git add -- crates/latexide-core/src/services/transactions/mod.rs crates/latexide-core/tests/transaction_contract.rs integration/requests/P02.json` és `git commit -m "feat(p02): establish tested service slice"`. A dokumentált későbbi munkavégzéskor commitolj; ez a terv most nem futtatja ezeket.

## Követelményenkénti feladatsor

Minden alábbi sor külön tesztelhető részeredmény és saját kis commit. A teszt neve az ID kisbetűs, aláhúzásos alakja; az elfogadás mondata a kötelező assertion. A dependency provider mockja csak izolált teszthez elég; release-hez a valódi providerrel is teljesüljön.

### DOC-04 — Gépelés összevont undo-csoportokkal (P1, M)

**Kiindulás:** 300 külön editből 256 vonható vissza, 44 karakter marad, OBS-02.

**Módosítás:** Idő-/kurzor-/eredetalapú gépelési csoportok; agenttranzakció külön határ.

**Valódi bekötés előtt:** nincs.

- [ ] `doc_04` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Egy szó gépelése természetesen vonható vissza; user/agent/save/IME határok következetesek.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test transaction_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P02` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `DOC-04` implemented/tested állapotúnak.

### DOC-06 — Fájl létrehozása és Save As (P1, M)

**Kiindulás:** Az API már létező fájlt canonicalize-ol; nincs create/save-as művelet.

**Módosítás:** Közös, gyökéren belüli create/save-as tranzakció és natív párbeszéd.

**Valódi bekötés előtt:** MCP-03 (P03).

- [ ] `doc_06` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Ütköző név, ékezet, csak olvasható cél és megszakítás adatvesztés nélkül működik.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test transaction_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P02` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `DOC-06` implemented/tested állapotúnak.

### DOC-08 — Többfájlos atomi szerkesztés (P1, L)

**Kiindulás:** apply_edits egyetlen dokumentumra érvényes.

**Módosítás:** WorkspaceEdit: minden revízió/útvonal validálása, egy közös memóriatranzakció és undo.

**Valódi bekötés előtt:** MCP-03 (P03), DOC-02 (P01).

- [ ] `doc_08` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Második fájl konfliktusánál egyik sem változik; tízfájlos művelet egyben visszavonható.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test transaction_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P02` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `DOC-08` implemented/tested állapotúnak.

### DOC-09 — Átnevezés, mozgatás és törlés (P1, L)

**Kiindulás:** Nincs közös fájlműveleti parancs vagy history.

**Módosítás:** Fájlazonosság a pathon túl; naplózott file-operation és visszaállítható törlés.

**Valódi bekötés előtt:** DOC-06 (P02), DOC-08 (P02), DOC-05 (P01).

- [ ] `doc_09` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Nyitott dirty fájl, hivatkozás és névütközés biztonságosan kezelhető; undo visszahozza.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test transaction_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P02` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `DOC-09` implemented/tested állapotúnak.

### DOC-10 — Mentés tartóssága és platformi csere (P1, M)

**Kiindulás:** Tempfájl, sync_all és rename van; crash/power-loss és Windows csere nincs kvalifikálva.

**Módosítás:** Platformfüggő atomic-replace, könyvtárszinkron és megszakításos mentési vizsgálat.

**Valódi bekötés előtt:** QA-01 (P19).

- [ ] `doc_10` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Hibapontoknál a régi vagy új teljes fájl marad; cél-ACL és attribútumok ellenőrzöttek.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test transaction_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P02` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `DOC-10` implemented/tested állapotúnak.

### DOC-11 — Kódolás és sorvég kezelőfelülete (P2, M)

**Kiindulás:** UTF-8/BOM és LF/CRLF megőrzött; más formátum explicit hiba.

**Módosítás:** Felismerés, previewzott felhasználói konverzió és fájlonkénti beállítás.

**Valódi bekötés előtt:** DOC-08 (P02).

- [ ] `doc_11` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **UTF-16/legacy fájl nem sérül; konverzió visszavonható és veszteséget előre jelez.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test transaction_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P02` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `DOC-11` implemented/tested állapotúnak.

### DOC-12 — Nagy fájl és dokumentumkvóta (P1, L)

**Kiindulás:** 1 MiB/fájl kezdeti korlát és String-másolások vannak.

**Módosítás:** Méretfüggő szolgáltatások és konfigurált bytekeret; mérés után indokolt tárolóváltás.

**Valódi bekötés előtt:** QA-03 (P23), MCP-02 (P03).

- [ ] `doc_12` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Nagy .bib és hosszú sor kontrolláltan nyílik; nincs UI-megállás vagy csendes csonkítás.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test transaction_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P02` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `DOC-12` implemented/tested állapotúnak.

## Task Z: Bekötés és átadás

- [ ] `python3 scripts/validate_development_packages.py` és `cargo +1.99.1 test -p latexide-core --test transaction_contract --locked` sikeres; a tesztparancs itt célállapot, futtatását külön rögzíteni kell.
- [ ] P00 alkalmazza a manifestben felsorolt közös hookokat; stagingben egyesével fusson a korábbi Rust/native/IPC/MCP regresszió a megváltozott rétegnek megfelelően. Nincs automatikus Save vagy új agent-edit jóváhagyás.
- [ ] Mac natív workflow valódi fókusz-/IME-/undo-eseménnyel; Windows külön MSBuild/link/launch/funkcionális kör. Linux saját runner/felület nélkül unrun. Nem natív csomagnál ezeket az adaptergazda végzi.
- [ ] `docs/development/results/P02.md` tartalmazza a teljesített ID-ket és reprodukálható bizonyítékot. P00 ennek alapján frissíti a handoffot, ledgert és a releváns skill-reference-et; más csomag ne írja párhuzamosan ezeket.

## Másolható indítófeladat

> Hajtsd végre a `docs/superpowers/plans/2026-10-31-parallel/p02-transactions.md` tervet. Előbb olvasd az AGENTS.md-t, a handoffot, a projekt-skillt, a közös csomagszerződést és a BASELINE_DELTA-t. Csak P02 saját útvonalait módosítsd, a közös fájlokhoz integration-manifestet adj. Őrizd meg a közvetlen agent-edit + explicit Save + Save után is elérhető undo szerződést. A követelményfüggőséget és platformi bizonyítékot ellenőrizd; planned/unrun állapotot ne nevezd késznek.
