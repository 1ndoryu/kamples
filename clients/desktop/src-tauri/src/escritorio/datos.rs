/* Copias de seguridad de la base: una al día, y solo si hay algo que copiar.
 *
 * [por que] La app guarda los samples de la usuaria en un solo cluster local.
 * Un `pg_dump` diario es la única red si el disco falla o una migración sale
 * mal. Se guarda en `%LOCALAPPDATA%\KamplesDatos\backups`, fuera del programa, y se
 * conservan las últimas N para no llenar el disco.
 */

use crate::escritorio::ajustes::Rutas;
use crate::escritorio::postgres;
use std::path::PathBuf;
use std::process::Stdio;

/// Copias que se conservan. 14 días cubren un error que se descubre tarde.
const CONSERVAR: usize = 14;

/// Hace la copia de hoy si aún no existe. Devuelve la ruta creada, si la hubo.
pub fn copia_diaria(r: &Rutas, puerto: u16) -> Result<Option<PathBuf>, String> {
    let destino = r.backups().join(format!("kamples-{}.dump", hoy()));
    if destino.exists() {
        return Ok(None);
    }

    let archivo = std::fs::File::create(&destino)
        .map_err(|e| format!("no se pudo crear {}: {e}", destino.display()))?;
    let salida = super::ajustes::comando(r.pg().join("bin").join("pg_dump.exe"))
        .env("PGPASSWORD", clave())
        .arg("-h")
        .arg("127.0.0.1")
        .arg("-p")
        .arg(puerto.to_string())
        .arg("-U")
        .arg(postgres::ROL)
        .arg("-Fc") // formato comprimido: restaurable con pg_restore
        .arg("-d")
        .arg(postgres::BASE)
        .stdout(Stdio::from(archivo))
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("no se pudo ejecutar pg_dump: {e}"))?;

    if !salida.status.success() {
        /* [por que] Una copia a medias es peor que ninguna: se borra para que
         * mañana se reintente en vez de dar por buena una copia truncada. */
        let _ = std::fs::remove_file(&destino);
        return Err(format!(
            "pg_dump falló: {}",
            String::from_utf8_lossy(&salida.stderr).trim()
        ));
    }
    rotar(r);
    Ok(Some(destino))
}

/// Borra las copias más viejas dejando las últimas `CONSERVAR`.
fn rotar(r: &Rutas) {
    let Ok(entradas) = std::fs::read_dir(r.backups()) else {
        return;
    };
    let mut copias: Vec<PathBuf> = entradas
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "dump"))
        .collect();
    copias.sort(); // el nombre lleva la fecha, así que ordenar es cronológico
    let sobran = copias.len().saturating_sub(CONSERVAR);
    for vieja in copias.into_iter().take(sobran) {
        let _ = std::fs::remove_file(vieja);
    }
}

fn hoy() -> String {
    /* [por que] Sin dependencias de fecha en el lanzador: `date` de Windows
     * respeta la configuración regional, así que no sirve para nombrar
     * archivos. Se usa el reloj del sistema convertido a año-mes-día UTC. */
    let segundos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (anio, mes, dia) = fecha_utc(segundos);
    format!("{anio:04}-{mes:02}-{dia:02}")
}

/// Fecha civil a partir de segundos UNIX (algoritmo de Howard Hinnant).
fn fecha_utc(segundos: u64) -> (i64, u32, u32) {
    let dias = (segundos / 86_400) as i64;
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn clave() -> &'static str {
    postgres::CLAVE
}

#[cfg(test)]
mod tests {
    use super::fecha_utc;

    #[test]
    fn fecha_conocida() {
        // 2026-11-15T00:00:00Z
        assert_eq!(fecha_utc(1_763_164_800), (2026, 11, 15));
    }
}
