# P00 reportfrissesség és lezárási mátrix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans. The user explicitly requested autonomous continuation of the accepted P00 requirements; retain inline execution.

**Goal:** A mentett bizonyíték újraellenőrizhető, a P00 saját részeredménye és a külső/natív feltétel külön gépi állapot.

**Architecture:** Hozzáadó Python coordinator-eszköz a meglévő trusted verifier/checkekkel. Nem módosítja a Rust DTO-kat vagy providert; változatlan történeti baseline mellett forrásmegfigyelést és részfeltételeket ad.

**Tech Stack:** Python3.13+ standard library, meglévő Rust1.99.1/SwiftPM suite és CI; új dependency nélkül.

**Spec:** [Önálló szelet](p00-readiness-spec.md), [v1](v1.md), [P00 főterv](../../superpowers/plans/2026-10-31-parallel/p00-integration.md).

## Global Constraints

- Egy authoritative Rust runtime; UTF-16 és expected_revision; agent edit közvetlen, Save explicit, undo Save után is.
- Contract1.0/legacyIPCv1/CABI/kvóták változatlanok. Minden változás P00 saját vagy meglévő integrátori útvonalon.
- Macarm64 eredmény nem Windows11/WinUI/Linuxdesktop. Report és source-only fájl nem acceptance vagy attesztáció.
- Csak ignorált helyen nyers report/log/cache. A frissességmód nem ír és nem indít folyamatot.

## Review Focus

- R1: Reportból kihagyott/átírt kötelező check, hamis required flag, duplikáció vagy nulla teszt nem lehet current.
- R2: Mentés után módosult forrás, új input, artifact/resource, log és leírás elavultnak minősül; a report nem írja felül önmagát.
- R3: Providerfájl/manifest önmagában source-only; a226baseline és LANG-06partial sértetlen.
- R4: A hard dependency-k az eredeti gráfot követik; P03hiány nem43csomagos kapu; Linuxfuture és release/AX scope külön marad.
- R5: Structure/shared/contracts profil nem Macnative eredmény, sikertelen/stale reportból saját P00-feltétel sem kaphat passed állapotot.

## Task 1: Mentett report read-only újraellenőrzése

**Files:** Create `integration/registry/evidence.py`, `tests/contracts/test_evidence.py`; modify `integration/registry/verify.py`.

**Interfaces:** `revalidate_report(root, report, current_source, expected_steps, artifact_roots=())->dict`. `current_source` a verifier frissen számolt source_identity-je; a check-definíció trusted build_steps. A közös `validate_summary` az evidence modulba kerül, verify kompatibilis alias marad. CLI `verify.py --recheck-report FILE` csak JSON stdout és0/1 exit; a normál report `selection` és `run_directory` metaadatot kap.

- [x] RED: valódi run_checks fixture report friss eset success; új/módosult source, kicserélt resource/log, törölt/dupla check, hamis required és hibás számláló fail. Régi formátum és failed report nem pass; CLI nem indít parancsot és nem ír fájlt.
- [x] Run `python3 -m unittest discover -s tests/contracts -p test_evidence.py`: FAIL az új API/CLI hiányán.
- [x] Implementáld bounded8MiB reportinputtal, deklarált source/outputgyökerekkel, hiányzó artifactkind és teljes fogyasztott készlet ellenőrzésével; a reportból nem választható végrehajtandó parancs.
- [x] GREEN ugyanazzal; teljes meglévő Python suite. Commit `feat(p00): revalidate stored verification evidence`.

## Task 2: Forrásleltár és önálló/külső feltételek

**Files:** Create `integration/registry/readiness.py`, `integration/registry/p00-acceptance.v1.json`, `tests/contracts/test_readiness.py`.

**Interfaces:** `capability_inventory(root)->dict`226baseline +44package source observations; `p00_readiness(root, report, validation)->dict` három requirement saját check-feltétele + valódi DAG és külön validation/program scope. Szűk sidecar schema1, nem a befagyasztott contract-v1 módosítása. Nincs provider-receipt import vagy automatikus acceptance-promóció.

- [x] RED: hiányzó és jelenlévő service/test más állapot, fájl/manifest nem acceptance; LANG-06partial megmarad. Sérült tulajdon/DAG vagy elhagyottMCP-03 deklaráció hiba. Structure/shared Macnative feltétel unrun; stale/failed report nem passed.
- [x] Run `python3 -m unittest discover -s tests/contracts -p test_readiness.py`: FAIL az új API hiányán.
- [x] Implementáld a source-only mérést, pontos owner/requirement/blocker/átadandó evidence mezőket, külön scoped WindowsP21/LinuxP22/Macadapter és nem-P00 programtételeket. A saját checkek valódi, frissen újraellenőrzött reportból jönnek.
- [x] GREEN ugyanazzal és teljesPython. Commit `feat(p00): expose capability and closure readiness`.

## Task 3: Verifier, contributor átadás és ellenőrzés

**Files:** Modify `verify.py`, `probes.py`, P00 manifest/work-status; create `docs/development/contracts/p00-closure.md`; update v1 guide, handoff, ledger, P00 results and project-skill reference.

**Interfaces:** A normál report `evidence_validation`, `capability_inventory`, `p00_readiness` mezőt ad. A recheck mód is friss állapotot számol, de nem futtat. A closure guide a machine sidecar szerződésének pontos emberi használata, relatív parancsokkal és aktuális state/build/run különbséggel.

- [x] RED: végponti reportban saját regresszió és külső várakozás külön, source-only és teljes acceptance nem mosódik össze; késői forrásváltozás minden derived saját állapotot érvénytelenít.
- [x] GREEN: `verify.py --profile integrated --offline` Macen és read-only recheck ugyanazon source/artifacton; rootPython, generator, skill/link/source guard. CI Macintegrated, Windows/Linuxshared marad.
- [x] Dokumentáld pontosan a P00 által még önállóan elvégezhető részek teljesítését, a P03igényt és a natív/egyéb scopehatárt. Commit `docs(p00): hand off explicit closure dependencies`.
- [ ] Egyetlen friss teljes-szelet reviewer; Important/Critical egy TDD-fix pass, minor ledger. Saját feature-PR és friss CI; Issue1 marad nyitott. Új main-merge nem része ennek a folytatásnak.
