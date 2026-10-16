#!/bin/sh
# [150] Detección de SQL destructivo en migraciones. Se carga con `. <ruta>`.
#
# [por que] Las migraciones se aplican A MANO con `psql` (roadmap:100) y no hay
# ninguna barrera que impida soltar un `DROP TABLE` contra producción. El
# incidente que esta viñeta debe evitar fue precisamente una operación de ese
# tipo. Esto no impide aplicar SQL destructivo: obliga a que sea una decisión
# escrita y con motivo, en vez de una línea que se cuela en un archivo.
#
# Limitación conocida: la sentencia se corta por `;`, así que un `;` dentro de
# un literal de texto partiría una sentencia en dos. No ocurre en las
# migraciones de este repo, y el detector prefiere sobrerreportar a callarse.

# migracion_es_destructiva <archivo>
#   Imprime a stdout los hallazgos, uno por línea: `<linea>:<motivo>`.
#   Devuelve 0 si hay al menos uno, 1 si no hay ninguno.
migracion_es_destructiva() {
    _me_archivo=$1
    [ -f "$_me_archivo" ] || return 1

    _me_norm=$(mktemp)
    _me_hall=$(mktemp)

    # Normaliza a una sentencia por línea, con el número de línea donde
    # empieza, y sin comentarios `--`.
    awk '
        {
            linea = $0
            sub(/--.*$/, "", linea)
            if (pendiente == "") inicio = NR
            pendiente = pendiente " " linea
            if (linea ~ /;/) {
                printf "%d:%s\n", inicio, pendiente
                pendiente = ""
            }
        }
        END { if (pendiente != "") printf "%d:%s\n", inicio, pendiente }
    ' "$_me_archivo" > "$_me_norm"

    awk '
        {
            sep = index($0, ":")
            n = substr($0, 1, sep - 1)
            s = tolower(substr($0, sep + 1))
            motivo = ""
            if (s ~ /drop[ \t]+(table|schema|database|column|materialized[ \t]+view)/)
                motivo = "DROP de objeto que contiene datos"
            else if (s ~ /truncate/)
                motivo = "TRUNCATE"
            else if (s ~ /alter[ \t]+column[^;]*[ \t]type/)
                motivo = "cambio de tipo de columna (puede truncar)"
            else if (s ~ /delete[ \t]+from/ && s !~ /delete[ \t]+from[^;]*where/)
                motivo = "DELETE FROM sin WHERE (borra la tabla entera)"
            if (motivo != "") printf "%s:%s\n", n, motivo
        }
    ' "$_me_norm" > "$_me_hall"

    if [ -s "$_me_hall" ]; then
        cat "$_me_hall"
        rm -f "$_me_norm" "$_me_hall"
        return 0
    fi

    rm -f "$_me_norm" "$_me_hall"
    return 1
}
