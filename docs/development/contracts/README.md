# Contract-v1 indulási kapu

A közös [csomagszerződés](../../superpowers/specs/2026-10-31-parallel-development-design.md) megadja a DTO-kat, tulajdont és a kapu ellenőrzését. A [P00 terv](../../superpowers/plans/2026-10-31-parallel/p00-integration.md) végrehajtása hozza létre a Rust contracts crate-et és a `v1.md` implementációs leírást.

A `latexide-contracts` crate és a közös validátorok most implementáltak. [A v1 használata és pontos határa](v1.md), [csomagonkénti portok](../../../integration/registry/ports.v1.json), [eredménynapló](../results/P00.md). A szolgáltatások megvalósítása továbbra a saját csomagok feladata; az adatmodell/tesztminta nem kész UI-funkció.

A P00 munkája `feature/p00-integration` branchen folyik. A teljes P00 csomag és QA-08/14/OS-10 követelménylezárás részleges; a mostani indulási kapu az önálló fejlesztést teszi lehetővé. Végleges forrásazonosságot és tényleges ellenőrzési scope-ot az eredménynapló rögzít.
