#!/bin/sh
# [150] Reglas compartidas para decidir si un volcado de PostgreSQL es
# aceptable. Se carga con `. <ruta>`; no ejecuta nada por sí sola.
#
# POSIX sh a propósito: la usan el script del host (bash) y el sidecar de
# backup (busybox sh dentro de `postgres:16-alpine`), para que el camino
# manual y el automático NO puedan divergir.
#
# [por que] El incidente que hay que evitar no fue «no había backup»: fue una
# restauración que dejó la base vacía. El archivo existía, tenía un nombre
# plausible y estaba vacío o truncado. Un backup que no se verifica no es un
# backup, y una restauración sin verificar antes es la forma más rápida de
# destruir datos. Todo lo que decide «esto sirve» vive aquí.

# Suelo de tamaño. Es una red de seguridad contra el archivo de 0 bytes, NO el
# criterio de validez: medido en la 89 con pg_dump real, una base recién creada
# y sin tablas pesa 913 bytes y una con una tabla de 500 filas pesa 3416, así
# que un umbral «generoso» rechazaría bases pequeñas legítimas. El criterio de
# verdad es la entrada TABLE DATA, más abajo.
DUMP_MIN_BYTES=${DUMP_MIN_BYTES:-512}

# dump_tamano <archivo> -> imprime el tamaño en bytes. !=0 si no existe.
dump_tamano() {
    [ -f "$1" ] || return 1
    stat -c '%s' "$1" 2>/dev/null || stat -f '%z' "$1" 2>/dev/null
}

# dump_verificar <dump> <lista-toc> [min-bytes]
#   0  = el archivo es un dump de pg_dump legible y con esquema dentro.
#   !=0 = no lo es; el motivo va a stderr.
#
# <lista-toc> es la salida de `pg_restore --list` sobre el dump. Se recibe como
# archivo, y no se invoca pg_restore aquí, para que esta función sea pura
# inspección de texto: así el host puede generar la lista con
# `docker compose exec` y el sidecar en local, sin duplicar las reglas.
dump_verificar() {
    _dv_dump=$1
    _dv_lista=$2
    _dv_min=${3:-$DUMP_MIN_BYTES}

    if [ ! -f "$_dv_dump" ]; then
        echo "no existe el archivo: $_dv_dump" >&2
        return 1
    fi

    _dv_bytes=$(dump_tamano "$_dv_dump") || _dv_bytes=0
    if [ "$_dv_bytes" -lt "$_dv_min" ]; then
        echo "dump demasiado pequeno (${_dv_bytes} bytes < ${_dv_min}): truncado o vacio" >&2
        return 1
    fi

    if [ ! -s "$_dv_lista" ]; then
        echo "la lista de contenido (pg_restore --list) esta vacia: no es un dump de pg_dump" >&2
        return 1
    fi

    # La firma exacta de una base vacía es no tener NINGUNA entrada TABLE DATA:
    # el esquema no llegó a capturarse. Un dump legítimo lista una por tabla,
    # aunque la tabla tenga 0 filas, así que esto no descarta una base nueva.
    _dv_tablas=$(grep -c 'TABLE DATA' "$_dv_lista" 2>/dev/null || true)
    if [ "${_dv_tablas:-0}" -lt 1 ]; then
        echo "el dump no tiene ninguna entrada TABLE DATA: se capturo una base sin esquema" >&2
        return 1
    fi

    return 0
}

# dump_nombre_valido <nombre> -> 0 si el nombre es un dump nuestro reconocible.
# Evita que un `find -delete` o un `ls` se lleven por delante un archivo ajeno
# que alguien haya dejado en el directorio de dumps.
dump_nombre_valido() {
    case "$1" in
        *.dump) return 0 ;;
        *) return 1 ;;
    esac
}

# Cómo se ejecuta un comando DENTRO del contenedor de postgres. Es la única
# pieza que cambia entre entornos, y por eso es una variable:
#
#   - Host con el stack delante (pruebas, otro servidor):
#       PG_EXEC='docker compose exec -T postgres'
#   - Producción en Coolify: AGENTS §10 prohíbe el SSH directo y el Docker
#     remoto, así que la única vía es coolify-manager-rs, que se ejecuta en el
#     portátil:
#       PG_EXEC='cm exec --name kamples --target postgres --'
#
# No lleva comillas al usarse a propósito: tiene que expandirse a varios
# argumentos.
PG_EXEC=${PG_EXEC:-docker compose exec -T postgres}

# pg_en_contenedor <cmd> [args…] -> ejecuta <cmd> dentro del contenedor de
# postgres con el ejecutor configurado. La expansión sin comillas de PG_EXEC
# vive solo aquí, para que el resto de los scripts no la repita.
pg_en_contenedor() {
    # shellcheck disable=SC2086
    $PG_EXEC "$@"
}
