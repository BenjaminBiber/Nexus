#!/bin/bash
# Spielt das Board-Schema ein: Login, Datenbanken und Rechte (000), dann fehlende Schema-Skripte (001 ff.)
# in AiHarness und AiHarness_Test. Wiederholbar: eingespielte Versionen (board.SchemaVersion) werden übersprungen.
set -euo pipefail

SQLCMD=(/opt/mssql-tools18/bin/sqlcmd -C -b -S mssql -U sa -P "$MSSQL_SA_PASSWORD")

echo "▸ Login ${HARNESS_DB_USER}"
"${SQLCMD[@]}" -Q "IF SUSER_ID(N'${HARNESS_DB_USER}') IS NULL CREATE LOGIN [${HARNESS_DB_USER}] WITH PASSWORD = N'${HARNESS_DB_PASSWORD}';"

echo "▸ Datenbanken und Rechte (000_create_databases.sql)"
sed "s/<LOGIN>/${HARNESS_DB_USER}/g" /sql/000_create_databases.sql > /tmp/000.sql
"${SQLCMD[@]}" -i /tmp/000.sql

for db in AiHarness AiHarness_Test; do
  current=$("${SQLCMD[@]}" -d "$db" -h -1 -W -Q "SET NOCOUNT ON; IF OBJECT_ID(N'board.SchemaVersion') IS NULL SELECT 0 ELSE SELECT ISNULL(MAX(Version), 0) FROM board.SchemaVersion;" | tr -d '[:space:]')
  echo "▸ ${db}: Schema-Version ${current}"
  for file in /sql/[0-9][0-9][0-9]_*.sql; do
    version=$((10#$(basename "$file" | cut -c1-3)))
    if [ "$version" -ge 1 ] && [ "$version" -gt "$current" ]; then
      echo "  $(basename "$file")"
      "${SQLCMD[@]}" -d "$db" -i "$file"
    fi
  done
done

echo "Fertig."
