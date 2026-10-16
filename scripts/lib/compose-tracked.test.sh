#!/usr/bin/env bash
# [16A-7] Guarda: todo script que docker-compose.yml monta tiene que EXISTIR, y
# si hay git, tiene que estar versionado.
#
#   ./scripts/lib/compose-tracked.test.sh
#
# [por que] `.gitignore` tiene `/scripts/*` con lista blanca, así que un script
# nuevo queda fuera del repo **en silencio** salvo que se le añada su `!`. Pasó
# de verdad con `backup-uploads-loop.sh`: el compose lo montaba y el repo no lo
# llevaba. En un clon limpio el sidecar no arranca, y como el resto del stack
# sigue en pie, **parece que hay copias cuando no hay ninguna**. Es el mismo
# engaño que la nota de protección de datos quiere evitar, solo que un paso
# antes: no la copia que no se verifica, sino la copia que no existe.
#
# Se comprueba contra el compose real, no contra una lista escrita a mano: una
# lista a mano se queda vieja justo cuando aparece el sidecar nuevo.

set -uo pipefail

AQUI=$(cd "$(dirname "$0")" && pwd)
RAIZ=$(cd "$AQUI/../.." && pwd)
COMPOSE="$RAIZ/docker-compose.yml"

PASAN=0
FALLAN=0

anotar() {
    local ok=$1 desc=$2 detalle=${3:-}
    if [ "$ok" = 1 ]; then
        PASAN=$((PASAN + 1))
        printf '  ok   %s\n' "$desc"
    else
        FALLAN=$((FALLAN + 1))
        printf '  FALLO %s%s\n' "$desc" "${detalle:+ ($detalle)}" >&2
    fi
}

[ -f "$COMPOSE" ] || { echo "no encuentro $COMPOSE" >&2; exit 1; }

# Rutas `./scripts/...` que el compose monta o ejecuta.
RUTAS=$(grep -oE '\./scripts/[A-Za-z0-9_./-]+' "$COMPOSE" | sort -u)
# Testigo: si el barrido dejara de encontrar nada (compose reescrito, otra
# sintaxis), el test tiene que fallar, no pasar por vacío.
if [ -z "$RUTAS" ]; then
    echo "el barrido no encontro ninguna ruta ./scripts/ en docker-compose.yml" >&2
    echo "el compose cambio de forma: revisa el patron antes de dar esto por bueno" >&2
    exit 1
fi

EN_GIT=0
if command -v git >/dev/null 2>&1 && git -C "$RAIZ" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    EN_GIT=1
else
    echo "  --   sin git aqui: solo se comprueba que los archivos existen"
fi

echo "scripts montados por docker-compose.yml:"

for ruta in $RUTAS; do
    rel=${ruta#./}
    if [ ! -f "$RAIZ/$rel" ]; then
        anotar 0 "$rel existe" "el compose lo monta y no esta en el arbol"
        continue
    fi
    anotar 1 "$rel existe"

    [ "$EN_GIT" = 1 ] || continue
    if git -C "$RAIZ" ls-files --error-unmatch "$rel" >/dev/null 2>&1; then
        anotar 1 "$rel esta versionado"
    else
        # El caso que muerde: existe en local, el repo no lo lleva.
        anotar 0 "$rel esta versionado" \
            "ignorado o sin add: añade '!/$rel' a .gitignore y commitealo"
    fi
done

echo
if [ "$FALLAN" -gt 0 ]; then
    echo "${PASAN} ok, ${FALLAN} fallo(s)" >&2
    exit 1
fi
echo "${PASAN} ok, 0 fallos"
