# P09 — Konzisztens fordítás és diagnosztika Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Valódi, teljes függőségpillanatképhez kötött PDF-et és megbízható compilerfolyamatot ad.

**Architecture:** BuildManifest a dirty és unopened inputokat is azonosítja; immutable stagingből fordítás, PDF-validálás és generation-státusz külön lépés. A régi supervisor folyamatkezelési tapasztalatai megmaradnak.

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

- R1: Exit0 nem-PDF output ne váljon sikeres builddé.
- R2: Unopened függőség, bib, kép vagy font változása stale állapotot adjon.
- R3: Fordítás alatti szerkesztés ne keveredjen az adott generation forrásai közé.
- R4: Timeout/cancel az egész folyamatfát lezárja és nem tünteti el a régi megtekinthető PDF-et.
- R5: Hiányzó csomag vagy bibliográfiai második pass ne látszódjon kész kiadványnak.

---

## Tulajdon és indulás

**Branch:** `feature/p09-build`. **Profil:** core.

**Kizárólagosan létrehozható/módosítható útvonalak:**

- `crates/latexide-build/src/pipeline/`
- `crates/latexide-build/tests/pipeline_contract.rs`
- `integration/requests/P09.json`
- `docs/development/results/P09.md`

**Teszt:** `crates/latexide-build/tests/pipeline_contract.rs`. **Eredménynapló:** `docs/development/results/P09.md`.

**Olvasandó jelenlegi belépési pontok:** `crates/latexide-build/src/lib.rs`, `crates/latexide-ffi/src/runtime.rs`, `tests/compiler_runtime_contract.py`.

Indulás: P00 contract-v1 után, fagyasztott fixture-ökkel és fake portokkal; a többi csomag teljes elkészülte nem indulási feltétel.

Integrációs szomszédok: P03, P04, P08, P10, P18, P19, P21. Ezek **nem csomagszintű topologikus sorrend**: az egyedi követelményfüggőségek maradnak irányadók. A kölcsönös portfüggést fixture és fokozatos bekötés oldja, nem két teljes csomag egymásra várása.

## Task 1: Önállóan ellenőrizhető szerződésszelet

**Create:** `crates/latexide-build/src/pipeline/mod.rs`, `crates/latexide-build/tests/pipeline_contract.rs`. **Consumes / produces:**

```rust
pub fn evaluate_artifact(manifest:&BuildManifest, current:&WorkspaceSnapshot, pdf:&[u8]) -> Result<ArtifactState,ArtifactError>;
// BuildManifest { generation:String, snapshot_id:String, inputs:Vec<BuildInput>, compiler_sha256:String }; BuildInput { path:String, content_sha256:String }; ArtifactState::{Current,Stale}; ArtifactError::{InvalidPdf,MissingInput,InconsistentSnapshot}
```

Az itt felsorolt DTO-mezőket P00 helyezi a `crates/latexide-contracts/src/` moduljaiba, serde serialize/deserialize és egyértelmű hiba-enummal. A szolgáltatás a saját modulban marad; ne hozz létre protocol→core→protocol függőségi kört. A port v1 helyi Rust-határ, nem automatikus új IPC-eszköz.

- [ ] **RED:** `crates/latexide-build/tests/pipeline_contract.rs` alatt `contract_v1_first_slice` néven rögzítsd ezt az ellenőrzést: **Exit0 és öt nem-PDF byte InvalidPdf; unopened chapter.tex változása mellett a korábbi valódi PDF Stale.**
- [ ] Futtasd: `cargo +1.99.1 test -p latexide-build --test pipeline_contract --locked`. Először a hiányzó szolgáltatásra vagy állított viselkedésre FAIL legyen; környezet-/linkhibát ne könyvelj reprodukciónak.
- [ ] Valósítsd meg az első portot `crates/latexide-build/src/pipeline/mod.rs` alatt; az alkalmazás-regisztrációt add át `integration/requests/P09.json` manifestben (schema, package_id, contract_version, requirement_ids, source_paths, hook_targets, test_commands, compatibility).
- [ ] Ugyanazzal a paranccsal legyen GREEN; tedd mellé R1–R5 célzott regresszióját a `review_focus_r1` … `review_focus_r5` tesztekbe. Az állítások a fenti öt elvárt viselkedést vizsgálják.
- [ ] Ellenőrizd a saját diffet, majd készíts kis commitot: `git add -- crates/latexide-build/src/pipeline/mod.rs crates/latexide-build/tests/pipeline_contract.rs integration/requests/P09.json` és `git commit -m "feat(p09): establish tested service slice"`. A dokumentált későbbi munkavégzéskor commitolj; ez a terv most nem futtatja ezeket.

## Követelményenkénti feladatsor

Minden alábbi sor külön tesztelhető részeredmény és saját kis commit. A teszt neve az ID kisbetűs, aláhúzásos alakja; az elfogadás mondata a kötelező assertion. A dependency provider mockja csak izolált teszthez elég; release-hez a valódi providerrel is teljesüljön.

### BLD-01 — PDF-frissesség teljes függőségekre (P0, L)

**Kiindulás:** Meg nem nyitott input változása után stale=false, OBS-06.

**Módosítás:** BuildManifest minden tényleges input content hashével; watcher/újraellenőrzés az artifact státuszához.

**Valódi bekötés előtt:** WS-01 (P04).

- [ ] `bld_01` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Külső chapter/bib/style/kép változása elavultnak jelöli a hozzá tartozó PDF-et.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-build --test pipeline_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P09` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `BLD-01` implemented/tested állapotúnak.

### BLD-02 — Konzisztens build inputpillanatkép (P0, L)

**Kiindulás:** Fordító élő lemezforrást olvas; csak nyitott revíziók vannak rögzítve.

**Módosítás:** Buildhez stabil input snapshot vagy start/end hash-ellenőrzés és explicit changed-during-build állapot.

**Valódi bekötés előtt:** BLD-01 (P09).

- [ ] `bld_02` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Build közben változó fájl esetén nincs téves current állítás; build újraindítható.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-build --test pipeline_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P09` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `BLD-02` implemented/tested állapotúnak.

### BLD-03 — Valódi PDF-validálás (P1, M)

**Kiindulás:** Öt nem-PDF byte is succeeded, OBS-05.

**Módosítás:** PDF header és parseres megnyithatóság/oldalszám ellenőrzése, típusos artifact-hiba.

**Valódi bekötés előtt:** nincs.

- [ ] `bld_03` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Sérült, üres vagy nem-PDF kimenet failed; korábbi megnyitható PDF megőrzött.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-build --test pipeline_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P09` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `BLD-03` implemented/tested állapotúnak.

### BLD-04 — Főfájl és buildprofil per projekt (P1, M)

**Kiindulás:** main.tex/első .tex fallback; backend motorokat tud, UI nem teljes.

**Módosítás:** Verziózott projektprofil: főfájl, engine, output és ellenőrzött recept.

**Valódi bekötés előtt:** WS-08 (P08).

- [ ] `bld_04` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Két fődokumentum és Xe/Lua/pdfLaTeX választás újranyitás után megmarad.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-build --test pipeline_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P09` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `BLD-04` implemented/tested állapotúnak.

### BLD-05 — Strukturált és végső-pass diagnosztika (P1, L)

**Kiindulás:** Naplósor-parser, max 200 találat; korai rerun warning fennmaradhat.

**Módosítás:** Pass-aware log feldolgozás file/range/code/owner/build id mezőkkel és deduplikációval.

**Valódi bekötés előtt:** BLD-01 (P09).

- [ ] `bld_05` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Végül feloldott ref-warning nem marad aktív; hibás sor továbbra is navigálható.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-build --test pipeline_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P09` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `BLD-05` implemented/tested állapotúnak.

### BLD-06 — Automatikus build és debounce (P1, M)

**Kiindulás:** Manuális build/cancel van.

**Módosítás:** Opcionális on-save/idle build, latest-wins ütemezés és függőség szerinti invalidálás.

**Valódi bekötés előtt:** BLD-01 (P09), BLD-04 (P09).

- [ ] `bld_06` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Gyors húsz mentésből korlátos build lesz; régi artifact nem írja felül az új státuszt.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-build --test pipeline_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P09` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `BLD-06` implemented/tested állapotúnak.

### BLD-07 — Bibliográfia és index toolfolyamat (P1, M)

**Kiindulás:** latexmk fut; BibTeX/Biber/makeindex/xindy készlet nem teljesen kvalifikált.

**Módosítás:** Deklarált recipe/toolcsomag és külön corpus minden támogatott segédprogramra.

**Valódi bekötés előtt:** BLD-04 (P09), QA-02 (P19).

- [ ] `bld_07` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Biblatex+Biber, natbib+BibTeX, index és glossary minták offline reprodukálhatók.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-build --test pipeline_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P09` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `BLD-07` implemented/tested állapotúnak.

### BLD-08 — Csomaghiány felismerése és javítása (P1, M)

**Kiindulás:** Kis verified TeX Live profil van; teljes sablonkorpusz nincs.

**Módosítás:** Missing package/font diagnosztika, verziózott csomagfeloldás és tudatos helyi telepítési művelet.

**Valódi bekötés előtt:** BLD-05 (P09).

- [ ] `bld_08` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Hiány pontosan azonosítható; nincs háttérben észrevétlen hálózati/fordítócsere.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-build --test pipeline_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P09` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `BLD-08` implemented/tested állapotúnak.

### BLD-09 — Reprodukálható compilerkörnyezet (P1, L)

**Kiindulás:** Gyökércsomagok/revíziók rögzítettek; teljes repository nincs fagyasztva.

**Módosítás:** TeX runtime snapshot, csomagmanifest, fontok és buildprofil az artifact mellé.

**Valódi bekötés előtt:** BLD-07 (P09), SEC-04 (P18).

- [ ] `bld_09` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Friss gépen a deklarált környezet újraépíti a dokumentumot; eltérés reportolt.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-build --test pipeline_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P09` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `BLD-09` implemented/tested állapotúnak.

### BLD-10 — Build-artifact megőrzés és takarítás (P2, M)

**Kiindulás:** Nyolc statusrekord, lemezes generációk automatikus kvótája nincs.

**Módosítás:** Byte-/idő-/darabkeret; aktív PDF és SyncTeX generáció referenciája védett.

**Valódi bekötés előtt:** PDF-03 (P10).

- [ ] `bld_10` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Száz build után korlátos lemezhasználat; megnyitott/visszanézett generáció nem tűnik el.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-build --test pipeline_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P09` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `BLD-10` implemented/tested állapotúnak.

### BLD-11 — Progress és növekményes buildlog (P2, M)

**Kiindulás:** Állapotok léteznek, bounded log snapshotot pollol a GUI.

**Módosítás:** Log-offset/esemény API, pass/tool állapot és felhasználói progress.

**Valódi bekötés előtt:** MCP-02 (P03).

- [ ] `bld_11` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Hosszú napló nem küldi újra a teljes előnézetet; cancel után végállapot egyértelmű.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-build --test pipeline_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P09` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `BLD-11` implemented/tested állapotúnak.

### BLD-12 — Folyamatfa megszakítása minden célon (P1, M)

**Kiindulás:** Mac kontrollált processztesztek vannak; Windows Job Object futtatás nincs.

**Módosítás:** Cancel/timeout/quit stressz és Windows indulás/Job-attach rés kvalifikáció.

**Valódi bekötés előtt:** OS-01 (P21), QA-01 (P19).

- [ ] `bld_12` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Azonnal forkoló és pipe-ot tartó child sem marad futva; app kilépése korlátos.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-build --test pipeline_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P09` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `BLD-12` implemented/tested állapotúnak.

### BLD-13 — Külső TeX-disztribúció támogatása (P2, M)

**Kiindulás:** Bundled TeX Live az út; compiler configure natív API létezik.

**Módosítás:** Kiválasztott verified rendszer-TeX integráció, doctor és profilettől elkülönített eredet.

**Valódi bekötés előtt:** BLD-04 (P09).

- [ ] `bld_13` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Hiányzó engine/PATH/font pontos hiba; bundled és külső környezet nem keveredik.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-build --test pipeline_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P09` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `BLD-13` implemented/tested állapotúnak.

### BLD-14 — Egyéni, korlátozott buildreceptek (P2, L)

**Kiindulás:** Fix argumentumok és -norc/-no-shell-escape; egyéni pipeline nincs.

**Módosítás:** Deklaratív tool-argv/profilmodell; külön explicit policy kódot futtató receptekhez.

**Valódi bekötés előtt:** SEC-01 (P18), WS-08 (P08).

- [ ] `bld_14` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Szóközös path argvban megőrzött; nem megbízható projekt nem kapcsolhat be futtatási jogot.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-build --test pipeline_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P09` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `BLD-14` implemented/tested állapotúnak.

### BLD-15 — Builderedmények összehasonlítása és exportja (P2, M)

**Kiindulás:** Egy PDF-előnézet és log látható.

**Módosítás:** Generációválasztó, source/toolchain/profil manifest export és reproducibility report.

**Valódi bekötés előtt:** BLD-09 (P09), PDF-03 (P10).

- [ ] `bld_15` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Megosztott reportból az inputok/verziók megállapíthatók; session-token nincs benne.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-build --test pipeline_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P09` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `BLD-15` implemented/tested állapotúnak.

### BLD-16 — Környezeti útvonal- és fontkorpusz (P1, M)

**Kiindulás:** Néhány Unicode/multifile minta és whitespace-javítás tesztelt.

**Módosítás:** Szóköz/ékezet/hosszú path/read-only/network-volume/fontset mátrix a támogatott célokon.

**Valódi bekötés előtt:** QA-02 (P19), OS-01 (P21).

- [ ] `bld_16` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Támogatási lista konkrét PASS/FAIL/unrun eredménnyel, váratlan globális telepítés nélkül.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-build --test pipeline_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P09` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `BLD-16` implemented/tested állapotúnak.

## Task Z: Bekötés és átadás

- [ ] `python3 scripts/validate_development_packages.py` és `cargo +1.99.1 test -p latexide-build --test pipeline_contract --locked` sikeres; a tesztparancs itt célállapot, futtatását külön rögzíteni kell.
- [ ] P00 alkalmazza a manifestben felsorolt közös hookokat; stagingben egyesével fusson a korábbi Rust/native/IPC/MCP regresszió a megváltozott rétegnek megfelelően. Nincs automatikus Save vagy új agent-edit jóváhagyás.
- [ ] Mac natív workflow valódi fókusz-/IME-/undo-eseménnyel; Windows külön MSBuild/link/launch/funkcionális kör. Linux saját runner/felület nélkül unrun. Nem natív csomagnál ezeket az adaptergazda végzi.
- [ ] `docs/development/results/P09.md` tartalmazza a teljesített ID-ket és reprodukálható bizonyítékot. P00 ennek alapján frissíti a handoffot, ledgert és a releváns skill-reference-et; más csomag ne írja párhuzamosan ezeket.

## Másolható indítófeladat

> Hajtsd végre a `docs/superpowers/plans/2026-10-31-parallel/p09-build.md` tervet. Előbb olvasd az AGENTS.md-t, a handoffot, a projekt-skillt, a közös csomagszerződést és a BASELINE_DELTA-t. Csak P09 saját útvonalait módosítsd, a közös fájlokhoz integration-manifestet adj. Őrizd meg a közvetlen agent-edit + explicit Save + Save után is elérhető undo szerződést. A követelményfüggőséget és platformi bizonyítékot ellenőrizd; planned/unrun állapotot ne nevezd késznek.
