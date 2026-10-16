#!/usr/bin/env bash
# [16A-7] Pruebas de `scripts/lib/tar-uploads.sh` y del camino manual
# `scripts/backup-uploads.sh`. Shell puro, con tar real y archivos reales: no
# hace falta base de datos ni servidor.
#
#   ./scripts/lib/tar-uploads.test.sh
#
# [por que] Un verificador que acepta todo es peor que no tener ninguno, porque
# da confianza. Los rechazos importan tanto como la aceptación: la copia que
# falla justo cuando se necesita es la que se aceptó sin mirar.

set -uo pipefail

AQUI=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=tar-uploads.sh
. "$AQUI/tar-uploads.sh"
BACKUP="$AQUI/../backup-uploads.sh"

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

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

# comprobar <aceptar|rechazar> <descripcion> <preparacion>
# La preparacion deja en $TAR y $LISTA los archivos a verificar.
comprobar() {
    local espera=$1 desc=$2 prepara=$3
    local TAR="$TMP/caso.tar.gz" LISTA="$TMP/caso.lista"
    rm -f "$TAR" "$LISTA"
    eval "$prepara"

    local veredicto="rechazar"
    if uploads_verificar "$TAR" "$LISTA" "${ESPERADOS:-3}" 2>/dev/null; then
        veredicto="aceptar"
    fi
    anotar "$([ "$veredicto" = "$espera" ] && echo 1 || echo 0)" "$desc" "esperaba $espera, dio $veredicto"
}

# --- uploads_contar y el empaquetado real ---------------------------------
ORIGEN="$TMP/origen/uploads"
mkdir -p "$ORIGEN/muestras/2026" "$ORIGEN/avatares"
printf 'sample uno' > "$ORIGEN/muestras/2026/a.wav"
printf 'sample dos' > "$ORIGEN/muestras/b.wav"
printf 'avatar'     > "$ORIGEN/avatares/u1.png"
# Un directorio vacío: no cuenta como archivo, y tar lo lista con barra final.
mkdir -p "$ORIGEN/vacio"

N=$(uploads_contar "$ORIGEN")
anotar "$([ "$N" = 3 ] && echo 1 || echo 0)" "uploads_contar cuenta 3 ficheros (y no los directorios)" "dio $N"

TAR_BUENO="$TMP/bueno.tar.gz"
uploads_empaquetar "$ORIGEN" "$TAR_BUENO"
LISTA_BUENO="$TMP/bueno.lista"
if uploads_listar "$TAR_BUENO" "$LISTA_BUENO"; then
    anotar 1 "uploads_listar abre el tar recien hecho"
else
    anotar 0 "uploads_listar abre el tar recien hecho"
fi

# El tar no debe llevar rutas absolutas: se extrae donde haga falta.
if tar tzf "$TAR_BUENO" | grep -q '^/'; then
    anotar 0 "el tar no guarda rutas absolutas"
else
    anotar 1 "el tar no guarda rutas absolutas"
fi

echo "uploads_verificar:"

# --- Acepta lo que debe ---------------------------------------------------
ESPERADOS=3
comprobar aceptar "tar completo (3 de 3)" \
    'cp "$TAR_BUENO" "$TAR"; cp "$LISTA_BUENO" "$LISTA"'
# Un directorio de uploads recién creado y vacío es legítimo.
ESPERADOS=0
comprobar aceptar "origen vacio (0 de 0)" \
    'mkdir -p "$TMP/vacio"; tar czf "$TAR" -C "$TMP/vacio" .; tar tzf "$TAR" > "$LISTA"'

# --- Rechaza lo que debe (cada uno es una forma real de perder archivos) ---
ESPERADOS=3
comprobar rechazar "archivo inexistente" 'true'
comprobar rechazar "tar de 0 bytes" ': > "$TAR"; cp "$LISTA_BUENO" "$LISTA"'
# El caso que importa: un tar cortado a la mitad. Un gzip truncado no se puede
# listar, así que `uploads_listar` falla y la copia se descarta.
comprobar rechazar "tar truncado a la mitad (no se puede listar)" \
    'head -c $(( $(stat -c %s "$TAR_BUENO") / 2 )) "$TAR_BUENO" > "$TAR"; tar tzf "$TAR" > "$LISTA" 2>/dev/null || true'
# La copia incompleta: el tar se abre, pero le falta un archivo.
comprobar rechazar "tar con menos archivos que el origen (2 de 3)" \
    'mkdir -p "$TMP/parcial/uploads"; cp "$ORIGEN/muestras/2026/a.wav" "$TMP/parcial/uploads/"; cp "$ORIGEN/muestras/b.wav" "$TMP/parcial/uploads/"; tar czf "$TAR" -C "$TMP/parcial" uploads; tar tzf "$TAR" > "$LISTA"'
comprobar rechazar "no es un tar (bytes cualesquiera)" \
    'head -c 8192 /dev/urandom > "$TAR"; tar tzf "$TAR" > "$LISTA" 2>/dev/null || true'

# --- El umbral es configurable --------------------------------------------
TAR_MIN_BYTES=100000
comprobar rechazar "umbral subido: el tar completo ya no llega" \
    'cp "$TAR_BUENO" "$TAR"; cp "$LISTA_BUENO" "$LISTA"'
TAR_MIN_BYTES=64

# --- uploads_nombre_valido ------------------------------------------------
if uploads_nombre_valido 'uploads-20261016T000000Z-manual.tar.gz'; then
    anotar 1 "uploads_nombre_valido acepta una copia nuestra"
else
    anotar 0 "uploads_nombre_valido acepta una copia nuestra"
fi
for ajeno in 'backup.sql' 'uploads-20261016.tar.gz.partial' 'otro.tar.gz'; do
    if uploads_nombre_valido "$ajeno"; then
        anotar 0 "uploads_nombre_valido rechaza '$ajeno'"
    else
        anotar 1 "uploads_nombre_valido rechaza '$ajeno'"
    fi
done

echo "backup-uploads.sh (camino manual):"

# --- Camino feliz: copia verificada, sin .partial colgando ---------------
DEST="$TMP/destino"
mkdir -p "$DEST"
SALIDA=$(UPLOADS_DIR="$ORIGEN" UPLOADS_DUMPS_DIR="$DEST" bash "$BACKUP" prueba 2>&1)
RC=$?
COPIAS=$(find "$DEST" -maxdepth 1 -name 'uploads-*.tar.gz' | wc -l | tr -d ' ')
PARTIALS=$(find "$DEST" -maxdepth 1 -name '*.partial' | wc -l | tr -d ' ')
anotar "$([ $RC = 0 ] && echo 1 || echo 0)" "copia manual sale en verde" "rc=$RC"
anotar "$([ "$COPIAS" = 1 ] && echo 1 || echo 0)" "deja exactamente una copia" "hay $COPIAS"
anotar "$([ "$PARTIALS" = 0 ] && echo 1 || echo 0)" "no deja .partial detras" "hay $PARTIALS"
if echo "$SALIDA" | grep -q '3 archivos'; then
    anotar 1 "informa del recuento verificado"
else
    anotar 0 "informa del recuento verificado"
fi

# --- Origen inexistente: falla loud y no deja nada a medias ---------------
SALIDA=$(UPLOADS_DIR="$TMP/no-existe" UPLOADS_DUMPS_DIR="$DEST" bash "$BACKUP" 2>&1)
RC=$?
anotar "$([ $RC != 0 ] && echo 1 || echo 0)" "origen inexistente falla (rc=$RC)"
COPIAS=$(find "$DEST" -maxdepth 1 -name 'uploads-*.tar.gz' | wc -l | tr -d ' ')
anotar "$([ "$COPIAS" = 1 ] && echo 1 || echo 0)" "el fallo no toca las copias anteriores" "hay $COPIAS"

# --- Destino sin permiso de escritura: no deja copia a medias -------------
# Como root los permisos no frenan nada, así que el caso no se puede montar:
# se dice en vez de fingir que pasa.
if [ "$(id -u)" = 0 ]; then
    echo "  --   destino sin escritura: omitido (root ignora los permisos)"
else
    SOLO_LECTURA="$TMP/solo-lectura"
    mkdir -p "$SOLO_LECTURA"
    chmod 500 "$SOLO_LECTURA"
    SALIDA=$(UPLOADS_DIR="$ORIGEN" UPLOADS_DUMPS_DIR="$SOLO_LECTURA" bash "$BACKUP" 2>&1)
    RC=$?
    chmod 700 "$SOLO_LECTURA"
    RESTOS=$(find "$SOLO_LECTURA" -mindepth 1 | wc -l | tr -d ' ')
    anotar "$([ $RC != 0 ] && echo 1 || echo 0)" "destino sin escritura falla (rc=$RC)"
    anotar "$([ "$RESTOS" = 0 ] && echo 1 || echo 0)" "y no deja restos en el destino" "hay $RESTOS"
fi

# --- La rotación conserva siempre la más reciente -------------------------
ROTA="$TMP/rota"
mkdir -p "$ROTA"
for viejo in a b c; do
    cp "$TAR_BUENO" "$ROTA/uploads-20200101T000000Z-$viejo.tar.gz"
done
# Todas con fecha de hace 100 días; la retención es de 30.
touch -d '100 days ago' "$ROTA"/uploads-*.tar.gz
# La más reciente (por mtime) se marca con `touch` para que `ls -1t` la ponga
# primera: es la que NO se debe borrar aunque supere la retención.
touch "$ROTA/uploads-20200101T000000Z-c.tar.gz"
UPLOADS_DIR="$ORIGEN" UPLOADS_DUMPS_DIR="$ROTA" UPLOADS_RETENTION_DAYS=30 \
    bash "$BACKUP" rotacion >/dev/null 2>&1
VIEJAS=$(find "$ROTA" -maxdepth 1 -name 'uploads-20200101*.tar.gz' | wc -l | tr -d ' ')
anotar "$([ "$VIEJAS" = 1 ] && echo 1 || echo 0)" "la rotacion borra las vencidas y conserva la mas reciente" "quedan $VIEJAS"

echo "backup-uploads-loop.sh (camino del sidecar):"

# El loop lleva la ruta del lib montada en la imagen del sidecar. Aquí se
# reescribe esa línea para poder ejecutarlo tal cual sobre directorios
# temporales, con el mismo `/bin/sh` que usa el contenedor.
LOOP="$TMP/loop.sh"
sed "s#^\. /usr/local/bin/tar-uploads.sh#. $AQUI/tar-uploads.sh#" \
    "$AQUI/../backup-uploads-loop.sh" > "$LOOP"

LDEST="$TMP/loop-destino"
mkdir -p "$LDEST"
# Intervalo corto a propósito: es lo que destapó que el umbral de ranciedad se
# calculaba en minutos enteros y con intervalos cortos daba 0, dejando el aviso
# encendido para siempre.
SALIDA=$(UPLOADS_DIR="$ORIGEN" UPLOADS_DUMPS_DIR="$LDEST" UPLOADS_INTERVAL_SECONDS=2 \
    timeout 4 /bin/sh "$LOOP" 2>&1)
LCOPIAS=$(find "$LDEST" -maxdepth 1 -name 'uploads-*-auto.tar.gz' | wc -l | tr -d ' ')
LPARTIALS=$(find "$LDEST" -maxdepth 1 -name '*.partial' | wc -l | tr -d ' ')
anotar "$([ "$LCOPIAS" -ge 1 ] && echo 1 || echo 0)" "el loop deja al menos una copia verificada" "hay $LCOPIAS"
anotar "$([ "$LPARTIALS" = 0 ] && echo 1 || echo 0)" "el loop no deja .partial detras" "hay $LPARTIALS"
anotar "$([ -f "$LDEST/.ultima-copia-ok" ] && echo 1 || echo 0)" "el loop escribe la marca de ultima copia buena"
if echo "$SALIDA" | grep -q 'ALERTA'; then
    anotar 0 "el loop sano no da aviso de ranciedad" "$(echo "$SALIDA" | grep 'ALERTA' | head -n1)"
else
    anotar 1 "el loop sano no da aviso de ranciedad"
fi
# La copia del loop se abre y trae los mismos 3 archivos que el origen.
ULTIMA=$(ls -1t "$LDEST"/uploads-*-auto.tar.gz 2>/dev/null | head -n1)
LLISTA="$TMP/loop.lista"
if uploads_listar "$ULTIMA" "$LLISTA" && uploads_verificar "$ULTIMA" "$LLISTA" 3 2>/dev/null; then
    anotar 1 "la copia del loop pasa la misma verificacion que la manual"
else
    anotar 0 "la copia del loop pasa la misma verificacion que la manual"
fi

# Origen inexistente: el sidecar debe morir en vez de fingir que copia.
SALIDA=$(UPLOADS_DIR="$TMP/no-existe" UPLOADS_DUMPS_DIR="$LDEST" timeout 4 /bin/sh "$LOOP" 2>&1)
RC=$?
anotar "$([ $RC != 0 ] && echo 1 || echo 0)" "el loop falla loud si no existe el origen" "rc=$RC"

echo
if [ "$FALLAN" -gt 0 ]; then
    echo "${PASAN} ok, ${FALLAN} fallo(s)" >&2
    exit 1
fi
echo "${PASAN} ok, 0 fallos"
