#!/usr/bin/env bash
# [150] Aplicar migraciones por la vía segura. Sustituye al `psql` a mano que
# documentaba el roadmap:100, que no paraba ante un error ni dejaba copia.
#
# Uso:
#   ./scripts/db/migrar.sh migrations/20260703000000_fix_colecciones_schema.up.sql
#
# El ejecutor dentro del contenedor lo elige `PG_EXEC` (ver
# scripts/lib/dump-postgres.sh). En producción, donde AGENTS §10 prohíbe el SSH
# directo, es obligatorio el de coolify-manager-rs:
#   PG_EXEC='cm exec --name kamples --target postgres --' ./scripts/db/migrar.sh <archivo.up.sql>
#
# Lo que hace, en este orden, y no admite saltarse ningún paso:
#   1. Guardia: se niega si el archivo lleva SQL destructivo no declarado.
#   2. Copia de seguridad VERIFICADA de la base. Si la copia no sale bien, no
#      se aplica nada.
#   3. Aplica con `ON_ERROR_STOP=1` y `--single-transaction`: un error a mitad
#      de archivo aborta y deshace todo, en vez de dejar el esquema a medias
#      (que es como el `psql` sin flags se comportaba: seguía como si nada).
#
# No existe un flag para saltarse la copia. Si hace falta, es señal de que la
# operación no debería hacerse ahora.

set -euo pipefail

AQUI=$(cd "$(dirname "$0")" && pwd)
RAIZ=$(cd "$AQUI/../.." && pwd)
# shellcheck source=../lib/migraciones.sh
. "$AQUI/../lib/migraciones.sh"
# shellcheck source=../lib/dump-postgres.sh
. "$AQUI/../lib/dump-postgres.sh"

if [ "$#" -eq 0 ]; then
    echo "uso: $0 <archivo.up.sql> [mas archivos]" >&2
    exit 2
fi

if [ -f ".env" ]; then
    # shellcheck disable=SC1091
    set -a; . ./.env; set +a
fi
: "${POSTGRES_USER:?POSTGRES_USER no definido}"
: "${POSTGRES_DB:?POSTGRES_DB no definido}"

# --- 1. Guardia ------------------------------------------------------------

echo "[migrar] paso 1/3: guardia de SQL destructivo"
if ! "$AQUI/guardia-migraciones.sh" "$@"; then
    echo "[migrar] ABORTADO en la guardia: no se ha tocado la base." >&2
    exit 1
fi

# --- 2. Copia de seguridad verificada -------------------------------------

echo "[migrar] paso 2/3: copia de seguridad de ${POSTGRES_DB}"
"$AQUI/backup-postgres.sh" pre-migracion

# --- 3. Aplicar ------------------------------------------------------------

echo "[migrar] paso 3/3: aplicando ${#} archivo(s)"
for f in "$@"; do
    [ -f "$f" ] || { echo "[migrar] no existe: $f" >&2; exit 1; }
    echo "[migrar] aplicando $(basename "$f")"
    # `-f -` lee el script por stdin: las migraciones viven en el repo y no
    # están montadas dentro del contenedor de postgres.
    pg_en_contenedor \
        psql -v ON_ERROR_STOP=1 --single-transaction --quiet \
            -U "$POSTGRES_USER" -d "$POSTGRES_DB" -f - < "$f"
done

echo "[migrar] OK: ${#} migracion(es) aplicada(s) sobre ${POSTGRES_DB}"
echo "[migrar] si algo va mal, la copia previa es la mas reciente en /data/kamples/db_dumps"
echo "[migrar] restaurar: ./scripts/db/restore-postgres.sh <archivo.dump>"
