# P21 natív kvalifikáció

Ide kerül a [P21 terv](../../../../docs/superpowers/plans/2026-10-31-parallel/p21-windows.md) Windows-specifikus kvalifikációs adaptere. A mappa most fejlesztői belépési pont; nincs benne kész szolgáltatás.

A meglévő alkalmazás a `MainWindow*`, `NativeCore*` és projektfájlokban javítható P21 tulajdonban. Ne kerüljön ide második editor-runtime vagy másolt Rust-üzleti logika. A `WindowsEvidence` modell helyes elutasítása csak modellteszt; a tényleges WinUI-build, indítás és natív workflow külön bizonyítékot igényel a [natív teszttérben](../../../../tests/windows_native/README.md).
