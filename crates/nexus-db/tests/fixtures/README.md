# Testdaten der SQLite-Tests

## `ts-board.db`

Eine leere Board-Datei, **angelegt vom TypeScript-Stand** (`packages/db`). Sie ist der Beweis,
dass Nexus und der alte Harness dieselbe Datei lesen und schreiben: `tests/sqlite_database.rs`
öffnet eine Kopie und stellt fest, dass keine Schema-Stufe nachgeholt wird, und vergleicht
`sqlite_master` einer von Rust angelegten Datei zeichenweise mit dieser.

Erzeugt im Wurzelverzeichnis des Repos mit:

```sh
npm ci            # nur, wenn node_modules fehlt
npm run build
node -e "import('./packages/db/dist/index.js').then(async m => { const db = await m.SqliteBoardDatabase.open('crates/nexus-db/tests/fixtures/ts-board.db'); await db.close(); })"
```

Danach `ts-board.db-wal` und `ts-board.db-shm` entfernen, falls sie übrig bleiben (`close()`
räumt sie in der Regel selbst weg), und nur `ts-board.db` einchecken.

Neu erzeugt wird die Datei nur, wenn das SQLite-Schema in TypeScript eine Stufe dazubekommt –
dann muss auch `crates/nexus-db/src/sqlite/schema/` dieselbe Stufe erhalten.
