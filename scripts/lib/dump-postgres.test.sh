#!/usr/bin/env bash
# [150] Prueba de `dump_verificar`. Shell puro, sin base de datos: se le dan
# archivos de mentira y listas de contenido de mentira.
#
#   ./scripts/lib/dump-postgres.test.sh
#
# [por que] Esta función es la que decide si un archivo es un backup válido y,
# por tanto, la que impide que una restauración se lleve por delante la base
# con un dump vacío. Los cuatro rechazos importan tanto como la aceptación:
# un verificador que acepta todo es peor que no tener ninguno, porque da
# confianza.

set -uo pipefail

AQUI=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=dump-postgres.sh
. "$AQUI/dump-postgres.sh"

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

PASAN=0
FALLAN=0

# comprobar <aceptar|rechazar> <descripcion> <preparacion>
# La preparacion deja en $DUMP y $LISTA los archivos a verificar.
comprobar() {
    local espera=$1 desc=$2 prepara=$3
    local DUMP="$TMP/caso.dump" LISTA="$TMP/caso.toc"
    rm -f "$DUMP" "$LISTA"
    eval "$prepara"

    local veredicto="rechazar"
    if dump_verificar "$DUMP" "$LISTA" 2>/dev/null; then
        veredicto="aceptar"
    fi

    if [ "$veredicto" = "$espera" ]; then
        PASAN=$((PASAN + 1))
        printf '  ok   %-52s (%s)\n' "$desc" "$veredicto"
    else
        FALLAN=$((FALLAN + 1))
        printf '  FALLO %-51s esperaba %s, dio %s\n' "$desc" "$espera" "$veredicto" >&2
    fi
}

# Un dump bueno: pasa del mínimo y su TOC lista datos de tabla. Los 2000 bytes
# son a propósito: un dump real y pequeño de la 89 pesa 3416, así que este caso
# es la regresión del umbral que llegó a rechazarlo por tenerlo a 4096.
BUENO="head -c 2000 /dev/zero > \"\$DUMP\"; printf '1; 0 0 TABLE DATA public usuarios postgres\n' > \"\$LISTA\""

echo "dump_verificar:"

# --- Acepta lo que debe ----------------------------------------------------
comprobar aceptar "dump con tamano y esquema" "$BUENO"

# --- Rechaza lo que debe (cada uno es una forma real de perder datos) ------
comprobar rechazar "archivo inexistente" \
    'true'
comprobar rechazar "dump vacio (0 bytes)" \
    ': > "$DUMP"; printf "1; 0 0 TABLE DATA public t postgres\n" > "$LISTA"'
comprobar rechazar "dump truncado (por debajo del minimo)" \
    'head -c 100 /dev/zero > "$DUMP"; printf "1; 0 0 TABLE DATA public t postgres\n" > "$LISTA"'
comprobar rechazar "no es un dump (lista de contenido vacia)" \
    'head -c 8192 /dev/zero > "$DUMP"; : > "$LISTA"'
comprobar rechazar "base capturada sin esquema (sin TABLE DATA)" \
    'head -c 8192 /dev/zero > "$DUMP"; printf "5; 2615 16403 SCHEMA public postgres\n" > "$LISTA"'

# --- El umbral es configurable --------------------------------------------
DUMP_MIN_BYTES=100
comprobar aceptar "umbral bajado a 100 bytes acepta 200" \
    'head -c 200 /dev/zero > "$DUMP"; printf "1; 0 0 TABLE DATA public t postgres\n" > "$LISTA"'
DUMP_MIN_BYTES=512

# --- El ejecutor dentro del contenedor es intercambiable ------------------
# `PG_EXEC` tiene que expandirse a varios argumentos (en producción es
# `cm exec --name kamples --target postgres --`). Un ejecutor mal pasado
# rompería todos los scripts a la vez, así que se prueba con un doble.
STUB="$TMP/stub.sh"
cat > "$STUB" <<'STUB_FIN'
#!/bin/sh
printf '%s\n' "$@"
STUB_FIN
chmod +x "$STUB"

PG_EXEC="bash $STUB --bandera"
SALIDA=$(pg_en_contenedor pg_dump --format=custom)
ESPERADO=$(printf '%s\n' --bandera pg_dump --format=custom)
if [ "$SALIDA" = "$ESPERADO" ]; then
    PASAN=$((PASAN + 1))
    echo "  ok   PG_EXEC se expande a varios argumentos"
else
    FALLAN=$((FALLAN + 1))
    printf '  FALLO PG_EXEC mal expandido\n    esperado: %s\n    obtenido: %s\n' \
        "$(printf '%s' "$ESPERADO" | tr '\n' ' ')" "$(printf '%s' "$SALIDA" | tr '\n' ' ')" >&2
fi

echo
if [ "$FALLAN" -gt 0 ]; then
    echo "${PASAN} ok, ${FALLAN} fallo(s)" >&2
    exit 1
fi
echo "${PASAN} ok, 0 fallos"
