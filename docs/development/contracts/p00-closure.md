# Issue1: P00 saját részeredménye és lezárási feltételei

A [P00 terv](../../superpowers/plans/2026-10-31-parallel/p00-integration.md) és az eredeti követelménygráf az alap. A [gépileg ellenőrzött sidecar](../../../integration/registry/p00-acceptance.v1.json) report-metaadat, nem új v1 DTO vagy elfogadott provider. A 43 másik csomag nem általános P00-lezárási kapu.

## Mit végez el P00 önállóan?

| Követelmény | P00 saját, ellenőrizhető része | Bizonyíték határa |
| --- | --- | --- |
| QA-08 | Source/terv/coordinator regresszió; közös Rust és seam; valódi queue; jelenlegi Mac IPC/MCP/control adapterpróba. | Kiválasztott tesztek, exit/timeout/skip, buildtermékek és inputazonosság. Nem MCP-03 vagy élő natív termékacceptance. |
| QA-14 | Mentett report read-only újraellenőrzése; minden input/artifact/resource/log/számláló/leírás egyeztetése; 226 történeti baseline és 44 csomag source-only leltára. | Tényleges jelenlegi fájlmegfigyelés és tesztfutás külön. A leírás ellenőrzése bájt-/helyilink-azonosság, nem teljes prózaminősítés vagy producerhitelesítés. |
| OS-10 | Relatív contributor-parancsok, külön structure/shared/Mac adapter szintek, önálló module-seam és karbantartott handoff/skill. | Forrás-/link-/fordítási belépés és futtatott teszt külön; nem igazolatlan Windows/Linux natív bootstrap. |

Az `independent_status=passed` csak azt jelenti, hogy az adott aktuális report a saját összes kiválasztott feltételt igazolta. A teljes requirement `acceptance` ettől még `partial`. Structure/shared körben a Mac adapterfeltétel `unrun`, ezért nem jelenthető teljes önálló integrált szelet.

## Melyik rész vár másik csomagra?

| Érintett állítás | Valódi függőség / gazda | Pontosan mit kell átadni? |
| --- | --- | --- |
| QA-08 teljes acceptance | MCP-03, P03 | Verziózott típusos runtime/protokoll és a régi/új kliens kompatibilitásának valódi tesztje; hibás válasz nem okozhat natív force-cast összeomlást. Saját source/test, `integration/requests/P03.json`, közös hookigény és friss providereredmény. P00 ezt integrálja, majd a tényleges runtime/adapterrel újrafuttat. |
| QA-14 teljes acceptance | QA-08, P00; azon keresztül MCP-03/P03 | A működő providerrel lefutott QA-08 bizonyítéka és az ahhoz tartozó tényleges source/binary/test/description kötés. A saját forrásleltár és frissességkezelés ettől függetlenül elkészíthető. |
| OS-10 teljes acceptance | QA-14, P00; azon keresztül QA-08 és MCP-03/P03 | Az új közös bekötés után friss contributor-belépés és dokumentáció. A mostani referencia/parancs/scope munka nem vár P03-ra; az új provider valós belépési eredményét később kell hozzáadni. |

Ezek az eredeti hard élek: `MCP-03/P03 → QA-08/P00 → QA-14/P00 → OS-10/P00`. A manifest vagy service/test fájl megjelenése önmagában nem oldja fel őket. Más csomag a saját `owned_paths` útvonalain párhuzamosan dolgozhat; éles közös bekötést P00 csak valódi manifest és regresszió mellett végez.

## Natív és egyéb bizonyítékok külön scope-ban

| Claim | Gazda és hiányzó valós ellenőrzés | Mit nem szabad ebből P00-blokkolónak nevezni? |
| --- | --- | --- |
| Élő csomagolt Mac workspace | Az érintett adapter gazdája; tipikusan P07/P08/P14. Valós focus/IME, direct edit, explicit Save, Save utáni undo és megmaradó el nem küldött munka. | A mostani control XCTest nem ez a workflow. A gazdák nem új hard DAG-élek; az érintett termékállítás előtt kell natív bizonyíték. |
| Windows11 x64 Intel és AMD | P21: MSBuild/link, launch, shared session/IPC és funkcionális focus/IME/edit/Save/undo a célgépeken. | A Windows Server Rust/Python CI nem WinUI vagy Windows11 futtatás. Nem akadályozza a jelenlegi Mac/common coordinator részeredményét. |
| Későbbi Linuxdesktop | P22: platform/UI döntés, natív build/launch, Unix IPC és XDG/runtime-dir viselkedés. | Jövőbeli külön port; nem a jelenleg vállalt Mac/common szelet előfeltétele. |
| Bővebb fault/corpus | P19 saját korpusz és fault-injection/protokoll harness. | Külön QA-program; nem hozzáadott P00 requirement-él. |
| Release/minősített csomagolás | P20 célplatformi package/signing/migration és valódi kiadási ellenőrzések. | A P00 debug-/tesztbuild nem kiadás. |
| AX/render/usability/performance | P23 valódi AX-/render-/használhatósági eredmények és release benchmark. | Néhány controlteszt vagy régi opt-in render skip nem minősíti ezeket; nem hozzáadott P00-DAG-él. |

A csomagok natív körét a csomaggazda végzi. P00 a beérkezett eredményt/hookot és a közös regressziót ellenőrzi; nem veszi át P21/P22/P23 implementációs útvonalait.

## Másolható contributor-belépés

A repository gyökeréből dolgozz. A Python3.13+ és a Rust1.99.1 legyen ténylegesen elérhető. A Rustup/egyéni telepített toolchain választását a [v1 útmutató](v1.md) írja le. Az `--offline` csak meglévő dependency-cache mellett használható; a cache hiánya hibás/unrun környezet, nem termék-PASS.

Mac/Linux shell:

```sh
python3 integration/registry/verify.py --profile structure
python3 integration/registry/verify.py --profile shared --offline
# Csak Mac arm64, elérhető SwiftPM és meglévő Rust toolchain mellett:
python3 integration/registry/verify.py --profile integrated --offline
# Ugyanazon checkouton, teszt/builder indítása és reportírás nélkül:
python3 integration/registry/verify.py --recheck-report .local/p00-verification/report.json
```

Windows PowerShell:

```powershell
python integration/registry/verify.py --profile structure
python integration/registry/verify.py --profile shared --offline
python integration/registry/verify.py --recheck-report .local/p00-verification/report.json
```

Windowson/Linuxon az `integrated` profil kötelező Mac-körei `unrun` állapotúak, a profil nem success. A recheck az eredeti platformon készült aktuális reportot vizsgálja; más host vagy régi selection-metaadat esetén friss ellenőrzést kér. A saját Rust CLI önállóan is használható: `cargo +1.99.1 run -p latexide-contracts --bin latexide-integration --locked -- --root .`.

## Gépi mezők és exitkód

- `evidence_validation`: mentett eredmény mostani source/selection/artifacts/logs/tests/descriptions azonossága. `current` esetén0 exit, elavult/hibás/korábbi sikertelen eredmény esetén1. A reportban tárolt `command` soha nem végrehajtási utasítás.
- `capability_inventory`: 226 baseline és 44 csomag jelenlegi service/test/manifest forrásmegfigyelése. `source_only`, `absent`, `reference_directory` és `unverified_symlink` külön állapot; nincs fájlalapú acceptance-promóció. A régi LANG-06partial megmarad.
- `p00_readiness.requirements`: QA-08/QA-14/OS-10 saját checkfeltételei és az eredeti közvetlen/transzitív előfeltételek.
- `external_package_blockers`: a külső hard requirement/gazda és az átadandó valódi acceptance; jelenleg MCP-03/P03. `manifest_present` csupán megfigyelés. A `blocks_requirements` minden fogyasztót külön `direct` jelzéssel mutat: QA-08 közvetlen, QA-14 és OS-10 transzitív.
- `validation_scopes`: natív/future/platform és külön QA-/releaseprogramok, explicit unrun eredménnyel, új DAG-él nélkül.

A normál verifier kiválasztott profilja lehet sikeres úgy, hogy az Issue1 acceptance még részleges. A későn megváltozott input/buildtermék a saját derived readiness-t is érvényteleníti. A recheck nem attesztáció, nem ellenséges swap-and-restore elleni írási határ, nem új MCP-jogosultság és nem automatikus merge.
