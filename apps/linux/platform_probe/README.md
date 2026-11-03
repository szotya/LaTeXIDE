# Linux runtime-directory és IPC próba

A [P22 terv](../../../docs/superpowers/plans/2026-10-31-parallel/p22-linux.md) első saját Rust-próbájának helye. A `main.rs` és a [natív teszt](../../../tests/linux_native/README.md) még nem létezik; ez fejlesztői mappakijelölés, nem megvalósítás.

A próba valódi, saját ideiglenes Linux runtime-directoryn vizsgálja az XDG útvonalat, tulajdonost, 0700 módot, symlinket, hiányzó/hibás könyvtárat és Unix socket roundtripet. A közös sessionhöz kapcsolás P00-integráció. Ne létesíts teszt kedvéért `/private/tmp` könyvtárat, és ne helyettesíts tényleges fájlrendszeradatot a teszt által megadott UID/mode értékkel.
