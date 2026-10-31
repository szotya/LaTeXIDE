# P03 — Aszinkron runtime, IPC, FFI és MCP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Határidős, többklienses, típusos és megszakítható szolgáltatáshatárt épít a közös runtime köré.

**Architecture:** Korlátos kérdéssor és eseményfolyam választja szét a vezérlőhívást a hosszú index/build művelettől. A v1 klienskompatibilitás explicit adapter, a v2 request ID és idempotencia állapota sessionhöz kötött.

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

- R1: Első kliens soha nem zárja le a frame-et: a második továbbra is kapjon választ.
- R2: Újraküldött azonos request ID ne alkalmazza másodszor ugyanazt az editet.
- R3: Lassú eseményfogyasztó memóriakorláttal és látható gap/resync állapottal működjön.
- R4: Runtime free alatt függő FFI-hívás ne okozzon use-after-free-t vagy pánikát a C-határon.
- R5: Nagy olvasás snapshotazonossága chunkváltáskor se változzon csendben.

---

## Tulajdon és indulás

**Branch:** `feature/p03-runtime`. **Profil:** core.

**Kizárólagosan létrehozható/módosítható útvonalak:**

- `crates/latexide-ffi/src/dispatch/`
- `tests/runtime_v2_contract.py`
- `integration/requests/P03.json`
- `docs/development/results/P03.md`

**Teszt:** `tests/runtime_v2_contract.py`. **Eredménynapló:** `docs/development/results/P03.md`.

**Olvasandó jelenlegi belépési pontok:** `crates/latexide-protocol/src/ipc.rs`, `crates/latexide-ffi/src/session.rs`, `crates/latexide-mcp/src/main.rs`.

Indulás: P00 contract-v1 után, fagyasztott fixture-ökkel és fake portokkal; a többi csomag teljes elkészülte nem indulási feltétel.

Integrációs szomszédok: P01, P02, P18, P21. Ezek **nem csomagszintű topologikus sorrend**: az egyedi követelményfüggőségek maradnak irányadók. A kölcsönös portfüggést fixture és fokozatos bekötés oldja, nem két teljes csomag egymásra várása.

## Task 1: Önállóan ellenőrizhető szerződésszelet

**Create:** `crates/latexide-ffi/src/dispatch/mod.rs`, `tests/runtime_v2_contract.py`. **Consumes / produces:**

```rust
pub fn admit_frame(client_id: String, frame: &[u8], elapsed_ms:u64, limits: FrameLimits) -> Result<AcceptedFrame, TransportError>;
// FrameLimits { max_bytes:usize, deadline_ms:u64 }; AcceptedFrame { request_id:String, operation:String }; TransportError::{FrameTooLarge,Deadline,Unauthorized,InvalidFrame,DuplicateConflict}
```

Az itt felsorolt DTO-mezőket P00 helyezi a `crates/latexide-contracts/src/` moduljaiba, serde serialize/deserialize és egyértelmű hiba-enummal. A szolgáltatás a saját modulban marad; ne hozz létre protocol→core→protocol függőségi kört. A port v1 helyi Rust-határ, nem automatikus új IPC-eszköz.

- [ ] **RED:** `tests/runtime_v2_contract.py` alatt `contract_v1_first_slice` néven rögzítsd ezt az ellenőrzést: **Egy félkész kliens határidővel kiesik, közben a második hiteles kliens workspace_info válasza megérkezik.**
- [ ] Futtasd: `python3 tests/runtime_v2_contract.py`. Először a hiányzó szolgáltatásra vagy állított viselkedésre FAIL legyen; környezet-/linkhibát ne könyvelj reprodukciónak.
- [ ] Valósítsd meg az első portot `crates/latexide-ffi/src/dispatch/mod.rs` alatt; az alkalmazás-regisztrációt add át `integration/requests/P03.json` manifestben (schema, package_id, contract_version, requirement_ids, source_paths, hook_targets, test_commands, compatibility).
- [ ] Ugyanazzal a paranccsal legyen GREEN; tedd mellé R1–R5 célzott regresszióját a `review_focus_r1` … `review_focus_r5` tesztekbe. Az állítások a fenti öt elvárt viselkedést vizsgálják.
- [ ] Ellenőrizd a saját diffet, majd készíts kis commitot: `git add -- crates/latexide-ffi/src/dispatch/mod.rs tests/runtime_v2_contract.py integration/requests/P03.json` és `git commit -m "feat(p03): establish tested service slice"`. A dokumentált későbbi munkavégzéskor commitolj; ez a terv most nem futtatja ezeket.

## Követelményenkénti feladatsor

Minden alábbi sor külön tesztelhető részeredmény és saját kis commit. A teszt neve az ID kisbetűs, aláhúzásos alakja; az elfogadás mondata a kötelező assertion. A dependency provider mockja csak izolált teszthez elég; release-hez a valódi providerrel is teljesüljön.

### MCP-01 — IPC frame-határidő és több kliens (P1, M)

**Kiindulás:** Hiányos első keret blokkolja a második klienst, OBS-07.

**Módosítás:** Olvasás/írás határidő, bounded connection queue és rendezett leállítás.

**Valódi bekötés előtt:** nincs.

- [ ] `mcp_01` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Lassú kliens mellett másik kliens és app kilépése korlátos; tokenvédelem megmarad.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `python3 tests/runtime_v2_contract.py` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P03` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `MCP-01` implemented/tested állapotúnak.

### MCP-02 — Aszinkron runtime és eseményfolyam (P1, L)

**Kiindulás:** Globális Mutex, szinkron FFI és 250/300 ms poll működik.

**Módosítás:** Soros command owner, háttér I/O/index/read-only snapshot és verziózott event stream.

**Valódi bekötés előtt:** nincs.

- [ ] `mcp_02` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Lassú disk/index nem blokkolja a caret/gépelést; sorrend és stale-elutasítás megmarad.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `python3 tests/runtime_v2_contract.py` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P03` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `MCP-02` implemented/tested állapotúnak.

### MCP-03 — Verziózott típusos protokoll (P1, M)

**Kiindulás:** V1 JSON enum és kézi Swift/C++ dictionary/castok vannak.

**Módosítás:** Képességalku, generált DTO/schema és stabil error code/operation id mezők.

**Valódi bekötés előtt:** nincs.

- [ ] `mcp_03` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Régi/új kliens kompatibilitása tesztelt; hibás válasz nem okoz force-cast összeomlást.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `python3 tests/runtime_v2_contract.py` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P03` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `MCP-03` implemented/tested állapotúnak.

### MCP-04 — MCP képességek a teljes IDE-hez (P1, M)

**Kiindulás:** Kilenc tool; syntax/completion/definition/search/file ops nem exportált.

**Módosítás:** Közös szolgáltatás-registry és olvasható/edit/run capabilityk; szűk explicit grants.

**Valódi bekötés előtt:** MCP-03 (P03).

- [ ] `mcp_04` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Tool, schema, prompt és allowlist egyezik; native/agent ugyanazt az állapotot használja.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `python3 tests/runtime_v2_contract.py` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P03` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `MCP-04` implemented/tested állapotúnak.

### MCP-05 — Request ID és idempotencia (P1, M)

**Kiindulás:** Edit revízióvédett; ismételt klienskérésre nincs operation dedup.

**Módosítás:** Client/request/transaction ID és nyugta-cache; retry szemantika definiálása.

**Valódi bekötés előtt:** MCP-03 (P03).

- [ ] `mcp_05` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Elveszett válasz utáni retry nem dupláz editet/fájlműveletet/buildet.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `python3 tests/runtime_v2_contract.py` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P03` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `MCP-05` implemented/tested állapotúnak.

### MCP-06 — Session discovery és csatlakozás UX (P2, M)

**Kiindulás:** Egy futó session konfigurációja másolható.

**Módosítás:** Privát sessionlista és explicit projektazonosság, lejárt kapcsolat hibával.

**Valódi bekötés előtt:** MCP-03 (P03), SEC-03 (P18).

- [ ] `mcp_06` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Több app/projekt közül a megfelelőhöz kapcsolódik; token nem kerül UI-logba.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `python3 tests/runtime_v2_contract.py` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P03` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `MCP-06` implemented/tested állapotúnak.

### MCP-07 — Subscribe és streamelt lekérdezések (P2, M)

**Kiindulás:** Pollolt állapot és chunk olvasás létezik.

**Módosítás:** Backpressure-es event/cancel/stream API search/index/build számára.

**Valódi bekötés előtt:** MCP-02 (P03), MCP-03 (P03).

- [ ] `mcp_07` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Lassú kliens nem növeli korlátlanul a memóriát; újracsatlakozáskor resync lehetséges.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `python3 tests/runtime_v2_contract.py` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P03` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `MCP-07` implemented/tested állapotúnak.

### MCP-08 — Olvasási snapshot és chunkköltség (P1, M)

**Kiindulás:** Minden read chunkhoz teljes DocumentSnapshot klónozható.

**Módosítás:** Borrows/snapshot-handle és verziózáras, bounded chunkolvasás.

**Valódi bekötés előtt:** DOC-12 (P02), MCP-03 (P03).

- [ ] `mcp_08` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Nagy fájl olvasása nem végez teljes másolást chunkonként; memória mérve.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `python3 tests/runtime_v2_contract.py` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P03` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `MCP-08` implemented/tested állapotúnak.

### MCP-09 — Agenttranzakció és undo előzmény API (P2, M)

**Kiindulás:** Origin és transaction_id van; listázott diff/history tool nincs.

**Módosítás:** Olvasható history/tranzakció és célzott, konfliktusvédett revert API.

**Valódi bekötés előtt:** DOC-08 (P02), DOC-05 (P01), MCP-04 (P03).

- [ ] `mcp_09` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **User utólagos editjét agentrevert nem törli; visszavonás új eredetű tranzakció.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `python3 tests/runtime_v2_contract.py` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P03` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `MCP-09` implemented/tested állapotúnak.

### MCP-10 — Transport és FFI ellenőrzés célonként (P1, M)

**Kiindulás:** Mac C/IPC/MCP tesztelt; Windows named pipe forrás/check szintű.

**Módosítás:** Null/invalid frame/EOF/concurrency/cleanup/ACL célplatformi harness és fuzz corpus.

**Valódi bekötés előtt:** OS-01 (P21), SEC-03 (P18).

- [ ] `mcp_10` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Windows és későbbi Linux saját linked/runtime bizonyítékot ad; Mac eredmény nem helyettesíti.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `python3 tests/runtime_v2_contract.py` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P03` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `MCP-10` implemented/tested állapotúnak.

## Task Z: Bekötés és átadás

- [ ] `python3 scripts/validate_development_packages.py` és `python3 tests/runtime_v2_contract.py` sikeres; a tesztparancs itt célállapot, futtatását külön rögzíteni kell.
- [ ] P00 alkalmazza a manifestben felsorolt közös hookokat; stagingben egyesével fusson a korábbi Rust/native/IPC/MCP regresszió a megváltozott rétegnek megfelelően. Nincs automatikus Save vagy új agent-edit jóváhagyás.
- [ ] Mac natív workflow valódi fókusz-/IME-/undo-eseménnyel; Windows külön MSBuild/link/launch/funkcionális kör. Linux saját runner/felület nélkül unrun. Nem natív csomagnál ezeket az adaptergazda végzi.
- [ ] `docs/development/results/P03.md` tartalmazza a teljesített ID-ket és reprodukálható bizonyítékot. P00 ennek alapján frissíti a handoffot, ledgert és a releváns skill-reference-et; más csomag ne írja párhuzamosan ezeket.

## Másolható indítófeladat

> Hajtsd végre a `docs/superpowers/plans/2026-10-31-parallel/p03-runtime.md` tervet. Előbb olvasd az AGENTS.md-t, a handoffot, a projekt-skillt, a közös csomagszerződést és a BASELINE_DELTA-t. Csak P03 saját útvonalait módosítsd, a közös fájlokhoz integration-manifestet adj. Őrizd meg a közvetlen agent-edit + explicit Save + Save után is elérhető undo szerződést. A követelményfüggőséget és platformi bizonyítékot ellenőrizd; planned/unrun állapotot ne nevezd késznek.
