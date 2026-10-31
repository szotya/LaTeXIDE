# P14 — Asszisztens, sessiontörténet és provideradapter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Tartós, újracsatlakoztatható és tényleges eszközállapotú natív agentmunkafolyamatot ad.

**Architecture:** A mostani Codex app-server adapter kap verzióalkut és per-session tartós eseménynaplót. A draft külön állapot; edit a meglévő MCP/Rust porton, nincs új engedélykapu minden szerkesztés előtt.

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

- R1: Reconnect után ismételt esemény ne ismételje meg az editet vagy a felhasználói turnt.
- R2: El nem küldött draft és csatolmány több ablak/session mellett se kerüljön más beszélgetésbe.
- R3: Unsupported Codex protokoll konkrét kompatibilitási hibát adjon.
- R4: Providerhiba vagy Stop valós toolállapotot mutasson, ne maradjon végtelen running.
- R5: Valós modellteszt credential/költség/online feltételei külön opt-in eredményként szerepeljenek.

---

## Tulajdon és indulás

**Branch:** `feature/p14-assistant`. **Profil:** core.

**Kizárólagosan létrehozható/módosítható útvonalak:**

- `apps/macos/Sources/LaTeXIDEKit/Features/AssistantHistory/`
- `apps/macos/Tests/LaTeXIDEKitTests/AssistantHistoryTests.swift`
- `integration/requests/P14.json`
- `docs/development/results/P14.md`

**Teszt:** `apps/macos/Tests/LaTeXIDEKitTests/AssistantHistoryTests.swift`. **Eredménynapló:** `docs/development/results/P14.md`.

**Olvasandó jelenlegi belépési pontok:** `apps/macos/Sources/LaTeXIDEKit/Assistant/AssistantSession.swift`, `apps/macos/Sources/LaTeXIDEKit/Assistant/CodexProcessTransport.swift`.

Indulás: P00 contract-v1 után, fagyasztott fixture-ökkel és fake portokkal; a többi csomag teljes elkészülte nem indulási feltétel.

Integrációs szomszédok: P01, P02, P03, P06, P08, P09, P13, P15, P17, P18, P19. Ezek **nem csomagszintű topologikus sorrend**: az egyedi követelményfüggőségek maradnak irányadók. A kölcsönös portfüggést fixture és fokozatos bekötés oldja, nem két teljes csomag egymásra várása.

## Task 1: Önállóan ellenőrizhető szerződésszelet

**Create:** `apps/macos/Sources/LaTeXIDEKit/Features/AssistantHistory/HistoryStore.swift`, `apps/macos/Tests/LaTeXIDEKitTests/AssistantHistoryTests.swift`. **Consumes / produces:**

```rust
pub fn reduce_assistant(state:AssistantState, event:AssistantEvent) -> Result<AssistantState,AssistantError>;
// AssistantState { session_id:String, draft:String, active_turn:Option<String>, last_sequence:u64 }; AssistantEvent { session_id:String, sequence:u64, kind:AssistantEventKind }; AssistantEventKind::{DraftChanged(String),TurnStarted(String),TurnStopped,Disconnected,Reconnected}; AssistantError::{WrongSession,SequenceGap,UnsupportedVersion}
```

Az itt felsorolt DTO-mezőket P00 helyezi a `crates/latexide-contracts/src/` moduljaiba, serde serialize/deserialize és egyértelmű hiba-enummal. A szolgáltatás a saját modulban marad; ne hozz létre protocol→core→protocol függőségi kört. A port v1 helyi Rust-határ, nem automatikus új IPC-eszköz.

**Saját adapter belépési pontja:**

```swift
func reduceAssistant(state: AssistantState, event: AssistantEvent) throws -> AssistantState
```

A fenti Rust-alak az interoperábilis szerződést rögzíti; a saját adapter az itt megadott nyelven készül. Az első assertiont az adapter saját tesztkerete ellenőrzi.

- [ ] **RED:** `apps/macos/Tests/LaTeXIDEKitTests/AssistantHistoryTests.swift` alatt `contract_v1_first_slice` néven rögzítsd ezt az ellenőrzést: **Panelváltás és reconnect után a draft változatlan; ismételt Stop idempotens, más session eseménye elutasított.**
- [ ] Futtasd: `swift test --package-path apps/macos --filter AssistantHistoryTests`. Először a hiányzó szolgáltatásra vagy állított viselkedésre FAIL legyen; környezet-/linkhibát ne könyvelj reprodukciónak.
- [ ] Valósítsd meg az első portot `apps/macos/Sources/LaTeXIDEKit/Features/AssistantHistory/HistoryStore.swift` alatt; az alkalmazás-regisztrációt add át `integration/requests/P14.json` manifestben (schema, package_id, contract_version, requirement_ids, source_paths, hook_targets, test_commands, compatibility).
- [ ] Ugyanazzal a paranccsal legyen GREEN; tedd mellé R1–R5 célzott regresszióját a `review_focus_r1` … `review_focus_r5` tesztekbe. Az állítások a fenti öt elvárt viselkedést vizsgálják.
- [ ] Ellenőrizd a saját diffet, majd készíts kis commitot: `git add -- apps/macos/Sources/LaTeXIDEKit/Features/AssistantHistory/HistoryStore.swift apps/macos/Tests/LaTeXIDEKitTests/AssistantHistoryTests.swift integration/requests/P14.json` és `git commit -m "feat(p14): establish tested service slice"`. A dokumentált későbbi munkavégzéskor commitolj; ez a terv most nem futtatja ezeket.

## Követelményenkénti feladatsor

Minden alábbi sor külön tesztelhető részeredmény és saját kis commit. A teszt neve az ID kisbetűs, aláhúzásos alakja; az elfogadás mondata a kötelező assertion. A dependency provider mockja csak izolált teszthez elég; release-hez a valódi providerrel is teljesüljön.

### AI-01 — Tartós draft és beszélgetéstörténet (P1, M)

**Kiindulás:** Draft panelváltást túlél, projekt/folyamatvégi lemezes history nincs.

**Módosítás:** Projektkötött, opcionálisan titkosított history/draft storage és retention/export/delete.

**Valódi bekötés előtt:** WS-02 (P01), SEC-06 (P18).

- [ ] `ai_01` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Restart után a draft megmarad; explicit reset töröl; más projekt nem kapja meg.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `swift test --package-path apps/macos --filter AssistantHistoryTests` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P14` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `AI-01` implemented/tested állapotúnak.

### AI-02 — Codex verzió- és capability-alku (P1, M)

**Kiindulás:** Egy alfa app-server változatra kvalifikált adapter van.

**Módosítás:** Startup version/capability ellenőrzés, kompatibilitási mátrix és szerződésfixture-k.

**Valódi bekötés előtt:** MCP-03 (P03).

- [ ] `ai_02` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Ismeretlen schema biztonságos hibát ad; támogatott verziók stream/cancel/tools működnek.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `swift test --package-path apps/macos --filter AssistantHistoryTests` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P14` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `AI-02` implemented/tested állapotúnak.

### AI-03 — Eszközlista és instrukció összhangja (P1, M)

**Kiindulás:** Kilenc eszköz exportált; a prompt undo/redo használatot is említ.

**Módosítás:** Tool registryből generált prompt/allowlist/schema; undo/redo támogatás vagy pontos korlátszöveg.

**Valódi bekötés előtt:** MCP-04 (P03).

- [ ] `ai_03` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Az agent csak létező eszközt kap ajánlásként; új tool nem nyer automatikus jogot.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `swift test --package-path apps/macos --filter AssistantHistoryTests` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P14` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `AI-03` implemented/tested állapotúnak.

### AI-04 — Alkalmazott változások és revíziók (P1, M)

**Kiindulás:** Stream/tool státusz létezik, tranzakciódiff nincs.

**Módosítás:** Tool call → transaction/build azonosítók és utólagos diff/revert kapcsolatok.

**Valódi bekötés előtt:** GIT-12 (P13), MCP-04 (P03).

- [ ] `ai_04` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Válasz megkülönbözteti tanács/edit/save/build eredményt; egy kattra megnyitható a valódi diff.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `swift test --package-path apps/macos --filter AssistantHistoryTests` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P14` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `AI-04` implemented/tested állapotúnak.

### AI-05 — Model/provider és költség áttekintés (P2, M)

**Kiindulás:** Modell meglévő Codex configból; név megjelenhet, usage UI nincs.

**Módosítás:** Támogatott modellválasztás, tényleges usage/limit jelzés és hálózati mód tisztázása.

**Valódi bekötés előtt:** AI-02 (P14).

- [ ] `ai_05` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Nem gyárt token/költségadatot; providerhiba és keretkimerülés külön látható.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `swift test --package-path apps/macos --filter AssistantHistoryTests` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P14` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `AI-05` implemented/tested állapotúnak.

### AI-06 — Strukturált kontextus és eredet (P2, M)

**Kiindulás:** Selection/diagnostics csatolható, 64 KiB inputhatár van.

**Módosítás:** Fájl/szimbólum/build kontextus kiválasztó, revízió és csonkítás látható.

**Valódi bekötés előtt:** LANG-06 (P06), BLD-01 (P09).

- [ ] `ai_06` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Régi kontextus nem látszik aktuálisnak; limitnél a csatolmány kezelhető, prompt nem vész el.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `swift test --package-path apps/macos --filter AssistantHistoryTests` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P14` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `AI-06` implemented/tested állapotúnak.

### AI-07 — Több beszélgetés és agent-session (P2, L)

**Kiindulás:** Egy ephemeral thread/workspace; több session UI nincs.

**Módosítás:** Névvel/projekttel azonosított sessionlista és korlátozott háttérműködés.

**Valódi bekötés előtt:** AI-01 (P14), AI-02 (P14).

- [ ] `ai_07` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Turn késői eseménye nem kerül másik chatbe; projektváltási leállítás megmarad.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `swift test --package-path apps/macos --filter AssistantHistoryTests` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P14` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `AI-07` implemented/tested állapotúnak.

### AI-08 — További agentadapterek és ACP (P3, L)

**Kiindulás:** A felhasználó választott Codex adaptere működik.

**Módosítás:** Agent-neutral session interfész, opcionális ACP/provider adapter saját kvalifikációval.

**Valódi bekötés előtt:** AI-02 (P14), EXT-04 (P15).

- [ ] `ai_08` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Codex regresszió nélkül működik; új adapter bizonyított state/tool/stop viselkedést ad.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `swift test --package-path apps/macos --filter AssistantHistoryTests` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P14` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `AI-08` implemented/tested állapotúnak.

### AI-09 — Hibatűrő reconnect és turnfolytatás (P2, M)

**Kiindulás:** EOF/timeout/cancel védett; tartós resume nincs.

**Módosítás:** Capabilityfüggő reconnect/resume, idempotens toolnyugta és explicit félbeszakadt turn.

**Valódi bekötés előtt:** AI-01 (P14), MCP-05 (P03).

- [ ] `ai_09` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Hiba után nincs kétszer alkalmazott edit/save; bizonytalan eredmény újraolvasással tisztázott.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `swift test --package-path apps/macos --filter AssistantHistoryTests` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P14` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `AI-09` implemented/tested állapotúnak.

### AI-10 — Valós provider- és hosszú session próba (P1, M)

**Kiindulás:** Kontrollált subprocess és kis tényleges modellkör igazolt.

**Módosítás:** Támogatott Codex/account/provider körök és hosszú vegyes user/agent stressz.

**Valódi bekötés előtt:** QA-05 (P19), AI-02 (P14).

- [ ] `ai_10` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Minden támogatott kombináció saját eredményt kap; fixture nem szerepel valós modellpróbaként.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `swift test --package-path apps/macos --filter AssistantHistoryTests` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P14` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `AI-10` implemented/tested állapotúnak.

### AI-11 — Inline magyarázat és szerkesztés (P2, M)

**Kiindulás:** Oldalsó natív chat van, inline belépési pont nincs.

**Módosítás:** Kijelöléshez/diagnosztikához kötött inline kérés azonos session/tool szerződéssel.

**Valódi bekötés előtt:** AI-04 (P14), WS-06 (P08).

- [ ] `ai_11` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Edit a Rust undo-ba kerül; mezőfókusz és Escape működik, nincs automatikus modellhívás.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `swift test --package-path apps/macos --filter AssistantHistoryTests` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P14` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `AI-11` implemented/tested állapotúnak.

### AI-12 — Tartós agentinstrukció és promptkatalógus (P2, M)

**Kiindulás:** Beépített product instrukció és felhasználói prompt van.

**Módosítás:** Projekt-/user promptprofil, explicit prioritás és forrásjelzés; skill adapter opcionális.

**Valódi bekötés előtt:** WS-04 (P08), SEC-06 (P18).

- [ ] `ai_12` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Dokumentumszöveg nem lép elő fejlesztői instrukcióvá; változás auditálható.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `swift test --package-path apps/macos --filter AssistantHistoryTests` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P14` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `AI-12` implemented/tested állapotúnak.

### AI-13 — Teszt/build eredmények agenthez kapcsolása (P2, M)

**Kiindulás:** Agent tud buildet kérni; strukturált run/test/artifact kontextus szűk.

**Módosítás:** Valódi build/test/diagnosztika toolok típusos eredménnyel és artifactazonossággal.

**Valódi bekötés előtt:** DEV-05 (P17), BLD-11 (P09), MCP-04 (P03).

- [ ] `ai_13` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Agent csak lefutott ellenőrzést nevez sikeresnek; stop minden birtokolt műveletet elér.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `swift test --package-path apps/macos --filter AssistantHistoryTests` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P14` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `AI-13` implemented/tested állapotúnak.

### AI-14 — Párhuzamos agentek és feladatok (P3, L)

**Kiindulás:** Egy Codex turn fut; orchestration nincs.

**Módosítás:** Izolált task/session modell, kvóták, revíziókonfliktus és közös tranzakcióeredet.

**Valódi bekötés előtt:** AI-07 (P14), DOC-08 (P02), MCP-05 (P03).

- [ ] `ai_14` teszt a fenti saját tesztfájlban; konkrét elvárt eredmény: **Két agent ugyanazon revízión nem írja felül egymást; háttérmunka állapota látható.**
- [ ] RED→minimális implementáció a saját modulban→GREEN a `swift test --package-path apps/macos --filter AssistantHistoryTests` paranccsal; a szomszéd portokat a contract-v1 fixture adja.
- [ ] Add az ID-t a saját integration-manifesthez; a valódi providerrel és natív adapterrel is ellenőrizd az elfogadást. A hibás/elavult/kieső provider ne okozzon részleges módosítást.
- [ ] Kis commit és `P14` eredménynapló: parancs, kimenet, forrás-SHA, platform, unrun részek. Csak az összes assertion után jelölhető `AI-14` implemented/tested állapotúnak.

## Task Z: Bekötés és átadás

- [ ] `python3 scripts/validate_development_packages.py` és `swift test --package-path apps/macos --filter AssistantHistoryTests` sikeres; a tesztparancs itt célállapot, futtatását külön rögzíteni kell.
- [ ] P00 alkalmazza a manifestben felsorolt közös hookokat; stagingben egyesével fusson a korábbi Rust/native/IPC/MCP regresszió a megváltozott rétegnek megfelelően. Nincs automatikus Save vagy új agent-edit jóváhagyás.
- [ ] Mac natív workflow valódi fókusz-/IME-/undo-eseménnyel; Windows külön MSBuild/link/launch/funkcionális kör. Linux saját runner/felület nélkül unrun. Nem natív csomagnál ezeket az adaptergazda végzi.
- [ ] `docs/development/results/P14.md` tartalmazza a teljesített ID-ket és reprodukálható bizonyítékot. P00 ennek alapján frissíti a handoffot, ledgert és a releváns skill-reference-et; más csomag ne írja párhuzamosan ezeket.

## Másolható indítófeladat

> Hajtsd végre a `docs/superpowers/plans/2026-10-31-parallel/p14-assistant.md` tervet. Előbb olvasd az AGENTS.md-t, a handoffot, a projekt-skillt, a közös csomagszerződést és a BASELINE_DELTA-t. Csak P14 saját útvonalait módosítsd, a közös fájlokhoz integration-manifestet adj. Őrizd meg a közvetlen agent-edit + explicit Save + Save után is elérhető undo szerződést. A követelményfüggőséget és platformi bizonyítékot ellenőrizd; planned/unrun állapotot ne nevezd késznek.
