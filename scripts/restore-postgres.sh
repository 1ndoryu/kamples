#!/usr/bin/env bash
# [150] Restauración de PostgreSQL con red de seguridad. Es la ÚNICA vía
# sancionada para restaurar: `pg_restore --clean --if-exists` a mano es lo que
# dejó una base vacía en el incidente que esta viñeta existe para evitar.
#
# Uso:
#   ./scripts/restore-postgres.sh <archivo.dump>              # simulación
#   ./scripts/restore-postgres.sh <archivo.dump> --aplicar    # restaura
#
# El ejecutor dentro del contenedor lo elige `PG_EXEC` (ver
# scripts/lib/dump-postgres.sh). En producción, donde AGENTS §10 prohíbe el SSH
# directo, es obligatorio el de coolify-manager-rs:
#   PG_EXEC='cm exec --name kamples --target postgres --' ./scripts/restore-postgres.sh <dump> --aplicar
#
# Garantías, en orden:
#   1. El dump se VERIFICA antes de tocar nada (tamaño, formato, esquema).
#      Un dump vacío o truncado se rechaza aquí, no después.
#   2. Antes de aplicar, se saca una copia de seguridad de la base ACTUAL y se
#      verifica igual. Si esa copia no sale, no se restaura nada.
#   3. Si la base tiene tablas y no se pasa --forzar, se niega: restaurar
#      encima de datos existentes es una decisión consciente, no un accidente.
#   4. La restauración va en UNA transacción (--single-transaction): si falla a
#      medias, PostgreSQL la deshace entera. Nunca queda una base a medio
#      poblar, que es indistinguible de una base perdida.
#   5. Al final se comprueba que la base quedó con tablas; si no, se dice dónde
#      está la copia previa.

set -euo pipefail

AQUI=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/dump-postgres.sh
. "$AQUI/lib/dump-postgres.sh"

DUMP_DIR="${DB_DUMPS_DIR:-/data/kamples/db_dumps}"

APLICAR=0
FORZAR=0
ARCHIVO=""
for arg in "$@"; do
    case "$arg" in
        --aplicar) APLICAR=1 ;;
        --forzar)  FORZAR=1 ;;
        -h|--help)
            awk 'NR>1 && /^#/ { sub(/^# ?/, ""); print; next } NR>1 { exit }' "$0"
            exit 0 ;;
        -*)
            echo "opcion desconocida: $arg" >&2; exit 2 ;;
        *)
            [ -z "$ARCHIVO" ] || { echo "sobra un argumento: $arg" >&2; exit 2; }
            ARCHIVO="$arg" ;;
    esac
done

[ -n "$ARCHIVO" ] || { echo "falta el archivo .dump (ver --help)" >&2; exit 2; }

if [ -f ".env" ]; then
    # shellcheck disable=SC1091
    set -a; . ./.env; set +a
fi
: "${POSTGRES_USER:?POSTGRES_USER no definido}"
: "${POSTGRES_DB:?POSTGRES_DB no definido}"

# --- 1. Verificar el dump ANTES de tocar nada ------------------------------

LISTA=$(mktemp)
limpiar() { rm -f "$LISTA"; }
trap limpiar EXIT

# El dump lo lee pg_restore DENTRO del contenedor, donde el bind-mount del
# host `/data/kamples/db_dumps` se llama `/dumps`. Sin esta traducción, pasar
# la ruta del host falla con «no such file» justo antes de tocar la base.
en_contenedor() {
    case "$1" in
        "$DUMP_DIR"/*) printf '/dumps/%s' "${1#"$DUMP_DIR"/}" ;;
        *)             printf '%s' "$1" ;;
    esac
}

echo "[restore] verificando $ARCHIVO"
pg_en_contenedor pg_restore --list "$(en_contenedor "$ARCHIVO")" > "$LISTA"
dump_verificar "$ARCHIVO" "$LISTA"
BYTES=$(dump_tamano "$ARCHIVO")
TABLAS=$(grep -c 'TABLE DATA' "$LISTA")
echo "[restore] dump aceptado: ${BYTES} bytes, ${TABLAS} entradas de datos de tabla"

# Cuántas tablas hay ahora en la base destino.
CONTAR_TABLAS="SELECT count(*) FROM information_schema.tables WHERE table_schema='public'"
TABLAS_ACTUALES=$(pg_en_contenedor \
    psql -tAq -U "$POSTGRES_USER" -d "$POSTGRES_DB" -c "$CONTAR_TABLAS")

if [ "$APLICAR" -eq 0 ]; then
    echo "[restore] SIMULACION. Nada se ha tocado."
    echo "[restore] destino: ${POSTGRES_DB} en el postgres del stack (${TABLAS_ACTUALES} tablas en public)"
    echo "[restore] para aplicar de verdad: $0 $ARCHIVO --aplicar"
    exit 0
fi

# --- 3. Negarse a pisar datos sin decirlo explícitamente -------------------

if [ "$TABLAS_ACTUALES" -gt 0 ] && [ "$FORZAR" -eq 0 ]; then
    echo "[restore] NEGADO: la base ${POSTGRES_DB} ya tiene ${TABLAS_ACTUALES} tablas." >&2
    echo "[restore] restaurar encima reemplaza su contenido. Si es lo que quieres:" >&2
    echo "[restore]   $0 $ARCHIVO --aplicar --forzar" >&2
    exit 1
fi

# Confirmación escrita: la operación es destructiva e irreversible sin la copia.
if [ -t 0 ]; then
    printf '[restore] vas a reemplazar la base "%s". Escribe su nombre para confirmar: ' "$POSTGRES_DB"
    read -r CONFIRMA
    [ "$CONFIRMA" = "$POSTGRES_DB" ] || { echo "[restore] cancelado" >&2; exit 1; }
else
    [ "$FORZAR" -eq 1 ] || {
        echo "[restore] sin terminal interactiva hace falta --forzar" >&2; exit 1; }
fi

# --- 2. Copia de seguridad de lo que hay AHORA ----------------------------

PREVIO="$DUMP_DIR/${POSTGRES_DB}-prerestore-$(date -u +%Y%m%dT%H%M%SZ).dump"
PREVIO_LISTA="$PREVIO.toc"
echo "[restore] copia de seguridad previa -> $PREVIO"
pg_en_contenedor \
    pg_dump --format=custom --no-owner --no-acl \
        -U "$POSTGRES_USER" -d "$POSTGRES_DB" > "$PREVIO"
pg_en_contenedor pg_restore --list "$(en_contenedor "$PREVIO")" > "$PREVIO_LISTA" 2>/dev/null || true

if ! dump_verificar "$PREVIO" "$PREVIO_LISTA"; then
    echo "[restore] ABORTADO: la copia previa no es válida; no se toca la base." >&2
    echo "[restore] el archivo sospechoso está en $PREVIO (revísalo o bórralo)" >&2
    exit 1
fi
rm -f "$PREVIO_LISTA"
echo "[restore] copia previa OK"

# --- 4. Restaurar en una sola transacción ---------------------------------

echo "[restore] restaurando (una transacción; si falla, se deshace entera)"
pg_en_contenedor \
    pg_restore --clean --if-exists --single-transaction --no-owner --no-acl \
        -U "$POSTGRES_USER" -d "$POSTGRES_DB" "$(en_contenedor "$ARCHIVO")"

# --- 5. Comprobar que la base quedó poblada -------------------------------

TABLAS_DESPUES=$(pg_en_contenedor \
    psql -tAq -U "$POSTGRES_USER" -d "$POSTGRES_DB" -c "$CONTAR_TABLAS")

if [ "$TABLAS_DESPUES" -lt 1 ]; then
    echo "[restore] ATENCION: la base quedó sin tablas en public." >&2
    echo "[restore] copia previa: $PREVIO" >&2
    exit 1
fi

echo "[restore] OK: ${TABLAS_DESPUES} tablas en public (antes ${TABLAS_ACTUALES})"
echo "[restore] copia previa conservada en $PREVIO"
