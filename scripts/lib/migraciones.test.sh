#!/usr/bin/env bash
# [150] Prueba de `migracion_es_destructiva`. Es shell puro: no necesita
# servidor, ni base de datos, ni compilar nada.
#
#   ./scripts/lib/migraciones.test.sh
#
# [por que] El detector decide si una migración se puede aplicar sin revisión.
# Si se calla ante un DROP, la guardia es decorativa; si grita ante un
# ADD COLUMN, se acabará ignorando. Las dos mitades se prueban aquí: los casos
# que DEBE detectar (positivos, que son el testigo de que el barrido funciona)
# y los que NO debe marcar (falsos positivos).

set -uo pipefail

AQUI=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=migraciones.sh
. "$AQUI/migraciones.sh"

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

PASAN=0
FALLAN=0

# comprobar <debe-detectar: si|no> <descripcion> <sql>
comprobar() {
    local espera=$1 desc=$2 sql=$3
    local f="$TMP/caso.sql"
    printf '%s\n' "$sql" > "$f"

    local detecta="no"
    if migracion_es_destructiva "$f" > /dev/null; then
        detecta="si"
    fi

    if [ "$detecta" = "$espera" ]; then
        PASAN=$((PASAN + 1))
        printf '  ok   %-58s (%s)\n' "$desc" "$detecta"
    else
        FALLAN=$((FALLAN + 1))
        printf '  FALLO %-57s esperaba %s, dio %s\n' "$desc" "$espera" "$detecta" >&2
    fi
}

echo "migracion_es_destructiva:"

# --- Positivos: lo que la guardia existe para parar ------------------------
comprobar si "DROP TABLE"                    'DROP TABLE foo;'
comprobar si "drop table minusculas y CASCADE" 'drop table if exists x cascade;'
comprobar si "DROP COLUMN"                   'ALTER TABLE t DROP COLUMN c;'
comprobar si "DROP SCHEMA"                   'DROP SCHEMA viejo CASCADE;'
comprobar si "TRUNCATE"                      'TRUNCATE TABLE t;'
comprobar si "DELETE FROM sin WHERE"         'DELETE FROM usuarios;'
comprobar si "cambio de tipo de columna"     'ALTER TABLE t ALTER COLUMN c TYPE text;'
comprobar si "sentencia partida en varias lineas" 'ALTER TABLE t
    DROP COLUMN c;'

# --- Negativos: lo que NO debe marcar -------------------------------------
comprobar no "DELETE con WHERE"              'DELETE FROM t WHERE id = 1;'
comprobar no "ADD COLUMN"                    'ALTER TABLE t ADD COLUMN c int;'
comprobar no "DROP INDEX"                    'DROP INDEX idx_t;'
comprobar no "DROP TRIGGER"                  'DROP TRIGGER trg ON t;'
comprobar no "DROP CONSTRAINT"               'ALTER TABLE t DROP CONSTRAINT fk_t;'
comprobar no "CREATE TABLE"                  'CREATE TABLE t (id BIGINT PRIMARY KEY);'
comprobar no "comentario que menciona DROP"  '-- DROP TABLE foo (esto es un comentario)
CREATE TABLE t (id BIGINT);'
comprobar no "columna llamada deleted_at"    'ALTER TABLE t ADD COLUMN deleted_at TIMESTAMPTZ;'

# --- El archivo real del repo ---------------------------------------------
# Testigo de que el detector mira de verdad las migraciones de este proyecto,
# no solo cadenas inventadas.
REAL="$AQUI/../../migrations/20260418000031_colecciones_guardadas.up.sql"
if [ -f "$REAL" ]; then
    if migracion_es_destructiva "$REAL" > /dev/null; then
        PASAN=$((PASAN + 1))
        echo "  ok   migracion real con DROP detectada"
    else
        FALLAN=$((FALLAN + 1))
        echo "  FALLO migracion real con DROP NO detectada" >&2
    fi
else
    FALLAN=$((FALLAN + 1))
    echo "  FALLO no encuentro $REAL" >&2
fi

echo
if [ "$FALLAN" -gt 0 ]; then
    echo "${PASAN} ok, ${FALLAN} fallo(s)" >&2
    exit 1
fi
echo "${PASAN} ok, 0 fallos"
