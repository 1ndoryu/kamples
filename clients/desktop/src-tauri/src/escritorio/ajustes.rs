/* Rutas y ajustes del modo escritorio.
 *
 * [por que] Una sola función decide dónde está cada pieza. Si esto se reparte
 * por los módulos, un día el backend escribe en la carpeta del programa y una
 * actualización se lleva los datos de la usuaria.
 */

use std::path::{Path, PathBuf};

/// Carpeta bajo %LOCALAPPDATA% donde viven los datos que deben perdurar.
///
/// [por que no `Kamples` a secas] Ese nombre es el que usa el instalador de
/// Windows para el programa: un desinstalador borra la carpeta entera y se
/// llevaría la base y los samples. Un nombre propio no lo toca nadie más.
pub const CARPETA_DATOS: &str = "KamplesDatos";

/// Número de puerto para el backend. Se busca uno libre a partir de aquí, así
/// que dos copias de la app abiertas a la vez no se pisan.
pub const PUERTO_PREFERIDO: u16 = 4317;

#[derive(Clone, Debug)]
pub struct Rutas {
    /// Carpeta del .exe: de aquí salen los binarios (se relee en cada arranque
    /// para que una actualización en caliente no use restos viejos).
    pub programa: PathBuf,
    /// %LOCALAPPDATA%\KamplesDatos: datos persistentes de la usuaria.
    pub datos: PathBuf,
}

impl Rutas {
    pub fn detectar() -> Result<Self, String> {
        let programa = std::env::current_exe()
            .map_err(|e| format!("no se pudo localizar el .exe: {e}"))?
            .parent()
            .ok_or("el .exe no tiene carpeta contenedora")?
            .to_path_buf();

        /* [por que] LOCALAPPDATA es la carpeta correcta en Windows para datos
         * de aplicación, pero no existe si la app se lanza en un contexto raro
         * (servicio, tarea sin perfil). Se falla con un mensaje claro en vez de
         * escribir en la carpeta del programa. */
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .ok_or("LOCALAPPDATA no está definido; no hay dónde guardar los datos")?;
        let datos = base.join(CARPETA_DATOS);

        Ok(Self { programa, datos })
    }

    /// Raíz del PostgreSQL portable que viaja con la app.
    pub fn pg(&self) -> PathBuf {
        self.programa.join("pgsql")
    }

    /// Cluster de la base de datos (el `PGDATA`).
    pub fn pgdata(&self) -> PathBuf {
        self.datos.join("pgdata")
    }

    /// Carpeta de los samples subidos y sus derivados.
    pub fn uploads(&self) -> PathBuf {
        self.datos.join("uploads")
    }

    /// Copias de seguridad de la base, una por día.
    pub fn backups(&self) -> PathBuf {
        self.datos.join("backups")
    }

    pub fn logs(&self) -> PathBuf {
        self.datos.join("logs")
    }

    /// Front compilado que el backend sirve como estáticos.
    pub fn front(&self) -> PathBuf {
        self.programa.join("front")
    }

    /// Migraciones `.up.sql` que se aplican la primera vez.
    pub fn migraciones(&self) -> PathBuf {
        self.programa.join("migraciones")
    }

    /// Binario del backend, junto al .exe del lanzador.
    pub fn backend_exe(&self) -> PathBuf {
        self.programa.join("glory-backend.exe")
    }

    /// ffmpeg/ffprobe que viajan con la app (pipeline de MP3 y waveform).
    pub fn ffmpeg_dir(&self) -> PathBuf {
        self.programa.join("bin")
    }

    /// Crea las carpetas de datos que falten. Idempotente y barato.
    pub fn preparar_datos(&self) -> Result<(), String> {
        for dir in [
            self.datos.clone(),
            self.uploads(),
            self.backups(),
            self.logs(),
        ] {
            std::fs::create_dir_all(&dir)
                .map_err(|e| format!("no se pudo crear {}: {e}", dir.display()))?;
        }
        Ok(())
    }
}

/// `Command` sin ventana de consola.
///
/// [por que] Cada `.exe` de Windows abre una consola al lanzarse. La app arranca
/// siete procesos (postgres, initdb, psql por migración, pg_dump, backend): sin
/// esto aparecen ocho ventanas negras encima del usuario cada vez que abre
/// Kamples. `CREATE_NO_WINDOW` las elimina sin quitar la salida redirigida.
pub fn comando(programa: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    #[allow(unused_mut)]
    let mut cmd = std::process::Command::new(programa);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Comprueba que una pieza del paquete está donde debe, con un mensaje que
/// diga qué falta y dónde se esperaba.
pub fn exigir(ruta: &Path, que: &str) -> Result<(), String> {
    if ruta.exists() {
        Ok(())
    } else {
        Err(format!("falta {que}: se esperaba en {}", ruta.display()))
    }
}

/// Puerto libre a partir de `desde`, comprobando de verdad que se puede abrir.
pub fn puerto_libre(desde: u16) -> u16 {
    for puerto in desde..desde.saturating_add(200) {
        if std::net::TcpListener::bind(("127.0.0.1", puerto)).is_ok() {
            return puerto;
        }
    }
    desde
}

/// URL del backend. Puerto FIJO a propósito.
///
/// [por que] Las ventanas cargan el front desde el backend y Tauri solo permite
/// el IPC (los `invoke` de sincronización y OAuth) desde orígenes declarados en
/// la capability. Con un puerto variable habría que abrir el IPC a cualquier
/// puerto local; con uno fijo, la lista blanca es una sola URL exacta.
pub fn url_backend() -> String {
    format!("http://127.0.0.1:{PUERTO_PREFERIDO}")
}

/// Si ya hay una Kamples viva en el puerto de siempre, su URL.
///
/// [por que] Abrir la app dos veces no puede arrancar un segundo PostgreSQL
/// sobre el mismo cluster: lo corrompe. La segunda ventana se engancha a la
/// primera si contesta `/api/health`.
pub fn instancia_viva() -> Option<String> {
    let url = url_backend();
    respuesta_ok(&format!("{url}/api/health")).then_some(url)
}

/// GET mínimo sin dependencias HTTP: solo interesa si el puerto contesta 200.
fn respuesta_ok(url: &str) -> bool {
    use std::io::{Read, Write};
    let Some(sin_esquema) = url.strip_prefix("http://") else {
        return false;
    };
    let Some((host, ruta)) = sin_esquema.split_once('/') else {
        return false;
    };
    let Ok(mut flujo) = std::net::TcpStream::connect(host) else {
        return false;
    };
    if flujo
        .set_read_timeout(Some(std::time::Duration::from_secs(2)))
        .is_err()
    {
        return false;
    }
    if write!(
        flujo,
        "GET /{ruta} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"
    )
    .is_err()
    {
        return false;
    }
    let mut respuesta = String::new();
    flujo.read_to_string(&mut respuesta).is_ok()
        && (respuesta.starts_with("HTTP/1.1 200") || respuesta.starts_with("HTTP/1.0 200"))
}

/// Comprueba que el puerto fijo esté libre antes de arrancar PostgreSQL.
pub fn exigir_puerto_libre() -> Result<(), String> {
    match std::net::TcpListener::bind(("127.0.0.1", PUERTO_PREFERIDO)) {
        Ok(_) => Ok(()),
        Err(_) => Err(format!(
            "el puerto {PUERTO_PREFERIDO} está ocupado por otro programa; \
             cierra lo que lo use y vuelve a abrir Kamples"
        )),
    }
}
