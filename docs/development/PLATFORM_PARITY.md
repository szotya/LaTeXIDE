# Platformparitás — fejlesztői induló mátrix

Frissítve: 2026-11-03. Termékbaseline: `dd14b4e047a5a13e5a4b2b30942895864bd809dd`. A [fejlesztési szabály](PLATFORM_DEVELOPMENT.md) szerint a Mac elfogadott működése irányadó; a táblázat nem késztermék- vagy teljes platformelfogadás. A Windows/Linux natív körök **unrun**. A közös Rust és a DTO-k jelenléte külön scope.

| Képesség / referencia | Mac | Windows P21 | Linux P22 |
| --- | --- | --- | --- |
| Egy hiteles Rust runtime, C ABI és MCP; [handoff](../DEVELOPER_HANDOFF.md) | Meglévő adapter és IPC/MCP regresszió igazolt | Natív bridge/named-pipe forrás, shared Rust CI; WinUI-session unrun | Shared Rust CI; natív host/IPC/XDG bekötés hiányzik |
| UTF-16, revízió, edit/save/undo; [natív szerződés](../../skills/latexide-platform-development/SKILL.md) | Kontrollált adapter/undo-tesztek és korábbi élő minták; teljes IME/eszközmátrix unrun | RichEdit-adapter forrás, hordozható helper; tényleges IME/fókusz/undo unrun | Nincs natív editoradapter |
| Témák, panelállapot, fókusz; [UI](../ui-verification.md) | Natív megvalósítás; a részletes dokumentum saját scope-ja irányadó | XAML/téma/Find/panel forrás; render és tartós workflow unrun | Nincs natív workspace |
| Saját makró/ref/cite completion; [ellenőrzés](../project-completion-verification.md) | Rust index + natív popup/friss elfogadás implementált és tesztelt | Közös API elérhető; natív popup/elfogadás portolandó | Közös API elérhető; widget/offset/popup portolandó |
| Definícióra ugrás; [ellenőrzés](../definition-navigation-verification.md) | Lexikális saját makró/ref/cite, Cmd+kattintás és kontrollteszt | Közös API; natív Ctrl+kattintás portolandó | Közös API; natív gesztus/koordináta-konverzió portolandó |
| Codex-asszisztens, Stop és draft; [ellenőrzés](../intelligence-verification.md) | Kísérleti CLI-integráció, subprocess tesztek és korábbi modellpróba; a draft workspace-életű | Az MCP-panel nem natív Codex-chat; process/session/panel portolandó | Nincs natív asszisztensadapter |
| PDF és forrás/PDF-munkatér; [termékirány](../IDE_PRODUCT_DIRECTION.md) | PDFKit-alapú megvalósítás; teljes SyncTeX további feladat | Windows.Data.Pdf lap/zoom forrás; futás és szövegkeresési paritás hiányzik | Toolkit/PDF-adapter nincs kiválasztva |
| Compiler, hibák, cancel/timeout; [compiler](../compiler-assessment.md) | Meglévő runtime és kontrollált Mac/TeX bizonyíték; teljes production-acceptance részleges | Job Object/compiler forrás; célgépes TeX/folyamatfa/PDF unrun | Unix shared tesztek; Linux TeX/native folyamatfa/desktop unrun |
| Recovery/VFS/security új szeletek; [P00](results/P00.md) | A friss handoff és csomageredmény mondja meg, mi standalone és mi bekötött | A Mac standalone eredmény nem Windows runtime-eredmény | A Unix-forrás vagy Ubuntu unit PASS nem Linux desktop/containment |

Új paritásbejegyzéshez a P21/P22 eredménynapló adjon pontos Mac-dokumentumot/tesztet, alap- és saját commitot, tényleges célplatformi parancsot/eredményt és megmaradó hiányt. A központi mátrixot P00 frissíti. A tervezett [P21](../superpowers/plans/2026-10-31-parallel/p21-windows.md) és [P22](../superpowers/plans/2026-10-31-parallel/p22-linux.md) követelmények ettől nem zárulnak le.
