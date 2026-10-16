#!/bin/sh
# [16A-7] Loop de backup de los ARCHIVOS FÍSICOS, ejecutado por el sidecar
# `uploads-backup`. Hasta ahora esta copia solo existía en su versión manual
# (scripts/backup-uploads.sh), así que en la práctica los uploads podían llevar
# meses sin copia mientras la base sí se copiaba cada 6 h.
#
# - Empaqueta $UPLOADS_DIR en un .tar.gz cada $UPLOADS_INTERVAL_SECONDS.
# - VERIFICA antes de aceptarlo (reglas en scripts/lib/tar-uploads.sh).
# - Rota copias más antiguas que $UPLOADS_RETENTION_DAYS, conservando siempre
#   la más reciente aunque supere la retención.
# - Falla loud (no silencia errores).
#
# Para restaurar: `tar xzf uploads-….tar.gz -C <padre-de-uploads>`. El tar no
# guarda rutas absolutas a propósito (se empaqueta con -C al padre), así que se
# extrae donde se quiera. El directorio de uploads NO se borra nunca desde aquí:
# este script solo lee de él (el sidecar lo monta :ro).

set -eu

# Reglas de verificación compartidas con el script manual del host. Se monta en
# la imagen del sidecar desde docker-compose.yml.
. /usr/local/bin/tar-uploads.sh

ORIGEN="${UPLOADS_DIR:-/uploads}"
DESTINO="${UPLOADS_DUMPS_DIR:-/uploads_dumps}"
INTERVAL="${UPLOADS_INTERVAL_SECONDS:-86400}"
RETENTION_DAYS="${UPLOADS_RETENTION_DAYS:-14}"
PATRON='uploads-*.tar.gz'
# Umbral de ranciedad: dos intervalos sin una copia buena es una avería, no una
# espera normal.
# Dos intervalos, redondeando HACIA ARRIBA: `find -mmin` cuenta minutos
# enteros, así que `INTERVAL/60*2` con un intervalo corto (pruebas) daba 0, y
# `-mmin +0` es cierto para casi todo: un aviso permanente que no avisa de nada.
RANCIEDAD_MIN=$(( (INTERVAL * 2 + 59) / 60 ))
FALLOS=0
MARCA_OK="$DESTINO/.ultima-copia-ok"

if [ ! -d "$ORIGEN" ]; then
    echo "[backup-uploads] ERROR: no existe el directorio de uploads: $ORIGEN" >&2
    exit 1
fi

mkdir -p "$DESTINO"

echo "[backup-uploads] iniciando loop. intervalo=${INTERVAL}s retencion=${RETENTION_DAYS}d origen=${ORIGEN}"

while true; do
    TS=$(date -u +%Y%m%dT%H%M%SZ)
    OUT="$DESTINO/uploads-${TS}-auto.tar.gz"
    TMP="${OUT}.partial"
    LISTA="${OUT}.lista"

    # Aviso de ranciedad: si la última copia buena es más vieja que dos
    # intervalos, algo lleva rato fallando. `find -mmin +N` solo imprime el
    # archivo si es más antiguo, así que un error de find calla en vez de dar
    # una falsa alarma.
    if [ -f "$MARCA_OK" ] && [ -n "$(find "$MARCA_OK" -mmin +"$RANCIEDAD_MIN" 2>/dev/null)" ]; then
        echo "[backup-uploads] ALERTA: la ultima copia buena tiene mas de ${RANCIEDAD_MIN} min" >&2
    fi

    # Se cuenta el origen ANTES de empaquetar: es el número contra el que se
    # compara lo que quedó dentro del tar.
    ESPERADOS=$(uploads_contar "$ORIGEN")
    echo "[backup-uploads] $(date -u -Iseconds) empaquetando ${ESPERADOS} archivos -> $OUT"

    if uploads_empaquetar "$ORIGEN" "$TMP" 2>&1 \
        && uploads_listar "$TMP" "$LISTA" 2>/dev/null \
        && uploads_verificar "$TMP" "$LISTA" "$ESPERADOS"; then
        BYTES=$(uploads_tamano "$TMP")
        mv "$TMP" "$OUT"
        rm -f "$LISTA"
        FALLOS=0
        date -u -Iseconds > "$MARCA_OK"
        echo "[backup-uploads] $(date -u -Iseconds) copia OK ${BYTES} bytes"
    else
        FALLOS=$((FALLOS + 1))
        rm -f "$TMP" "$LISTA"
        echo "[backup-uploads] $(date -u -Iseconds) ERROR: copia descartada (fallo consecutivo ${FALLOS}); las anteriores siguen intactas" >&2
        if [ "$FALLOS" -ge 3 ]; then
            echo "[backup-uploads] ALERTA: ${FALLOS} fallos consecutivos; revisa espacio en disco y permisos de $ORIGEN" >&2
        fi
    fi

    # Rotación: se conserva SIEMPRE la copia más reciente, aunque supere la
    # retención. Si el backup lleva días fallando, borrar la última buena
    # dejaría los archivos sin ninguna copia.
    MAS_NUEVO=$(ls -1t "$DESTINO"/$PATRON 2>/dev/null | head -n1 || true)
    if [ -n "$MAS_NUEVO" ]; then
        find "$DESTINO" -maxdepth 1 -name "$PATRON" -type f \
            -mtime +"$RETENTION_DAYS" ! -path "$MAS_NUEVO" -delete 2>/dev/null || true
    else
        echo "[backup-uploads] sin copias que rotar (todavia no hay ninguna buena)" >&2
    fi

    sleep "$INTERVAL"
done
