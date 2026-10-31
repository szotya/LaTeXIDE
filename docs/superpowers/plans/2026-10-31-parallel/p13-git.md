# P13 — Git, diff, merge és agentváltozások Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A munkakönyvtár, index, HEAD és agentváltozások átlátható, biztonságos verziókezelését adja.

**Architecture:** Argumentumtömbös Git-adapter és immutable diffmodellek, háromutas merge a közös tranzakcióporton. A hálózati és destruktív műveletek külön felhasználói parancsok.

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

- R1: Dirty IDE-buffer és más lemezverzió diffje ne legyen összekeverve.
- R2: Sortöréses/Unicode fájlnév és átnevezés ne sértse a parserargumentumokat.
- R3: Merge/rebase/stash közben fennálló konfliktus ne vesszen el újabb parancstól.
- R4: Credential ne jelenjen meg URL-ben, logban vagy support bundle-ben.
- R5: Push/pull/reset ne fusson puszta projektmegnyitás vagy agentváltozás miatt.

---

## Tulajdon és indulás

**Branch:** `feature/p13-git`. **Profil:** core.

**Kizárólagosan létrehozható/módosítható útvonalak:**

- `crates/latexide-core/src/services/git/`
- `crates/latexide-core/tests/git_service_contract.rs`
- `integration/requests/P13.json`
- `docs/development/results/P13.md`

**Teszt:** `crates/latexide-core/tests/git_service_contract.rs`. **Eredménynapló:** `docs/development/results/P13.md`.

**Olvasandó jelenlegi belépési pontok:** `crates/latexide-core/src/workspace.rs`, `apps/macos/Sources/LaTeXIDEKit/WorkspaceStore.swift`.

Indulás: P00 contract-v1 után, fagyasztott fixture-ökkel és fake portokkal; a többi csomag teljes elkészülte nem indulási feltétel.

Integrációs szomszédok: P01, P02, P03, P04, P11, P18. Ezek **nem csomagszintű topologikus sorrend**: az egyedi követelményfüggőségek maradnak irányadók. A kölcsönös portfüggést fixture és fokozatos bekötés oldja, nem két teljes csomag egymásra várása.

## Task 1: Önállóan ellenőrizhető szerződésszelet

**Create:** `crates/latexide-core/src/services/git/mod.rs`, `crates/latexide-core/tests/git_service_contract.rs`. **Consumes / produces:**

```rust
pub fn parse_status_v2(bytes:&[u8]) -> Result<GitStatus,GitError>;
// GitStatus { branch:Option<String>, entries:Vec<GitEntry> }; GitEntry { path:Vec<u8>, old_path:Option<Vec<u8>>, index_state:char, worktree_state:char }; GitError::{InvalidOutput,NotRepository,CommandFailed,Cancelled}
```

Az itt felsorolt DTO-mezőket P00 helyezi a `crates/latexide-contracts/src/` moduljaiba, serde serialize/deserialize és egyértelmű hiba-enummal. A szolgáltatás a saját modulban marad; ne hozz létre protocol→core→protocol függőségi kört. A port v1 helyi Rust-határ, nem automatikus új IPC-eszköz.

- [ ] **RED:** `crates/latexide-core/tests/git_service_contract.rs` alatt `contract_v1_first_slice` néven rögzítsd ezt az ellenőrzést: **NUL-delimited Git státusz szóközt és sortörést tartalmazó nevet is egy fájlként őriz, index és worktree külön állapot.**
- [ ] Futtasd: `cargo +1.99.1 test -p latexide-core --test git_service_contract --locked`. Először a hiányzó szolgáltatásra vagy állított viselkedésre FAIL legyen; környezet-/linkhibát ne könyvelj reprodukciónak.
- [ ] Valósítsd meg az első portot `crates/latexide-core/src/services/git/mod.rs` alatt; az alkalmazás-regisztrációt add át `integration/requests/P13.json` manifestben (schema, package_id, contract_version, requirement_ids, source_paths, hook_targets, test_commands, compatibility).
- [ ] Ugyanazzal a paranccsal legyen GREEN; tedd mellé R1–R5 célzott regresszióját a `review_focus_r1` … `review_focus_r5` tesztekbe. Az állítások a fenti öt elvárt viselkedést vizsgálják.
- [ ] Ellenőrizd a saját diffet, majd készíts kis commitot: `git add -- crates/latexide-core/src/services/git/mod.rs crates/latexide-core/tests/git_service_contract.rs integration/requests/P13.json` és `git commit -m "feat(p13): establish tested service slice"`. A dokumentált későbbi munkavégzéskor commitolj; ez a terv most nem futtatja ezeket.

## Követelményenkénti feladatsor

Minden alábbi sor külön tesztelhető részeredmény és saját kis commit. A teszt neve az ID kisbetűs, aláhúzásos alakja; az elfogadás mondata a kötelező assertion. A dependency provider mockja csak izolált teszthez elég; release-hez a valódi providerrel is teljesüljön.

### EDT-13 — Diff és merge szerkesztő (P2, M)

**Kiindulás:** Csak normál forrásszerkesztő van.

**Módosítás:** Két-/háromutas natív diffnézet közös változásmodellel és hozzáférhető szöveges móddal.

**Valódi bekötés előtt:** DOC-08 (P02), DOC-01 (P01).

- [ ] `edt_13` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Konfliktusos merge újraindítás után folytatható; eredmény a Rust dokumentumba kerül.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test git_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P13` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `EDT-13` implemented/tested állapotúnak.

### GIT-01 — Git státusz és ágjelzés (P1, M)

**Kiindulás:** Nincs Git backend; a vizsgált projekt sem Git-repository.

**Módosítás:** Közös Git szolgáltatás repository discoveryvel és file-state eseményekkel.

**Valódi bekötés előtt:** WS-01 (P04), MCP-02 (P03).

- [ ] `git_01` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Nem-Git projekt is működik; külső CLI-módosítás frissíti a státuszt.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test git_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P13` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `GIT-01` implemented/tested állapotúnak.

### GIT-02 — Working tree/index/HEAD diff (P1, M)

**Kiindulás:** Nincs VCS diffnézet.

**Módosítás:** Dirty buffer, disk, index és HEAD külön összehasonlítási forrás.

**Valódi bekötés előtt:** GIT-01 (P13), EDT-13 (P13).

- [ ] `git_02` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **A diff jelzi, melyik állapotot mutatja; agent nem mentett változása is látható.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test git_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P13` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `GIT-02` implemented/tested állapotúnak.

### GIT-03 — Stage/unstage hunk és commit (P2, M)

**Kiindulás:** Nincs staging vagy commit UI.

**Módosítás:** Fájl/hunk staging, commitellenőrzés és pontos helyi eredményállapot.

**Valódi bekötés előtt:** GIT-02 (P13).

- [ ] `git_03` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Részleges staging nem nyeli el a nem mentett buffer szerkesztését.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test git_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P13` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `GIT-03` implemented/tested állapotúnak.

### GIT-04 — Branch, tag és worktree (P2, M)

**Kiindulás:** Nincs Git műveleti workflow.

**Módosítás:** Branch/tag/worktree lista és tudatos checkout/switch a dirty-védelemmel.

**Valódi bekötés előtt:** GIT-01 (P13), DOC-07 (P01).

- [ ] `git_04` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Ágváltás előtt nincs csendes adatvesztés; új manifest/index/PDF-stale frissül.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test git_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P13` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `GIT-04` implemented/tested állapotúnak.

### GIT-05 — Merge konfliktusfeloldás (P1, L)

**Kiindulás:** Nincs háromutas merge workflow.

**Módosítás:** Base/ours/theirs nézet, konfliktuslista és folytatható megoldás.

**Valódi bekötés előtt:** GIT-02 (P13), EDT-13 (P13), DOC-01 (P01).

- [ ] `git_05` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Minden konfliktus feloldott vagy nyitottnak jelölt; abort visszaállítható.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test git_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P13` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `GIT-05` implemented/tested állapotúnak.

### GIT-06 — Commit/file history, blame és timeline (P2, M)

**Kiindulás:** Nincs Git history vagy blame.

**Módosítás:** Fájlelőzmény, annotáció és verziómegnyitás readonly snapshotként.

**Valódi bekötés előtt:** GIT-01 (P13), DOC-05 (P01).

- [ ] `git_06` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Rename-történet és Unicode path megjelenik; historical snapshot nem dirty dokumentum.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test git_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P13` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `GIT-06` implemented/tested állapotúnak.

### GIT-07 — Fetch/pull/push és hitelesítés (P2, M)

**Kiindulás:** Nincs remote Git.

**Módosítás:** Natív credential-helper integráció, progress és konfliktusos állapotkezelés.

**Valódi bekötés előtt:** GIT-03 (P13), GIT-05 (P13).

- [ ] `git_07` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Kulcs nem naplózott; művelet eredménye a valós remote/exit állapotból származik.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test git_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P13` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `GIT-07` implemented/tested állapotúnak.

### GIT-08 — Stash és shelve (P2, M)

**Kiindulás:** Nincs félretételi workflow.

**Módosítás:** Dirty bufferre is értelmezett shelve/stash és kontrollált visszaalkalmazás.

**Valódi bekötés előtt:** GIT-04 (P13), DOC-08 (P02).

- [ ] `git_08` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Másik ág változásával ütköző visszahozás nem veszít egyik változatból sem.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test git_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P13` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `GIT-08` implemented/tested állapotúnak.

### GIT-09 — PR és code review integráció (P3, M)

**Kiindulás:** Nincs szolgáltatóadapter.

**Módosítás:** Opcionális GitHub/GitLab review provider, diff és kommentkapcsolat.

**Valódi bekötés előtt:** GIT-02 (P13), SEC-06 (P18).

- [ ] `git_09` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Fájlsor mapping és commitazonosság helyes; szolgáltató hiányában a helyi munka folytatható.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test git_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P13` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `GIT-09` implemented/tested állapotúnak.

### GIT-10 — LaTeX project ignore és artifact policy (P2, M)

**Kiindulás:** Nincs termékbeli VCS-beállítás.

**Módosítás:** Sablonalapú ignore-ajánlás és generált/forrás megkülönböztetés, explicit alkalmazással.

**Valódi bekötés előtt:** GIT-01 (P13), RES-02 (P11).

- [ ] `git_10` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **PDF/cache/autogen állapot érthető; felhasználói ignore nincs önkényesen felülírva.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test git_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P13` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `GIT-10` implemented/tested állapotúnak.

### GIT-11 — Rebase és cherry-pick állapotok (P2, L)

**Kiindulás:** Nincs fejlett VCS művelet.

**Módosítás:** Folyamatmodell continue/abort és hibatűrő újranyitás támogatással.

**Valódi bekötés előtt:** GIT-05 (P13), DOC-01 (P01).

- [ ] `git_11` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Elakadt művelet újraindítás után azonosítható és folytatható; dirty munka megmarad.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test git_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P13` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `GIT-11` implemented/tested állapotúnak.

### GIT-12 — Agentváltozások helyi áttekintése (P1, M)

**Kiindulás:** Eredet/transaction_id van; változáslista és diffrészlet nincs.

**Módosítás:** User/agent tranzakciók diffje, fájllista és opcionális utólagos revert.

**Valódi bekötés előtt:** DOC-08 (P02), EDT-13 (P13).

- [ ] `git_12` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Közvetlen agentedit azonnali marad; áttekintés nem vezet be szerkesztésenkénti approvalt.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test git_service_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P13` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `GIT-12` implemented/tested állapotúnak.

## Task Z: Bekötés és átadás

- [ ] `python3 scripts/validate_development_packages.py` és `cargo +1.99.1 test -p latexide-core --test git_service_contract --locked` sikeres; a tesztparancs itt célállapot, futtatását külön rögzíteni kell.
- [ ] P00 alkalmazza a manifestben felsorolt közös hookokat; stagingben egyesével fusson a korábbi Rust/native/IPC/MCP regresszió a megváltozott rétegnek megfelelően. Nincs automatikus Save vagy új agent-edit jóváhagyás.
- [ ] Mac natív workflow valódi fókusz-/IME-/undo-eseménnyel; Windows külön MSBuild/link/launch/funkcionális kör. Linux saját runner/felület nélkül unrun. Nem natív csomagnál ezeket az adaptergazda végzi.
- [ ] `docs/development/results/P13.md` tartalmazza a teljesített ID-ket és reprodukálható bizonyítékot. P00 ennek alapján frissíti a handoffot, ledgert és a releváns skill-reference-et; más csomag ne írja párhuzamosan ezeket.

## Másolható indítófeladat

> Hajtsd végre a `docs/superpowers/plans/2026-10-31-parallel/p13-git.md` tervet. Előbb olvasd az AGENTS.md-t, a handoffot, a projekt-skillt, a közös csomagszerződést és a BASELINE_DELTA-t. Csak P13 saját útvonalait módosítsd, a közös fájlokhoz integration-manifestet adj. Őrizd meg a közvetlen agent-edit + explicit Save + Save után is elérhető undo szerződést. A követelményfüggőséget és platformi bizonyítékot ellenőrizd; planned/unrun állapotot ne nevezd késznek.
