# P00 integrációk és ellenőrzések Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this continuation task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A kiadott P00 indulási kaput valódi providerpróbákkal, beérkező manifestellenőrzéssel és a main ágvédelméhez illeszkedő CI-kapuval kiegészíteni.

**Architecture:** A v1 DTO-k és a meglévő IPC/MCP/C ABI változatlanok. P00 saját koordinátori eszközei a Rust manifestvalidátort és a meglévő FFI/native/IPC/MCP teszteket használják. A report külön forrás-, artifact-, teszt- és dokumentációazonosságot tart; egy manifest nem elfogadási bizonyíték és nem shellparancs.

**Tech Stack:** Meglévő Rust1.99.1/serde és Python3.13+ standard library, SwiftPM/XCTest a Mac adapterhez; új dependency nélkül.

**Spec:** [Közös csomagszerződés](../../superpowers/specs/2026-10-31-parallel-development-design.md), [P00 főterv](../../superpowers/plans/2026-10-31-parallel/p00-integration.md), [v1 szerződés](v1.md). A felhasználó kifejezetten kérte a folytatást; az elfogadott P00-scope és inline végrehajtás megmarad.

## Global Constraints

- Egyetlen authoritative Rust runtime, nulla alapú UTF-16, expected_revision, közvetlen agent edit, külön Save és Save után is undo.
- Contract1.0, IPCv1 és C ABI nem változhat észrevétlenül a párhuzamos branchek alatt.
- Más csomag implementációs útvonala nem írható; közös hookot P00 saját manifestje rögzít.
- A report/log/binary helyi ignorált adat. A privát repóba csak forrás, mesterséges fixture és szűk eredményösszegzés kerülhet.
- Windows Server Rust/Python nem Windows11WinUI; Linux Rust nem Linuxdesktop. A hiányzó provider/platform marad explicit unrun vagy partial.
- Nincs automatikus patchalkalmazás, main-merge vagy élő user-app újraindítás.

## Review Focus

- R1: A kötelező összesítő check mindig megjelenik; required-runtime skipped/failed/cancelled esetben nem lehet success.
- R2: Docs-only és kódváltozás helyesen szétválik új fájl/törlés/átnevezés és hiányzó Git-bázis mellett is; ismeretlen állapot nem engedhet lazább kaput.
- R3: A v1 manifestet a tényleges Rust validator ellenőrzi; symlink, nem létező forrás, név/csomag eltérés és idegen követelmény nem adhat kész integrációt.
- R4: A build összes felhasznált artifactja friss; sikertelen build, artifactcsere, elmaradt teszt vagy opcionális skip nem változhat teljes PASS-á.
- R5: A Mac-only providerpróba és a még hiányzó MCP-03/Windows11/Linuxdesktop acceptance explicit marad, a korábbi részleges LANG-06 sem lesz újra missing.

---

## Task 1: Állandó nevű, feltételes teljes CI-kapu

**Files:** Create `integration/registry/ci_gate.py`, `tests/contracts/test_ci_gate.py`; modify `.github/workflows/ci.yml`, `.github/workflows/rust-contracts.yml`.

**Interfaces:** `requires_runtime(paths:list[str])->bool`; `gate_succeeded(needs:dict)->bool`. CLI `changes --root PATH --base SHA --head SHA` Git NUL-path listából ad `runtime=true|false` GitHub outputot; `aggregate` a `P00_CI_NEEDS` környezeti JSON-t ellenőrzi. A nem feloldható Git-tartomány FAIL, workflow_dispatch/kezdeti push teljes kör.

- [ ] RED: CLI-tesztek valódi ideiglenes Git-repóval: docs-only eset `runtime_required=false`; Rust/Swift/manifest/CI-változás true; törlés/átnevezés nem maradhat ki; hibás bázis nem lehet docs-only.
- [ ] RED: aggregate sikeres planning+changes és docs-only runtime skip mellett0; required runtime skip/cancel/failure, hiányzó mező és rossz JSON mellett nem0.
- [ ] Futtasd: `python3 -m unittest discover -s tests/contracts -p 'test_ci_gate.py'`; az új CLI hiánya miatt FAIL legyen.
- [ ] Implementáld a tiszta döntési függvényeket és fix argv-s CLI-t. A source workflow hívja feltételesen a reusable Rust/Mac workflow-t; `always()` összesítő neve pontosan `Planning and Python contracts`, mindhárom előzményt ellenőrzi. A távoli rulesetbe nem ír.
- [ ] GREEN: ugyanaz a parancs, majd `python3 integration/registry/verify.py --profile structure`; valamennyi kiválasztott kör PASS.
- [ ] Commit: `feat(p00): add required aggregate CI gate`.

## Task 2: Valódi manifest-port és read-only integrációs queue

**Files:** Create `crates/latexide-contracts/src/coordinator.rs`, `src/bin/latexide-integration.rs`, `tests/coordinator_contract.rs`; modify a crate manuális `lib.rs` prefixét, P00 manifestjét és a verifier queue-lépését.

**Interfaces:** `IntegrationCoordinator` implementálja a meglévő `IntegrationPort` traitet. `inspect_queue(root:&Path)->Result<QueueInspection,ContractError>` a rögzített `integration/registry/owners.v1.json` és `integration/requests/*.json` adatait olvassa, determinisztikus manifest-/hook-/követelménysorrendet ad. CLI `latexide-integration --root PATH` JSON-t ír és0/1 exitet ad.

- [ ] RED: Rust-tesztek happy path, determinisztikus közös hook, idegen követelmény, hibás filename/package kötés, symlink és hiányzó forrás elutasítása; a manifestben lévő parancs nem hozhat létre sentinel fájlt.
- [ ] Futtasd: `cargo +1.99.1 test -p latexide-contracts --test coordinator_contract --locked`; az új koordinátor hiánya miatt FAIL.
- [ ] Implementáld a portadaptert a meglévő validátor delegálásával. A fájlrendszerellenőrzés repo-beli normál forrást kér; új hooknál meglévő, biztonságos szülőt. Bounded JSON,44 ismert csomag, szigorú serde; a queue nem alkalmaz patch-et és nem minősít acceptance-et.
- [ ] A reportban minden külső/saját requirement-előfeltétel külön unqualified marad; manifest jelenléte önmagában nem bizonyít providert.
- [ ] GREEN: ugyanaz a Rust-parancs és `cargo +1.99.1 run -p latexide-contracts --bin latexide-integration --locked -- --root .`; saját P00 queue validált, végrehajtási állapot `not_applied`.
- [ ] Commit: `feat(p00): validate integration queue through the real Rust port`.

## Task 3: Providerpróbák és frissesség a közös verifierben

**Files:** Modify `integration/registry/verify.py`, create `integration/registry/probes.py`, `tests/contracts/test_integrated_verification.py`; update CI, `docs/development/contracts/v1.md`, P00 eredmény, handoff, ledger, project-skill reference.

**Interfaces:** `build_steps(root,args,platform,machine)->list[dict]` fix argv-listákat készít. Új `integrated` profil: shared Rust + queue, FFI/MCP build, tényleges IPC/MCP és Mac native control tesztek, schema. `run_checks` több artifactot is producerhez köt, fogyasztás előtt/után egyeztet; report `artifacts`, `test_summary` és dokumentációidentity külön mező. A probes csak fix suite-neveket fogad, gépi summaryt ír.

- [ ] RED: több artifactból egy csere, buildhiba utáni régi binary, futás közbeni csere, elmaradt/hibás summary nem lehet PASS. A summary valódi unittest count/skip/failure adatból készül; ismeretlen suite nem futhat.
- [ ] RED: a runtime profilban FFI/static/MCP és XCTest artifact szükséges; Windows/Linux hoston a Mac-only kötelező kör explicit unrun és a profil nem success. A runtime/native/test/guide/workflow bemenet változása elavulttá teszi a saját bizonyítékát.
- [ ] Futtasd: `python3 -m unittest discover -s tests/contracts -p 'test_integrated_verification.py'`; az új viselkedés hiánya miatt FAIL.
- [ ] Implementáld a meglévő tesztprogramok adapterét új service/tool nélkül. `integrated` Mac arm64 alatt az aktuális library/bridge/static library és tesztbinary hashét rögzítse; explicit root target-dir, saját ignorált Swift/cache. Nyers adat nem kerülhet trackelt fájlba.
- [ ] GREEN: teljes `verify.py --profile integrated --offline` helyben, root Python-tesztek és formális skill/link/source ellenőrzés. Rust/Mac reusable CI ugyanezt futtatja; más host shared profilt.
- [ ] Frissítsd a gépi P00 állapotot: koordinátori providerbinding implementált/tesztelt; MCP-03 és natív célplatform acceptance továbbra részleges. Következő csomagkérelemhez másolható CLI és report-használat.
- [ ] Commit: `feat(p00): verify real runtime providers and evidence freshness`.

## Átadás és végső ellenőrzés

- [ ] Egyetlen friss reviewer e folytatás teljes tartományán, kiinduló `f9e050b01f33deb239f823283f9c7e5d84f2b5fa`; korábbi szelet review-ját nem ismételjük.
- [ ] Fontos találat esetén egy RED→GREEN javítási kör, teljes kiválasztott suite; minor a ledgerbe. Nyitott requirement csak valódi provider/platform acceptance után zárható.
- [ ] Feature PR45 frissítése és a teljes forrásmódosítás távoli CI-je success. Docs-only összesítő ágát külön valódi PR-futás vagy a megfelelő környezet hiányának explicit jelölése igazolja.
- [ ] Főágvédelmet/PR46 fájljait nem módosítjuk; a P00 handoff/ledger appendelve frissül, más csomag fájljai szabadok maradnak.
