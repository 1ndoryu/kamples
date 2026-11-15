/* Migraciones: se aplican una vez, en orden y sin dejar el esquema a medias.
 *
 * [por que] El backend NO las aplica (`sqlx::migrate!` está comentado en
 * `main.rs`), y en producción las lanza `scripts/db/migrar.sh`. Aquí no hay bash
 * ni contenedor: el lanzador hace lo mismo — una transacción por archivo,
 * registro en una tabla propia y nada de "seguir como si nada" ante un error.
 */

use crate::escritorio::ajustes::Rutas;
use crate::escritorio::postgres;
use std::path::Path;

/// Tabla de control. No usa `_sqlx_migrations` porque estas migraciones no las
/// aplica SQLx: mezclar ambos registros haría que un cambio de herramienta
/// reaplicara archivos ya ejecutados.
const CONTROL: &str = "_kamples_migraciones";

/// Aplica las pendientes. Devuelve cuántas aplicó.
pub fn aplicar(r: &Rutas, puerto: u16) -> Result<usize, String> {
    postgres::psql(
        r,
        puerto,
        &format!(
            "CREATE TABLE IF NOT EXISTS {CONTROL} (
                nombre text PRIMARY KEY,
                aplicada timestamptz NOT NULL DEFAULT now()
            );"
        ),
    )?;

    let aplicadas = postgres::psql(r, puerto, &format!("SELECT nombre FROM {CONTROL};"))?;
    let ya: Vec<&str> = aplicadas.lines().map(str::trim).collect();

    let mut archivos = archivos_ordenados(&r.migraciones())?;
    archivos.retain(|(nombre, _)| !ya.contains(&nombre.as_str()));

    for (nombre, ruta) in &archivos {
        let sql = std::fs::read_to_string(ruta)
            .map_err(|e| format!("no se pudo leer {}: {e}", ruta.display()))?;
        /* [por que] La mitad de las migraciones ya trae su `BEGIN`/`COMMIT`; si
         * se envuelven otra vez, PostgreSQL avisa de transacción anidada y el
         * registro queda fuera de la transacción real. Se envuelve solo lo que
         * no la trae, y el registro va siempre dentro del mismo bloque. */
        let ya_tiene_tx = abre_transaccion(&sql);
        let mut guion = String::new();
        if !ya_tiene_tx {
            guion.push_str("BEGIN;\n");
        }
        guion.push_str(&sql);
        if !sql.trim_end().ends_with(';') {
            guion.push('\n');
        }
        guion.push_str(&format!(
            "\nINSERT INTO {CONTROL} (nombre) VALUES ('{}');\n",
            nombre.replace('\'', "''")
        ));
        if !ya_tiene_tx {
            guion.push_str("COMMIT;\n");
        }
        postgres::psql(r, puerto, &guion).map_err(|e| format!("migración {nombre} falló: {e}"))?;
    }

    Ok(archivos.len())
}

/// ¿El archivo abre y cierra su propia transacción?
///
/// Se mira cada línea sin comentarios: `-- BEGIN` en prosa no cuenta, y un
/// `BEGIN` dentro de un bloque `DO $$ … $$` sí aparece como `BEGIN` suelto pero
/// también lo hace el `END $$`, así que se exige `COMMIT` para darlo por hecho.
fn abre_transaccion(sql: &str) -> bool {
    let sentencias: Vec<String> = sql
        .lines()
        .map(|l| l.split("--").next().unwrap_or("").trim().to_ascii_uppercase())
        .filter(|l| !l.is_empty())
        .collect();
    let inicio = sentencias.iter().position(|l| l == "BEGIN" || l == "BEGIN;");
    let fin = sentencias.iter().any(|l| l == "COMMIT" || l == "COMMIT;");
    inicio.is_some() && fin
}

/// Los `.up.sql` en orden de nombre: el prefijo de fecha los ordena solo.
fn archivos_ordenados(dir: &Path) -> Result<Vec<(String, std::path::PathBuf)>, String> {
    let entradas = std::fs::read_dir(dir)
        .map_err(|e| format!("no se pudo leer {}: {e}", dir.display()))?;
    let mut lista: Vec<(String, std::path::PathBuf)> = entradas
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|x| x == "sql")
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.ends_with(".up.sql"))
        })
        .filter_map(|p| {
            let nombre = p.file_name()?.to_str()?.to_string();
            Some((nombre, p))
        })
        .collect();
    lista.sort_by(|a, b| a.0.cmp(&b.0));
    if lista.is_empty() {
        return Err(format!("no hay migraciones en {}", dir.display()));
    }
    Ok(lista)
}
