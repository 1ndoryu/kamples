#!/usr/bin/env bash
# [150] Backup de los ARCHIVOS FÍSICOS (uploads). La base de datos no es el
# único dato que debe perdurar: los samples, avatares y portadas viven en
# /data/kamples/uploads y no están en ningún dump de PostgreSQL. Hasta ahora
# solo existía un `tar czf` manual escrito en un runbook, así que en la
# práctica no había copia.
#
# Uso (desde el host del servidor, en el directorio del stack Coolify):
#   ./scripts/backup-uploads.sh [etiqueta]
#
# Sale un .tar.gz en /data/kamples/uploads_dumps/. Igual que el dump de la
# base, se escribe a .partial, se VERIFICA y solo entonces se renombra: un tar
# a medias es peor que ninguno, porque parece una copia.

set -euo pipefail

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
ESPERADOS=$(find "$ORIGEN" -type f 2>/dev/null | wc -l | tr -d ' ')

echo "[backup-uploads] empaquetando $ORIGEN ($ESPERADOS archivos) -> $OUT"

# -C al directorio padre para no guardar rutas absolutas dentro del tar.
tar czf "$PARCIAL" -C "$(dirname "$ORIGEN")" "$(basename "$ORIGEN")"

# Verificación: el tar debe listarse sin error y traer el mismo número de
# archivos que había en el origen. Un tar legible pero incompleto es la copia
# que falla justo cuando la necesitas.
tar tzf "$PARCIAL" > "$LISTA"

INCLUIDOS=$(grep -vc '/$' "$LISTA" || true)
if [ "$INCLUIDOS" -ne "$ESPERADOS" ]; then
    echo "[backup-uploads] RECHAZADO: el tar trae ${INCLUIDOS} archivos y el origen tiene ${ESPERADOS}" >&2
    exit 1
fi

BYTES=$(stat -c '%s' "$PARCIAL" 2>/dev/null || stat -f '%z' "$PARCIAL")
mv "$PARCIAL" "$OUT"
echo "[backup-uploads] OK $OUT (${BYTES} bytes, ${INCLUIDOS} archivos)"

# Rotación, conservando siempre el más reciente aunque supere la retención.
MAS_NUEVO=$(ls -1t "$DESTINO"/uploads-*.tar.gz 2>/dev/null | head -n1 || true)
if [ -n "$MAS_NUEVO" ]; then
    find "$DESTINO" -maxdepth 1 -name 'uploads-*.tar.gz' -type f \
        -mtime +"$RETENTION_DAYS" ! -path "$MAS_NUEVO" -delete 2>/dev/null || true
fi
