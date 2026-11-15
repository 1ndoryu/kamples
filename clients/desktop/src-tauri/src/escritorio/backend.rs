/* El backend Axum que sirve la app: se lanza, se espera y se apaga con la ventana.
 *
 * [por que] Las ventanas cargan el front DESDE el backend (mismo origen), igual
 * que en producción. Así `/api/...` y `/uploads/...` resuelven sin proxies ni
 * cambios en el front, y no hay CORS entre la ventana y el servidor.
 */

use crate::escritorio::ajustes::{exigir, Rutas};
use crate::escritorio::ffmpeg::Herramientas;
use crate::escritorio::postgres;
use std::io::Write;
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

pub struct Backend {
    hijo: Child,
    pub url: String,
}

impl Backend {
    pub fn arrancar(
        r: &Rutas,
        puerto_pg: u16,
        puerto: u16,
        secretos: &Secretos,
        ff: &Herramientas,
        log: &Path,
        trabajo: Option<&super::trabajo::Trabajo>,
    ) -> Result<Self, String> {
        exigir(&r.backend_exe(), "glory-backend.exe")?;
        exigir(&r.front().join("index.html"), "el front compilado (front/index.html)")?;

        let salida = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)
            .map_err(|e| format!("no se pudo abrir el log del backend: {e}"))?;
        let errores = salida
            .try_clone()
            .map_err(|e| format!("no se pudo duplicar el log: {e}"))?;

        /* El binario es estático: no necesita PATH ni DLLs del sistema, pero sí
         * que el directorio de trabajo sea la carpeta del programa, porque los
         * `ServeDir` del front y de las migraciones son relativos. */
        let hijo = super::ajustes::comando(r.backend_exe())
            .current_dir(&r.programa)
            .env("DATABASE_URL", url_base(puerto_pg))
            .env("JWT_SECRET", &secretos.jwt)
            .env("WS_SECRET", &secretos.ws)
            .env("HOST", "127.0.0.1")
            .env("PORT", puerto.to_string())
            .env("STORAGE_ROOT", r.uploads())
            .env("STATIC_DIR", r.front())
            .env("PUBLIC_BASE_URL", format!("http://127.0.0.1:{puerto}"))
            .env("FFMPEG_PATH", &ff.ffmpeg)
            .env("FFPROBE_PATH", &ff.ffprobe)
            /* [por que] Redis, SMTP, Stripe, FCM y VAPID no existen en un PC
             * doméstico. Vacíos, el backend cae a memoria y a los stubs que ya
             * tiene; sin esto, algunos envían correos reales o fallan al
             * arrancar por configuración incompleta. */
            .env_remove("REDIS_URL")
            .env_remove("STRIPE_SECRET_KEY")
            .env_remove("SMTP_HOST")
            .env("KAMPLES_AUTOMATION_ENABLED", "false")
            .stdin(Stdio::null())
            .stdout(Stdio::from(salida))
            .stderr(Stdio::from(errores))
            .spawn()
            .map_err(|e| format!("no se pudo lanzar el backend: {e}"))?;

        let mut backend = Self {
            hijo,
            url: format!("http://127.0.0.1:{puerto}"),
        };
        /* Sin esto, un cierre brusco del lanzador dejaba el backend vivo sirviendo
         * en 4317: la siguiente apertura creía que había una instancia buena. */
        if let Some(trabajo) = trabajo {
            trabajo.asignar(&backend.hijo);
        }
        backend.esperar(Duration::from_secs(90))?;
        Ok(backend)
    }

    /// Espera a que `/api/health` conteste. El primer arranque aplica
    /// migraciones en la base y tarda más, así que el tope es generoso.
    fn esperar(&mut self, tope: Duration) -> Result<(), String> {
        let inicio = Instant::now();
        while inicio.elapsed() < tope {
            if let Some(estado) = self.hijo.try_wait().ok().flatten() {
                return Err(format!(
                    "el backend terminó al arrancar ({estado}); mira el log de datos/logs"
                ));
            }
            if TcpStream::connect(("127.0.0.1", puerto_de(&self.url))).is_ok() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        Err("el backend no abrió su puerto en 90 s".into())
    }
}

impl Drop for Backend {
    fn drop(&mut self) {
        let _ = self.hijo.kill();
        let _ = self.hijo.wait();
    }
}

fn puerto_de(url: &str) -> u16 {
    url.rsplit(':')
        .next()
        .and_then(|p| p.parse().ok())
        .unwrap_or(4317)
}

/// La URL se arma aquí y no en `postgres` para no arrastrar credenciales por
/// toda la app: el rol y la clave están fijos y ya viven en su módulo.
fn url_base(puerto: u16) -> String {
    format!(
        "postgres://{}:{}@127.0.0.1:{puerto}/{}",
        postgres::ROL,
        postgres::CLAVE,
        postgres::BASE
    )
}

/// Secretos de firma. Se generan la primera vez y se guardan en los datos: si
/// cambiaran en cada arranque, todas las sesiones iniciadas se invalidarían y
/// la usuaria tendría que entrar de nuevo cada vez que abre la app.
pub struct Secretos {
    pub jwt: String,
    pub ws: String,
}

impl Secretos {
    pub fn cargar(r: &Rutas) -> Result<Self, String> {
        let archivo = r.datos.join("secretos.txt");
        if let Ok(texto) = std::fs::read_to_string(&archivo) {
            let mut lineas = texto.lines().filter(|l| !l.trim().is_empty());
            if let (Some(jwt), Some(ws)) = (lineas.next(), lineas.next()) {
                return Ok(Self {
                    jwt: jwt.trim().to_string(),
                    ws: ws.trim().to_string(),
                });
            }
        }
        let secretos = Self {
            jwt: aleatorio(),
            ws: aleatorio(),
        };
        let mut f = std::fs::File::create(&archivo)
            .map_err(|e| format!("no se pudo guardar los secretos: {e}"))?;
        writeln!(f, "{}\n{}", secretos.jwt, secretos.ws)
            .map_err(|e| format!("no se pudo escribir los secretos: {e}"))?;
        Ok(secretos)
    }
}

/// 32 bytes del generador criptográfico del sistema (`BCryptGenRandom` en
/// Windows), en hexadecimal. Un hash del reloj sería adivinable y permitiría
/// falsificar un JWT de administración.
fn aleatorio() -> String {
    let mut bytes = [0u8; 32];
    if getrandom::getrandom(&mut bytes).is_err() {
        /* [por que] Sin entropía del sistema las sesiones no se pueden firmar
         * de forma segura: se prefiere no arrancar a arrancar con una clave
         * predecible. */
        panic!("el sistema no dio entropía para los secretos de sesión");
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
