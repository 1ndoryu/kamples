#!/usr/bin/env bash
# [174A-114 / 150] Backup manual bajo demanda. Es el paso obligatorio ANTES de
# un deploy, de una migración o de cualquier operación riesgosa contra
# producción.
#
# Uso:
#   ./scripts/db/backup-postgres.sh [etiqueta]
#
# El dump queda en /data/kamples/db_dumps/<db>-<ISO8601>-<etiqueta>.dump
# y NO se rota (el sidecar postgres-backup solo rota los suyos).
#
# El ejecutor dentro del contenedor lo elige `PG_EXEC` (ver
# scripts/lib/dump-postgres.sh). En producción, donde AGENTS §10 prohíbe el SSH
# directo, es obligatorio pasar el de coolify-manager-rs:
#   PG_EXEC='cm exec --name kamples --target postgres --' ./scripts/db/backup-postgres.sh pre-deploy
#
# [150 por que] Antes esta función volcaba con
# `docker compose exec -T postgres pg_dump ... > "$OUT"`. Si el exec fallaba,
# el redirect ya había creado $OUT y dejaba un archivo de 0 bytes con nombre
# plausible en el directorio de dumps. Esa es exactamente la firma del
# incidente: una restauración posterior sobre un dump vacío dejó la base sin
# datos. Ahora se vuelca a .partial, se VERIFICA y solo entonces se renombra;
# un fallo no deja nada que alguien pueda confundir con un backup.

set -euo pipefail

AQUI=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=../lib/dump-postgres.sh
. "$AQUI/../lib/dump-postgres.sh"

LABEL="${1:-manual}"
DUMP_DIR="${DB_DUMPS_DIR:-/data/kamples/db_dumps}"
TS=$(date -u +%Y%m%dT%H%M%SZ)

mkdir -p "$DUMP_DIR"

# Carga env del compose (POSTGRES_USER/POSTGRES_DB) si existe .env adyacente.
if [ -f ".env" ]; then
    # shellcheck disable=SC1091
    set -a; . ./.env; set +a
fi

: "${POSTGRES_USER:?POSTGRES_USER no definido}"
: "${POSTGRES_DB:?POSTGRES_DB no definido}"

NOMBRE="${POSTGRES_DB}-${TS}-${LABEL}.dump"
OUT="$DUMP_DIR/$NOMBRE"
PARCIAL="$OUT.partial"
LISTA=$(mktemp)

# El trap borra el parcial y la lista pase lo que pase: si el script muere a
# medias (set -e), no queda basura con aspecto de backup.
limpiar() {
    rm -f "$PARCIAL" "$LISTA"
}
trap limpiar EXIT

echo "[backup-manual] generando $OUT"

# El dump se escribe DENTRO del contenedor y sale por stdout al archivo del
# host, que es quien tiene permiso de escritura en el bind-mount. `pipefail`
# hace que un pg_dump roto dentro del contenedor no se confunda con éxito.
pg_en_contenedor \
    pg_dump --format=custom --no-owner --no-acl \
        -U "$POSTGRES_USER" -d "$POSTGRES_DB" > "$PARCIAL"

# Verificación: `pg_restore --list` valida la cabecera del archivo y da el
# contenido para comprobar que el esquema se capturó de verdad.
pg_en_contenedor pg_restore --list "/dumps/$NOMBRE.partial" > "$LISTA"

dump_verificar "$PARCIAL" "$LISTA"
BYTES=$(dump_tamano "$PARCIAL")

mv "$PARCIAL" "$OUT"
echo "[backup-manual] OK $OUT (${BYTES} bytes)"
