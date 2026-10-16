#!/usr/bin/env bash
# [150 / 16A-7] Copia de los ARCHIVOS FÍSICOS (uploads). La base de datos no es
# el único dato que debe perdurar: los samples, avatares y portadas viven en
# /data/kamples/uploads y no están en ningún dump de PostgreSQL.
#
# Uso (desde el host del servidor, en el directorio del stack):
#   ./scripts/backup-uploads.sh [etiqueta]
#
# Sale un .tar.gz en $UPLOADS_DUMPS_DIR. Igual que el dump de la base, se
# escribe a .partial, se VERIFICA y solo entonces se renombra: un tar a medias
# es peor que ninguno, porque parece una copia.
#
# [16A-7] Las reglas de verificación viven en scripts/lib/tar-uploads.sh y las
# comparte el sidecar `uploads-backup` de docker-compose.yml. Este script es la
# vía manual; el sidecar, la automática. Ninguna de las dos decide por su cuenta
# si una copia sirve.

set -euo pipefail

AQUI=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=lib/tar-uploads.sh
. "$AQUI/lib/tar-uploads.sh"

LABEL="${1:-manual}"
ORIGEN="${UPLOADS_DIR:-/data/kamples/uploads}"
# Configurable para poder probar el script sin tocar el directorio real del
# servidor (y para mover las copias a otro disco sin editar el script).
DESTINO="${UPLOADS_DUMPS_DIR:-/data/kamples/uploads_dumps}"
RETENTION_DAYS="${UPLOADS_RETENTION_DAYS:-30}"
TS=$(date -u +%Y%m%dT%H%M%SZ)

if [ ! -d "$ORIGEN" ]; then
    echo "[backup-uploads] no existe el directorio de uploads: $ORIGEN" >&2
    exit 1
fi

mkdir -p "$DESTINO"

NOMBRE="uploads-${TS}-${LABEL}.tar.gz"
OUT="$DESTINO/$NOMBRE"
PARCIAL="$OUT.partial"
LISTA=$(mktemp)

limpiar() { rm -f "$PARCIAL" "$LISTA"; }
trap limpiar EXIT

# Cuántos archivos hay que esperar del otro lado. Se mide antes para poder
# comparar después: un tar que se corta a medias sigue siendo un tar válido.
ESPERADOS=$(uploads_contar "$ORIGEN")

echo "[backup-uploads] empaquetando $ORIGEN ($ESPERADOS archivos) -> $OUT"

uploads_empaquetar "$ORIGEN" "$PARCIAL"
uploads_listar "$PARCIAL" "$LISTA"
uploads_verificar "$PARCIAL" "$LISTA" "$ESPERADOS"

BYTES=$(uploads_tamano "$PARCIAL")
mv "$PARCIAL" "$OUT"
echo "[backup-uploads] OK $OUT (${BYTES} bytes, ${ESPERADOS} archivos)"

# Rotación, conservando siempre el más reciente aunque supere la retención: si
# el backup lleva días fallando, borrar el último bueno dejaría los archivos sin
# ninguna copia, que es justo el escenario que esto evita.
MAS_NUEVO=$(ls -1t "$DESTINO"/uploads-*.tar.gz 2>/dev/null | head -n1 || true)
if [ -n "$MAS_NUEVO" ]; then
    find "$DESTINO" -maxdepth 1 -name 'uploads-*.tar.gz' -type f \
        -mtime +"$RETENTION_DAYS" ! -path "$MAS_NUEVO" -delete 2>/dev/null || true
fi
