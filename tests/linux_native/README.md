# Linux natív ellenőrzések — P22

P22 saját tesztfája. A tervezett `test_runtime_dir.py`, a probe és a GUI-harness **még nincs implementálva**. A [P22 eredménynapló](../../docs/development/results/P22.md) az indulást és a későbbi valódi eredményt külön vezeti.

Az első saját teszt Linuxon fordítja/indítja az `apps/linux/platform_probe` programot. Saját eldobható útvonalon vizsgáljon valódi owner/mode/symlink adatot, XDG hiányt és hibát, 0700 engedélyt, socketnév-hosszt, kétirányú IPC-t és leállítást. Hiányzó szolgáltatás tesztje nem meglévő natív képesség; caller által megadott metaadat nem helyettesíti az OS-lekérdezést. A közös `session.rs` bekötését P00 végzi.

A kiválasztott frontend után külön Wayland/X11 fókusz/IME/clipboard, widget→UTF-16 konverzió, közvetlen MCP-edit/Save/undo, compiler cancel/timeout és friss PDF-menet kell. Rögzíts disztribúciót, architektúrát, toolkit/PDF/toolverziókat, base/head SHA-t és [Mac-referenciát](../../docs/development/PLATFORM_PARITY.md). Ubuntu shared Rust PASS nem Linux GUI-minősítés. A Mac `/private/tmp` útvonala és telepítőpayloadja nem kerülhet át Linux-implementációként.
