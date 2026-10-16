#!/bin/sh
# [174A-114 / 150] Loop de backup ejecutado por el sidecar postgres-backup.
# - Hace pg_dump custom-format cada $BACKUP_INTERVAL_SECONDS.
# - VERIFICA el dump antes de aceptarlo (ver scripts/lib/dump-postgres.sh).
# - Rota dumps más antiguos que $BACKUP_RETENTION_DAYS, conservando siempre el
#   más reciente aunque supere la retención.
# - Falla loud (no silencia errores).
#
# Para restaurar NO se usa pg_restore a mano: usa scripts/restore-postgres.sh,
# que verifica el dump y saca una copia de seguridad previa antes de tocar
# nada. La instrucción destructiva que había aquí antes
# (`pg_restore --clean --if-exists`) es justo lo que dejó una base vacía: no
# se reintroduce.

set -eu

# Reglas de verificación compartidas con el script manual del host. Se monta
# en la imagen del sidecar desde docker-compose.yml.
. /usr/local/bin/dump-postgres.sh

DUMP_DIR=/dumps
INTERVAL="${BACKUP_INTERVAL_SECONDS:-21600}"
RETENTION_DAYS="${BACKUP_RETENTION_DAYS:-14}"
PATRON="${PGDATABASE}-*.dump"
# Umbral de ranciedad: dos intervalos sin un dump bueno es una avería, no una
# espera normal.
RANCIEDAD_MIN=$(( INTERVAL / 60 * 2 ))
FALLOS=0
MARCA_OK="$DUMP_DIR/.ultimo-backup-ok"

mkdir -p "$DUMP_DIR"

echo "[backup] iniciando loop. intervalo=${INTERVAL}s retencion=${RETENTION_DAYS}d host=${PGHOST} db=${PGDATABASE}"

while true; do
    TS=$(date -u +%Y%m%dT%H%M%SZ)
    OUT="$DUMP_DIR/${PGDATABASE}-${TS}.dump"
    TMP="${OUT}.partial"
    LISTA="${OUT}.toc"

    # Aviso de ranciedad: si el último dump bueno es más viejo que dos
    # intervalos, algo lleva rato fallando. `find -mmin +N` solo imprime el
    # archivo si es más antiguo, así que un error de find calla en vez de
    # dar una falsa alarma.
    if [ -f "$MARCA_OK" ] && [ -n "$(find "$MARCA_OK" -mmin +"$RANCIEDAD_MIN" 2>/dev/null)" ]; then
        echo "[backup] ALERTA: el ultimo dump bueno tiene mas de ${RANCIEDAD_MIN} min" >&2
    fi

    echo "[backup] $(date -u -Iseconds) iniciando dump -> $OUT"

    if pg_dump --format=custom --no-owner --no-acl --file="$TMP" 2>&1 \
        && pg_restore --list "$TMP" > "$LISTA" 2>/dev/null \
        && dump_verificar "$TMP" "$LISTA"; then
        BYTES=$(dump_tamano "$TMP")
        mv "$TMP" "$OUT"
        rm -f "$LISTA"
        FALLOS=0
        date -u -Iseconds > "$MARCA_OK"
        echo "[backup] $(date -u -Iseconds) dump OK ${BYTES} bytes"
    else
        FALLOS=$((FALLOS + 1))
        rm -f "$TMP" "$LISTA"
        echo "[backup] $(date -u -Iseconds) ERROR: dump descartado (fallo consecutivo ${FALLOS}); los anteriores siguen intactos" >&2
        if [ "$FALLOS" -ge 3 ]; then
            echo "[backup] ALERTA: ${FALLOS} fallos consecutivos; revisa espacio en disco y salud de postgres" >&2
        fi
    fi

    # Rotación: se conserva SIEMPRE el dump más reciente, aunque supere la
    # retención. Si el backup lleva días fallando, borrar el último bueno
    # dejaría la base sin ninguna copia, que es el escenario que esto evita.
    MAS_NUEVO=$(ls -1t "$DUMP_DIR"/$PATRON 2>/dev/null | head -n1 || true)
    if [ -n "$MAS_NUEVO" ]; then
        find "$DUMP_DIR" -maxdepth 1 -name "$PATRON" -type f \
            -mtime +"$RETENTION_DAYS" ! -path "$MAS_NUEVO" -delete 2>/dev/null || true
    else
        echo "[backup] sin dumps que rotar (todavia no hay ninguno bueno)" >&2
    fi

    sleep "$INTERVAL"
done
