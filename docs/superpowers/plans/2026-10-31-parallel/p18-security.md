# P18 — Compilerizoláció, trust és adatvédelem Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Tényleges, tesztelt fájlrendszer-/hálózathatárt és egységes titok-/sessionvédelmet ad.

**Architecture:** A trust policy explicit erőforrásdöntést hoz; compiler sandbox OS-specifikus és runtime-ban próbált, nincs fallback engedékeny módra. A közvetlen agent-edit/undo meglévő szerződése megmarad.

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

- R1: Symlink/rename versenyben az ellenőrzött fájlidentitás ne cserélődjön észrevétlenül.
- R2: Sandbox hiánya vagy indulási hibája ne vezessen korlátozatlan compilerfuttatáshoz.
- R3: Sessiondescriptor más user általi olvasása és ACL-hibája legyen tesztelt elutasítás.
- R4: Log/export/support bundle ne tartalmazzon credentialt vagy teljes tokent.
- R5: Windows child-process indulása előtt legyen érvényben a folyamatfa- és erőforráshatár.

---

## Tulajdon és indulás

**Branch:** `feature/p18-security`. **Profil:** core.

**Kizárólagosan létrehozható/módosítható útvonalak:**

- `crates/latexide-core/src/services/security/`
- `crates/latexide-core/tests/security_policy_contract.rs`
- `integration/requests/P18.json`
- `docs/development/results/P18.md`

**Teszt:** `crates/latexide-core/tests/security_policy_contract.rs`. **Eredménynapló:** `docs/development/results/P18.md`.

**Olvasandó jelenlegi belépési pontok:** `crates/latexide-build/src/lib.rs`, `crates/latexide-ffi/src/session.rs`, `docs/compiler-assessment.md`.

Indulás: P00 contract-v1 után, fagyasztott fixture-ökkel és fake portokkal; a többi csomag teljes elkészülte nem indulási feltétel.

Integrációs szomszédok: P01, P02, P03, P09, P14, P19, P20, P21. Ezek **nem csomagszintű topologikus sorrend**: az egyedi követelményfüggőségek maradnak irányadók. A kölcsönös portfüggést fixture és fokozatos bekötés oldja, nem két teljes csomag egymásra várása.

## Task 1: Önállóan ellenőrizhető szerződésszelet

**Create:** `crates/latexide-core/src/services/security/mod.rs`, `crates/latexide-core/tests/security_policy_contract.rs`. **Consumes / produces:**

```rust
pub fn authorize_resource(policy:&ExecutionPolicy, request:ResourceRequest) -> Result<ResourceGrant,SecurityError>;
// ExecutionPolicy { project_root:String, writable_roots:Vec<String>, network:bool, trusted:bool }; ResourceRequest { path:String, write:bool, network:bool }; ResourceGrant { canonical_path:String }; SecurityError::{OutsideRoot,NetworkDenied,Untrusted,IdentityChanged}
```

Az itt felsorolt DTO-mezőket P00 helyezi a `crates/latexide-contracts/src/` moduljaiba, serde serialize/deserialize és egyértelmű hiba-enummal. A szolgáltatás a saját modulban marad; ne hozz létre protocol→core→protocol függőségi kört. A port v1 helyi Rust-határ, nem automatikus új IPC-eszköz.

- [ ] **RED:** `crates/latexide-core/tests/security_policy_contract.rs` alatt `contract_v1_first_slice` néven rögzítsd ezt az ellenőrzést: **Compiler projektgyökéren kívüli írása és hálózatkérése elutasított; ez külön OS subprocess-próbával is igazolt.**
- [ ] Futtasd: `cargo +1.99.1 test -p latexide-core --test security_policy_contract --locked`. Először a hiányzó szolgáltatásra vagy állított viselkedésre FAIL legyen; környezet-/linkhibát ne könyvelj reprodukciónak.
- [ ] Valósítsd meg az első portot `crates/latexide-core/src/services/security/mod.rs` alatt; az alkalmazás-regisztrációt add át `integration/requests/P18.json` manifestben (schema, package_id, contract_version, requirement_ids, source_paths, hook_targets, test_commands, compatibility).
- [ ] Ugyanazzal a paranccsal legyen GREEN; tedd mellé R1–R5 célzott regresszióját a `review_focus_r1` … `review_focus_r5` tesztekbe. Az állítások a fenti öt elvárt viselkedést vizsgálják.
- [ ] Ellenőrizd a saját diffet, majd készíts kis commitot: `git add -- crates/latexide-core/src/services/security/mod.rs crates/latexide-core/tests/security_policy_contract.rs integration/requests/P18.json` és `git commit -m "feat(p18): establish tested service slice"`. A dokumentált későbbi munkavégzéskor commitolj; ez a terv most nem futtatja ezeket.

## Követelményenkénti feladatsor

Minden alábbi sor külön tesztelhető részeredmény és saját kis commit. A teszt neve az ID kisbetűs, aláhúzásos alakja; az elfogadás mondata a kötelező assertion. A dependency provider mockja csak izolált teszthez elég; release-hez a valódi providerrel is teljesüljön.

### SEC-01 — Compiler fájlrendszer- és hálózathatár (P0, L)

**Kiindulás:** env_clear és no-shell-escape van; ez nem OS sandbox.

**Módosítás:** Projektkötött compiler policy: szükséges read/write helyek és hálózat tiltása/explicit módja OS-enként.

**Valódi bekötés előtt:** BLD-04 (P09).

- [ ] `sec_01` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Saját tesztsentinel a projekten kívül nem olvasható/írható; offline build tovább működik.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test security_policy_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P18` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `SEC-01` implemented/tested állapotúnak.

### SEC-02 — Projekt trust és futtatási jogok (P1, M)

**Kiindulás:** Agent raw fájlrendszer read-only, compiler fix; általános trust modell nincs.

**Módosítás:** Projektcode-execution policy task/LSP/plugin/compiler indításhoz, edit-approval nélkül.

**Valódi bekötés előtt:** MCP-03 (P03).

- [ ] `sec_02` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Projekt megnyitása nem indít rejtett kódot; megbízható projekten direct agent edit megmarad.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test security_policy_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P18` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `SEC-02` implemented/tested állapotúnak.

### SEC-03 — Session, descriptor és ACL keményítés (P1, M)

**Kiindulás:** Mac owner/mode/token védett; Windows ACL forrás/check szintű.

**Módosítás:** Symlink/reparse/race/owner/lejárt-session és named-pipe tesztek, célplatformi jogosultságokkal.

**Valódi bekötés előtt:** OS-01 (P21), MCP-01 (P03).

- [ ] `sec_03` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Másik jogosulatlan helyi user nem csatlakozik; jogos kliens nem veszít runtime-hozzáférést.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test security_policy_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P18` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `SEC-03` implemented/tested állapotúnak.

### SEC-04 — Supply chain és SBOM (P1, M)

**Kiindulás:** Pinned eszközök, hash/GPG nyomok vannak; teljes natív audit nincs.

**Módosítás:** Rust/Swift/NuGet/TeX/binary SBOM, licenc- és advisory-monitor, hiteles frissítés.

**Valódi bekötés előtt:** QA-10 (P20).

- [ ] `sec_04` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Minden kiadott komponens verziója/eredete ismert; signature nem malware-free állítás.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test security_policy_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P18` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `SEC-04` implemented/tested állapotúnak.

### SEC-05 — Path-verseny és fájlazonosság (P1, M)

**Kiindulás:** Canonical root és mentési cél retarget ellenőrzött; minden OS-race nincs kvalifikálva.

**Módosítás:** Handle-alapú azonosság, sandbox és symlink/reparse/rename fault corpus.

**Valódi bekötés előtt:** DOC-10 (P02).

- [ ] `sec_05` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Projektcél lecserélése nem ír másik fájlt; hardlink/case/Unicode viselkedés deklarált.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test security_policy_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P18` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `SEC-05` implemented/tested állapotúnak.

### SEC-06 — Titok-, napló- és exportvédelem (P1, M)

**Kiindulás:** Compiler credential env szűrt, privát descriptor van; globális policy nincs.

**Módosítás:** Strukturált redaction, secret-free support export és opt-in hálózati provider adathatár.

**Valódi bekötés előtt:** MCP-03 (P03).

- [ ] `sec_06` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Saját sentinel nem jelenik meg log/export/assistant kontextusban, kivéve tudatos kijelölését.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test security_policy_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P18` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `SEC-06` implemented/tested állapotúnak.

### SEC-07 — Threat model és támogatási policy (P2, M)

**Kiindulás:** Konkrét provenance/határdokumentumok vannak; teljes termékmodell nincs.

**Módosítás:** Helyi projekt/TeX/PDF/plugin/agent trust boundary és támadási felület dokumentálása.

**Valódi bekötés előtt:** SEC-02 (P18).

- [ ] `sec_07` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Minden aktív futtatási felülethez tulajdonos, mitigáció és ellenőrzési bizonyíték tartozik.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test security_policy_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P18` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `SEC-07` implemented/tested állapotúnak.

### SEC-08 — Kvóták, deadline és adversarial bemenet (P1, M)

**Kiindulás:** Számos méretkorlát van; IPC stall és teljes release fuzz hiányzik.

**Módosítás:** Egységes erőforráskeretek, timeout/cancel és parser/JSON/PDF hostile corpus.

**Valódi bekötés előtt:** MCP-01 (P03), QA-09 (P19).

- [ ] `sec_08` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Egy rossz projekt vagy kliens korlátos hibát okoz, editorban mentés/recovery elérhető.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test security_policy_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P18` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `SEC-08` implemented/tested állapotúnak.

### SEC-09 — Adatmegőrzés és privát mód (P2, M)

**Kiindulás:** Vannak memóriabeli limitek; tartós history/plugin adattárolás még nincs.

**Módosítás:** Retention/delete/export/privát munkamenet policy az új helyi tárolókra.

**Valódi bekötés előtt:** DOC-05 (P01), AI-01 (P14).

- [ ] `sec_09` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Törlés hatóköre látható; érzékeny projekt nem kerül automatikus syncbe vagy telemetrybe.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test security_policy_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P18` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `SEC-09` implemented/tested állapotúnak.

### SEC-10 — Windows child-process indulási rés (P2, M)

**Kiindulás:** Job Object hozzárendelés spawn után; runtime célon nem futott.

**Módosítás:** Felfüggesztett indulás/Job attach lehetőség vizsgálata és célzott descendant stressz.

**Valódi bekötés előtt:** OS-01 (P21), BLD-12 (P09).

- [ ] `sec_10` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Azonnal childot indító compiler cancel/timeout után sem hagy utódot a kijelölt támogatott OS-en.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `cargo +1.99.1 test -p latexide-core --test security_policy_contract --locked` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P18` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `SEC-10` implemented/tested állapotúnak.

## Task Z: Bekötés és átadás

- [ ] `python3 scripts/validate_development_packages.py` és `cargo +1.99.1 test -p latexide-core --test security_policy_contract --locked` sikeres; a tesztparancs itt célállapot, futtatását külön rögzíteni kell.
- [ ] P00 alkalmazza a manifestben felsorolt közös hookokat; stagingben egyesével fusson a korábbi Rust/native/IPC/MCP regresszió a megváltozott rétegnek megfelelően. Nincs automatikus Save vagy új agent-edit jóváhagyás.
- [ ] Mac natív workflow valódi fókusz-/IME-/undo-eseménnyel; Windows külön MSBuild/link/launch/funkcionális kör. Linux saját runner/felület nélkül unrun. Nem natív csomagnál ezeket az adaptergazda végzi.
- [ ] `docs/development/results/P18.md` tartalmazza a teljesített ID-ket és reprodukálható bizonyítékot. P00 ennek alapján frissíti a handoffot, ledgert és a releváns skill-reference-et; más csomag ne írja párhuzamosan ezeket.

## Másolható indítófeladat

> Hajtsd végre a `docs/superpowers/plans/2026-10-31-parallel/p18-security.md` tervet. Előbb olvasd az AGENTS.md-t, a handoffot, a projekt-skillt, a közös csomagszerződést és a BASELINE_DELTA-t. Csak P18 saját útvonalait módosítsd, a közös fájlokhoz integration-manifestet adj. Őrizd meg a közvetlen agent-edit + explicit Save + Save után is elérhető undo szerződést. A követelményfüggőséget és platformi bizonyítékot ellenőrizd; planned/unrun állapotot ne nevezd késznek.
