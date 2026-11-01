# P00 önálló frissesség- és lezárási szelet

A felhasználó az Issue1 önálló P00-feladatait és a pontos külső/natív várakozási határt kérte. Az elfogadott QA-08/QA-14/OS-10 és contract-v1 belépés a követelmény; ez a dokumentum ezek technikai részletezése, nem új termékarchitektúra.

- Új, read-only report-újraellenőrzés: a mentett siker a jelenlegi bemenetek, összes artifact, log, tesztszámláló és leírás mellett is érvényes-e. Nincs teszt/parancs/manifest-végrehajtás ebben a módban, nincs outputfelülírás. A report nem hitelesített attesztáció.
- A frissesség az aktuális trusted check-definícióval egyezik; kimaradt/duplikált kötelező check, hiányzó/cserélt resource, elavult forrás/leírás vagy sérült tesztösszesítés nem current PASS. A csak régebbi formátumú report újrafuttatást kér, nem kap implicit minősítést.
- A226 történeti baseline megmarad; a44 csomag tervezett entry/test útvonalának tényleges létezése külön source-only megfigyelés. Fájl vagy manifest jelenléte nem provideracceptance. A LANG-06 korábbi partial állapota nem változik missing-re.
- A QA-08/QA-14/OS-10 saját feltételeit külön mérjük a külső acceptance-től. A hard requirement-élek az owners/backlog valódi gráfjával egyeznek: MCP-03/P03 → QA-08/P00 → QA-14/P00 → OS-10/P00. Nincs43csomagos mesterséges kapu.
- Windows11x64 Intel/AMD natív MSBuild/link/launch és funkcionális kör P21; a Windows Server Rust közös réteg. Linux natív döntés/felület/runtime P22 külön jövőbeli scope, nem a jelenlegi Mac/common szelet feltétele. Az érintett Mac adapter valódi workflow-próbáját az adaptergazda adja; XCTest nem élő csomagolt-app acceptance.
- P19 korpusz/fault injection, P20 release és P23 AX/performance saját program: nem lesz belőlük implicit kötelező P00-DAG-él. A jövőbeli valódi providerek integrációját P00 a saját manifestjük után végzi.
- V1 DTO/IPC/C ABI, kvóták, közvetlen agent edit, explicit Save és Save utáni undo változatlan. Nincs product-provider/GUI/platform-implementáció más csomag útvonalán; nincs automatikus main-merge.

A gépi terv hozzáadó coordinator/report-metaadat. A teljes Issue1 részleges marad mindaddig, amíg a hiányzó valódi provider és a kért platform-/workflow-acceptance nincs igazolva. A generic frissességellenőrző nem ad önmagában jogosultságot írásra, merge-re vagy új MCP-toolra.
