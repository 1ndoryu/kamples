/* Arranque completo de la app: de cero a ventana usable.
 *
 * [por que este orden] PostgreSQL lanza `initdb` como subproceso y en Windows no
 * se pueden redirigir los dos flujos a la vez sin riesgo de cuelgue; por eso
 * primero se crea el cluster, luego se arranca el servidor y solo entonces se
 * crea la base y se migra. Cada paso deja su log en `%LOCALAPPDATA%\KamplesDatos\logs`
 * y, si algo falla, el mensaje dice qué pieza y dónde mirar.
 */

use crate::escritorio::{
    ajustes::{self, exigir, url_backend, Rutas},
    backend::{Backend, Secretos},
    datos, ffmpeg, migraciones, postgres, trabajo::Trabajo,
};
use std::sync::Mutex;

/// Lo que el lanzador mantiene vivo. El orden de destrucción importa: el
/// backend se apaga antes que PostgreSQL, o sus consultas fallarían al cerrar.
pub struct Servicio {
    _backend: Backend,
    _servidor: postgres::Servidor,
    /* [por que aquí dentro] El job debe vivir tanto como los hijos que
     * contiene: si se suelta antes, `KILL_ON_JOB_CLOSE` los mata en mitad del
     * arranque. */
    _trabajo: Option<Trabajo>,
}

pub struct Estado(pub Mutex<Option<Servicio>>);

/// Arranca todo y devuelve la URL que debe cargar la ventana.
pub fn arrancar(r: &Rutas) -> Result<(String, Servicio), String> {
    r.preparar_datos()?;
    exigir(&r.pg(), "PostgreSQL portable (carpeta pgsql)")?;
    ajustes::exigir_puerto_libre()?;

    let logs = r.logs();
    let secretos = Secretos::cargar(r)?;
    let herramientas = ffmpeg::Herramientas::buscar(r)?;

    /* Se crea antes que el primer hijo: a partir de aquí, todo lo que lance este
     * proceso muere con él aunque sea un cierre brusco. */
    /* Puede fallar (políticas de grupo que prohíben jobs): en ese caso se sigue
     * sin él y el cierre ordenado cubre la salida normal. */
    let trabajo = Trabajo::nuevo();
    let servidor =
        postgres::Servidor::arrancar(r, &logs.join("postgres.log"), trabajo.as_ref())?;
    postgres::asegurar_base(r, servidor.puerto)?;

    let aplicadas = migraciones::aplicar(r, servidor.puerto)?;
    if aplicadas > 0 {
        guardar(&logs.join("migraciones.log"), &format!("aplicadas: {aplicadas}\n"));
    }

    /* La copia va DESPUÉS de migrar, no antes: así la copia diaria refleja el
     * esquema que la app va a usar. La copia previa a una migración arriesgada
     * la hace el propio lanzador solo si va a aplicar alguna. */
    if let Err(e) = datos::copia_diaria(r, servidor.puerto) {
        guardar(&logs.join("backups.log"), &format!("{e}\n"));
    }

    let backend = Backend::arrancar(
        r,
        servidor.puerto,
        ajustes::PUERTO_PREFERIDO,
        &secretos,
        &herramientas,
        &logs.join("backend.log"),
        trabajo.as_ref(),
    )?;
    debug_assert_eq!(backend.url, url_backend());

    let url = backend.url.clone();
    Ok((
        url,
        Servicio {
            _backend: backend,
            _servidor: servidor,
            _trabajo: trabajo,
        },
    ))
}

fn guardar(ruta: &std::path::Path, texto: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(ruta)
    {
        let _ = f.write_all(texto.as_bytes());
    }
}
