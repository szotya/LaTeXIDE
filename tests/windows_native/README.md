# Windows natív ellenőrzések — P21

P21 saját tesztfája. A tervezett `qualification.ps1` és a natív workflow-harness **még nincs implementálva**. A mappa és a [P21 eredménynapló](../../docs/development/results/P21.md) az indulást készíti elő.

Az első valódi Windows 11 x64 kör a [Windows-validáció](../../docs/windows-validation.md) alapján rögzítse a forrás-SHA-t, OS/CPU-vendort, VS/MSVC/SDK/NuGet/Rust verziókat, restore/MSBuild/link/launch parancsokat és eredményüket. Intel és AMD bizonyítéka külön scope. A teljes `tests/windows_native/` P21-é; a meglévő hordozható `tests/windows_text_adapter.cpp` is P21-tulajdon.

Natív menetek: Unicode/BOM/LF/CRLF/emoji, RichEdit paragraph és UTF-16 range; IME egyszeri commit és hibás commit rollback; Ctrl+Z/Y és prompt/kereső fókusza; named pipe/descriptor ACL és hibás token; közvetlen MCP-edit → Save → GUI-undo/redo → újramentés → latexmk/XeLaTeX → friss PDF; cancel/timeout/app-close folyamatfa; DPI/AltGr/Narrator és PDF. A Mac-referencia és az eltérés a [paritásmátrixhoz](../../docs/development/PLATFORM_PARITY.md) kötött.

Egy boolean evidence-validátor PASS csak modellteszt. Windows Server Rust CI vagy Macen futtatott C++ helper nem WinUI-futtatás. Nincs tesztstub, amely nulla natív eset mellett zöld minősítést ad. Nyers log/binary/session a helyi ignorált helyen marad; a verziózott eredménynapló redaktált, reprodukálható összegzés.
