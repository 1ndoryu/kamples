/* PostgreSQL portable: ciclo de vida del servidor que viaja con la app.
 *
 * [por que] `initdb`/`pg_ctl` de Windows no sirven para un producto: `pg_ctl
 * start` devuelve en cuanto arranca, deja el proceso desacoplado y su salida en
 * un log propio, y `pg_ctl stop` puede tardar sin avisar. Aquí se lanza
 * `postgres.exe` como hijo directo para poder esperarlo, matarlo sin dejar
 * huérfanos y leer su salida por la tubería.
 *
 * Dos cosas aprendidas del PostgreSQL real:
 *   - El cluster NO se puede crear en una ruta con espacios si se pasa como
 *     argumento sin comillas en Windows: se usa SIEMPRE `.arg()`, nunca una
 *     cadena de shell.
 *   - Si `postgres.exe` muere a medias (apagón, cuelgue), deja `postmaster.pid`
 *     y el siguiente arranque se niega. Se comprueba el PID antes de borrarlo.
 */

use crate::escritorio::ajustes::Rutas;
use std::path::Path;
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

/// Nombre del rol y de la base. Fijos: la app es de un solo usuario en su PC.
///
/// [por que siguen al slug y no al nombre] El rol y la base viven dentro de un
/// `pgdata` ya inicializado; renombrarlos por un cambio de marca dejaría la app
/// sin su propia base. Cambiarlos exige recrear el cluster y migrar los datos.
pub const ROL: &str = crate::marca::IDENTIDAD_SLUG;
pub const BASE: &str = crate::marca::IDENTIDAD_SLUG;
/// Clave del rol. Fija: el servidor escucha SOLO en 127.0.0.1, así que no
/// protege de nadie; evita el caso de `trust`, que sí quedaría expuesto si
/// alguna vez se cambia `listen_addresses`.
pub const CLAVE: &str = "kamples_local";

pub struct Servidor {
    hijo: Child,
    pub puerto: u16,
    /// Para el apagado ordenado (`pg_ctl stop -m fast`): sin esto el `Drop` no
    /// podría hacer checkpoint y habría que matar el proceso.
    pg_bin: std::path::PathBuf,
    pgdata: std::path::PathBuf,
}

impl Servidor {
    /// Arranca (o crea y arranca) el servidor y espera a que acepte conexiones.
    pub fn arrancar(
        r: &Rutas,
        log: &Path,
        trabajo: Option<&super::trabajo::Trabajo>,
    ) -> Result<Self, String> {
        if !r.pgdata().join("PG_VERSION").exists() {
            crear_cluster(r, log)?;
        }
        limpiar_pid_huerfano(r);

        let puerto = super::ajustes::puerto_libre(5433);
        let salida = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)
            .map_err(|e| format!("no se pudo abrir el log de PostgreSQL: {e}"))?;
        let errores = salida
            .try_clone()
            .map_err(|e| format!("no se pudo duplicar el log: {e}"))?;

        /* [por que] `-p` y no `port=` en el .conf: si el 5433 está ocupado por
         * otra cosa, se usa otro sin tocar la configuración del cluster. */
        let hijo = super::ajustes::comando(r.pg().join("bin").join("postgres.exe"))
            .arg("-D")
            .arg(r.pgdata())
            .arg("-p")
            .arg(puerto.to_string())
            .arg("-c")
            .arg("listen_addresses=127.0.0.1")
            .arg("-c")
            .arg("fsync=on")
            .stdout(Stdio::from(salida))
            .stderr(Stdio::from(errores))
            .spawn()
            .map_err(|e| format!("no se pudo lanzar PostgreSQL: {e}"))?;

        let mut servidor = Self {
            hijo,
            puerto,
            pg_bin: r.pg().join("bin"),
            pgdata: r.pgdata(),
        };
        /* [por que aquí y no al salir] El job se crea en el lanzador y vive
         * mientras viva la app; asignando a cada hijo en cuanto nace, ni una
         * caída brusca deja un postgres escribiendo en el cluster. */
        if let Some(trabajo) = trabajo {
            trabajo.asignar(&servidor.hijo);
        }
        servidor.esperar(Duration::from_secs(120))?;
        Ok(servidor)
    }

    /// Espera a que PostgreSQL **conteste una consulta**, no a que abra el puerto.
    ///
    /// [por que] Tras un cierre brusco, el postmaster abre el puerto en cuanto
    /// arranca y sigue en recuperación por WAL: durante ese rato acepta la
    /// conexión y contesta `FATAL: the database system is starting up`. Con un
    /// simple `TcpStream::connect` el lanzador daba el servidor por bueno, creaba
    /// la base y migraba contra uno que aún no servía, y la app abría la pantalla
    /// de error. `pg_isready` pregunta de verdad y distingue "aún arrancando" de
    /// "listo".
    fn esperar(&mut self, tope: Duration) -> Result<(), String> {
        let inicio = Instant::now();
        while inicio.elapsed() < tope {
            if let Some(estado) = self.hijo.try_wait().ok().flatten() {
                return Err(format!(
                    "PostgreSQL terminó al arrancar ({estado}); mira el log de datos/logs"
                ));
            }
            if pg_isready(&self.pg_bin, self.puerto) {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        Err("PostgreSQL no contestó consultas en 120 s (¿recuperación atascada?): mira el log de datos/logs".into())
    }

}

impl Drop for Servidor {
    /// Apagado ordenado al cerrar la app: `pg_ctl stop -m fast` cierra las
    /// conexiones y hace el checkpoint final, así el siguiente arranque no tiene
    /// que recuperar por WAL. `taskkill` sin `/F` no sirve: Windows no manda
    /// Ctrl-C a un proceso sin consola, y el apagado acababa en un `kill()` tras
    /// 15 s de espera. Si `pg_ctl` no responde en 20 s, se termina.
    fn drop(&mut self) {
        let ordenado = super::ajustes::comando(self.pg_bin.join("pg_ctl.exe"))
            .arg("stop")
            .arg("-D")
            .arg(&self.pgdata)
            .arg("-m")
            .arg("fast")
            .arg("-w")
            .arg("-t")
            .arg("20")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        /* Se espera al hijo incluso cuando `pg_ctl` dijo que fue bien: el
         * proceso es nuestro hijo directo y hay que recogerlo para no dejar
         * zombis en el administrador de tareas. */
        let _ = self.hijo.wait();
        if !ordenado {
            let _ = self.hijo.kill();
            let _ = self.hijo.wait();
        }
    }
}

fn crear_cluster(r: &Rutas, log: &Path) -> Result<(), String> {
    let pgdata = r.pgdata();
    std::fs::create_dir_all(&pgdata)
        .map_err(|e| format!("no se pudo crear {}: {e}", pgdata.display()))?;

    /* [por que] La primera vez se crea el rol y la base en el mismo paso que el
     * cluster. Si se omite el rol y se conecta como `postgres`, la app
     * funcionaría en un PC y fallaría en otro según lo que hubiera instalado. */
    let clave = r.datos.join("pgpass.txt");
    std::fs::write(&clave, format!("{CLAVE}\n"))
        .map_err(|e| format!("no se pudo escribir la clave temporal: {e}"))?;

    /* [por que -A scram y no trust] `trust` dejaría entrar a cualquiera que
     * llegara al puerto. Aunque escucha solo en 127.0.0.1, un `scram` con clave
     * fija no cuesta nada y evita depender de que ese ajuste no cambie nunca.
     * [por que --locale=C] initdb con la configuración regional de Windows
     * aborta cuando la carpeta del usuario tiene caracteres que el código no
     * reconoce; `C` es neutro y las collations del SQL no las usa el esquema. */
    let salida = super::ajustes::comando(r.pg().join("bin").join("initdb.exe"))
        .arg("-D")
        .arg(&pgdata)
        .arg("-U")
        .arg(ROL)
        .arg("--pwfile")
        .arg(&clave)
        .arg("-A")
        .arg("scram-sha-256")
        .arg("--encoding=UTF8")
        .arg("--locale=C")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("no se pudo ejecutar initdb: {e}"))?;
    let _ = std::fs::remove_file(&clave);

    let detalle = format!(
        "{}{}",
        String::from_utf8_lossy(&salida.stdout),
        String::from_utf8_lossy(&salida.stderr)
    );
    std::fs::write(log, &detalle).ok();
    if !salida.status.success() {
        return Err(format!(
            "initdb falló; el detalle completo está en {}",
            log.display()
        ));
    }
    Ok(())
}

/// `postmaster.pid` solo se borra si su PID ya no existe: si el servidor sigue
/// vivo, arrancar otro sobre el mismo cluster corrompe los datos.
fn limpiar_pid_huerfano(r: &Rutas) {
    let ruta = r.pgdata().join("postmaster.pid");
    let Ok(texto) = std::fs::read_to_string(&ruta) else {
        return;
    };
    let pid = texto.lines().next().unwrap_or("").trim().to_string();
    if pid.is_empty() || !proceso_vivo(&pid) {
        let _ = std::fs::remove_file(&ruta);
    }
}

#[cfg(windows)]
fn proceso_vivo(pid: &str) -> bool {
    // `tasklist` trae el PID con /FO CSV; si no aparece, no existe.
    let Ok(salida) = super::ajustes::comando("tasklist")
        .arg("/FI")
        .arg(format!("PID eq {pid}"))
        .arg("/NH")
        .arg("/FO")
        .arg("CSV")
        .output()
    else {
        return true; // ante la duda, no se toca el pid
    };
    String::from_utf8_lossy(&salida.stdout).contains(&format!("\"{pid}\""))
}

#[cfg(not(windows))]
fn proceso_vivo(_pid: &str) -> bool {
    true
}

/// Ejecuta `psql` contra la base y devuelve si terminó bien. Se usa para las
/// migraciones y para `CREATE EXTENSION`.
pub fn psql(r: &Rutas, puerto: u16, sql: &str) -> Result<String, String> {
    let mut cmd = super::ajustes::comando(r.pg().join("bin").join("psql.exe"));
    cmd.env("PGPASSWORD", CLAVE)
        .arg("-h")
        .arg("127.0.0.1")
        .arg("-p")
        .arg(puerto.to_string())
        .arg("-U")
        .arg(ROL)
        .arg("-d")
        .arg(BASE)
        .arg("-v")
        .arg("ON_ERROR_STOP=1")
        .arg("-At")
        .arg("-f")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut hijo = cmd
        .spawn()
        .map_err(|e| format!("no se pudo ejecutar psql: {e}"))?;
    {
        use std::io::Write;
        let mut entrada = hijo.stdin.take().ok_or("psql sin stdin")?;
        entrada
            .write_all(sql.as_bytes())
            .map_err(|e| format!("no se pudo escribir a psql: {e}"))?;
    }
    let salida = hijo
        .wait_with_output()
        .map_err(|e| format!("psql no terminó: {e}"))?;
    if salida.status.success() {
        Ok(String::from_utf8_lossy(&salida.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&salida.stderr).trim().to_string())
    }
}

/// ¿Acepta consultas? `pg_isready` sale 0 solo cuando el servidor está listo;
/// el puerto abierto no basta (durante la recuperación también lo está).
fn pg_isready(pg_bin: &Path, puerto: u16) -> bool {
    super::ajustes::comando(pg_bin.join("pg_isready.exe"))
        .arg("-h")
        .arg("127.0.0.1")
        .arg("-p")
        .arg(puerto.to_string())
        .arg("-q")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Crea la base de la app si aún no existe (idempotente).
pub fn asegurar_base(r: &Rutas, puerto: u16) -> Result<(), String> {
    let mut cmd = super::ajustes::comando(r.pg().join("bin").join("createdb.exe"));
    cmd.env("PGPASSWORD", CLAVE)
        .arg("-h")
        .arg("127.0.0.1")
        .arg("-p")
        .arg(puerto.to_string())
        .arg("-U")
        .arg(ROL);
    // `createdb` falla si ya existe; se comprueba antes para no tratarlo como error.
    let existe = psql(r, puerto, &format!("SELECT 1 FROM pg_database WHERE datname='{BASE}';"))
        .map(|s| s.trim() == "1")
        .unwrap_or(false);
    if existe {
        return Ok(());
    }
    let salida = cmd
        .arg(BASE)
        .output()
        .map_err(|e| format!("no se pudo ejecutar createdb: {e}"))?;
    if salida.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&salida.stderr).trim().to_string())
    }
}
