#!/bin/sh
# [150 / 16A-7] Reglas compartidas para decidir si una copia de los ARCHIVOS
# FÍSICOS (uploads) sirve. Se carga con `. <ruta>`; no ejecuta nada por sí sola.
#
# POSIX sh a propósito, igual que `lib/dump-postgres.sh`: la usan el script del
# host (bash) y el sidecar `uploads-backup` (busybox sh dentro de una imagen
# alpine), para que el camino manual y el automático NO puedan divergir.
#
# [por que] La base de datos no es el único dato que debe perdurar. Los samples,
# avatares y portadas viven en /data/kamples/uploads y no están en ningún dump de
# PostgreSQL. La lección del incidente vale igual aquí: una copia que no se
# verifica no es una copia, porque parece una. Un `.tar.gz` cortado a la mitad
# se sigue abriendo, y solo se nota el día que se necesita.

# Un tar por debajo de esto no puede contener nada útil. No es el criterio de
# validez (lo es el recuento de abajo), solo la red contra el archivo de 0 bytes
# que deja un `tar` interrumpido antes de escribir la cabecera.
TAR_MIN_BYTES=${TAR_MIN_BYTES:-64}

# uploads_tamano <archivo> -> tamaño en bytes. !=0 si no existe.
uploads_tamano() {
    [ -f "$1" ] || return 1
    stat -c '%s' "$1" 2>/dev/null || stat -f '%z' "$1" 2>/dev/null
}

# uploads_contar <dir> -> número de ficheros regulares dentro (los directorios
# no cuentan: `tar` los lista con barra final y se descartan al comparar).
uploads_contar() {
    find "$1" -type f 2>/dev/null | wc -l | tr -d ' '
}

# uploads_empaquetar <origen> <destino> -> escribe el tar.gz en <destino>.
# El -C al padre es a propósito: dentro del tar no deben quedar rutas absolutas,
# para poder extraerlo en cualquier sitio.
uploads_empaquetar() {
    tar czf "$2" -C "$(dirname "$1")" "$(basename "$1")"
}

# uploads_listar <tar> <lista> -> vuelca el índice del tar. !=0 si el tar no es
# legible (es lo que detecta un gzip cortado a la mitad).
uploads_listar() {
    tar tzf "$1" > "$2"
}

# uploads_verificar <tar> <lista> <esperados> [min-bytes]
#   0  = el tar se abre y trae exactamente los ficheros que había en el origen.
#   !=0 = no sirve; el motivo va a stderr.
#
# <lista> es la salida de `tar tzf` sobre el tar. Se recibe como archivo para
# que esta función sea pura inspección de texto, igual que `dump_verificar`.
uploads_verificar() {
    _uv_tar=$1
    _uv_lista=$2
    _uv_esperados=$3
    _uv_min=${4:-$TAR_MIN_BYTES}

    if [ ! -f "$_uv_tar" ]; then
        echo "no existe el archivo: $_uv_tar" >&2
        return 1
    fi

    _uv_bytes=$(uploads_tamano "$_uv_tar") || _uv_bytes=0
    if [ "$_uv_bytes" -lt "$_uv_min" ]; then
        echo "tar demasiado pequeno (${_uv_bytes} bytes < ${_uv_min}): cortado o vacio" >&2
        return 1
    fi

    # Un tar que se abre pero trae menos ficheros que el origen es la copia
    # incompleta: se empaquetó mientras algo se movía, o se truncó después.
    _uv_incluidos=$(grep -vc '/$' "$_uv_lista" 2>/dev/null || true)
    if [ "${_uv_incluidos:-0}" -ne "$_uv_esperados" ]; then
        echo "el tar trae ${_uv_incluidos} archivos y el origen tiene ${_uv_esperados}" >&2
        return 1
    fi

    return 0
}

# uploads_nombre_valido <nombre> -> 0 si es una copia nuestra reconocible. Evita
# que un `find -delete` o un `ls` se lleven por delante un archivo ajeno que
# alguien haya dejado en el directorio de copias.
uploads_nombre_valido() {
    case "$1" in
        uploads-*.tar.gz) return 0 ;;
        *) return 1 ;;
    esac
}
