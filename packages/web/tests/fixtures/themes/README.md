# Test-Themes

`e2e-shell/` ist die Quelle des Test-Themes mit eigener Shell (Slot `Shell`, `main.js` als reines ESM
ohne Build). `e2e-shell.nexus` ist das daraus gebaute Paket für die Ende-zu-Ende-Tests
(`crates/nexus-server/tests/theme_e2e.rs`, `packages/web/tests/ThemeE2EShell.test.tsx`).

Nach jeder Änderung an der Quelle das Paket neu bauen (der Server-Test vergleicht Paket und Quelle):

```
cargo run -p nexus-cli -- theme pack packages/web/tests/fixtures/themes/e2e-shell --out packages/web/tests/fixtures/themes/e2e-shell.nexus
```
