#!/usr/bin/env bash
# [150] Guardia de migraciones: se niega a dar por bueno SQL que destruye
# datos, salvo que el archivo esté declarado en migrations/.destructivas-permitidas
# con su motivo escrito.
#
# Uso:
#   ./scripts/guardia-migraciones.sh                 # revisa todas las .up.sql
#   ./scripts/guardia-migraciones.sh <archivo.sql>…  # revisa solo esas
#
# Exit: 0 sin hallazgos no declarados, 1 si hay alguno.
#
# [por que] Las migraciones se aplican a mano con psql y no había ninguna
# barrera. Esto no prohíbe el SQL destructivo: obliga a que sea una decisión
# escrita, con motivo, y no una línea que se cuela en un archivo.

set -euo pipefail

AQUI=$(cd "$(dirname "$0")" && pwd)
RAIZ=$(cd "$AQUI/.." && pwd)
# shellcheck source=lib/migraciones.sh
. "$AQUI/lib/migraciones.sh"

PERMITIDAS="$RAIZ/migrations/.destructivas-permitidas"

# ¿Está <nombre-de-archivo> declarado en la lista blanca?
esta_permitida() {
    [ -f "$PERMITIDAS" ] || return 1
    # Formato: <nombre-de-archivo>|<motivo>. Se ignoran comentarios y vacías.
    grep -v '^[[:space:]]*#' "$PERMITIDAS" 2>/dev/null \
        | grep -v '^[[:space:]]*$' \
        | cut -d'|' -f1 \
        | sed 's/[[:space:]]*$//' \
        | grep -Fxq "$1"
}

if [ "$#" -gt 0 ]; then
    ARCHIVOS=("$@")
else
    shopt -s nullglob
    ARCHIVOS=("$RAIZ"/migrations/*.up.sql)
    shopt -u nullglob
fi

if [ "${#ARCHIVOS[@]}" -eq 0 ]; then
    echo "[guardia] no hay migraciones que revisar" >&2
    exit 2
fi

NUEVOS=0
DECLARADOS=0

for f in "${ARCHIVOS[@]}"; do
    [ -f "$f" ] || { echo "[guardia] no existe: $f" >&2; exit 2; }
    nombre=$(basename "$f")

    if hallazgos=$(migracion_es_destructiva "$f"); then
        if esta_permitida "$nombre"; then
            DECLARADOS=$((DECLARADOS + 1))
            echo "[guardia] declarada  $nombre"
            printf '%s\n' "$hallazgos" | sed 's/^/             /'
        else
            NUEVOS=$((NUEVOS + 1))
            echo "[guardia] PELIGROSA  $nombre" >&2
            printf '%s\n' "$hallazgos" | sed 's/^/             /' >&2
        fi
    fi
done

if [ "$NUEVOS" -gt 0 ]; then
    cat >&2 <<FIN

[guardia] ${NUEVOS} migracion(es) con SQL destructivo no declarado.
Si es intencionado, anadelas a migrations/.destructivas-permitidas con el
formato <nombre-de-archivo>|<motivo> y saca copia de seguridad ANTES:
    ./scripts/backup-postgres.sh pre-migracion
FIN
    exit 1
fi

echo "[guardia] OK: ${DECLARADOS} destructiva(s) ya declarada(s), ninguna nueva."
