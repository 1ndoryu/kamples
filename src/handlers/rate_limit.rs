/* [16A-8] Rate limit global por IP para las rutas mutantes.
 *
 * Por qué existe: la migración PHP → Rust dejó fuera el limitador. Los
 * handlers lo dicen en sus comentarios ("NO portado: falta RateLimiter
 * global"), y mientras tanto /api/auth/login, /api/payments y los 20 y pico
 * ficheros de admin aceptaban POST sin tope. La regla
 * `ruta-post-sin-rate-limit` de Sentinel marcaba 61 rutas por eso.
 *
 * Diseño: cubo de fichas por IP, en memoria del proceso. Es suficiente
 * mientras haya una sola instancia; con varias, cada una limita por su
 * cuenta (el tope efectivo se multiplica por el número de réplicas). Si el
 * día que se escala horizontalmente esto molesta, el sitio natural del
 * contador es Redis, que ya está en el stack.
 *
 * Solo se limitan los métodos mutantes. Un GET de las rutas de imagen sirve
 * los assets de una página entera y limitarlo rompería la UI sin ganar nada:
 * el riesgo que cubre la regla es el POST.
 *
 * La clave es la IP del cliente. El backend vive detrás del proxy de
 * Coolify, así que la IP real viene en cabecera; el peer de la conexión es
 * el proxy y solo se usa como último recurso. Confiar en `X-Forwarded-For`
 * sin proxy delante permitiría falsificar la clave, pero el despliegue
 * siempre lleva proxy: si algún día se expone el binario a pelo, hay que
 * quitar esas cabeceras de `CABECERAS_IP`. */

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, Request, State};
use axum::http::{Extensions, HeaderMap, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;

/* Orden de precedencia: las dos primeras las pone el proxy y traen una sola
 * IP; `x-forwarded-for` es una cadena "cliente, proxy1, proxy2" y se toma la
 * primera entrada, que es la del cliente original. */
const CABECERAS_IP: [&str; 3] = ["cf-connecting-ip", "x-real-ip", "x-forwarded-for"];

/* Poda de cubos ociosos: sin ella el mapa crece con cada IP que pasa una vez
 * (un escaneo de puertos bastaría para llenar la memoria del proceso). */
const INTERVALO_PODA: Duration = Duration::from_secs(60);
const MAX_ENTRADAS: usize = 20_000;

struct Cubo {
    fichas: f64,
    ultima: Instant,
}

/* Estado compartido de la capa. `Mutex` de std, no de tokio: la sección
 * crítica no hace `await`, así que no hay motivo para el coste del async. */
pub struct Limitador {
    cubos: Mutex<HashMap<String, Cubo>>,
    ultima_poda: Mutex<Instant>,
    capacidad: f64,
    por_segundo: f64,
}

impl Limitador {
    /* [16A-8] Atajo para el cableado: `handlers/mod.rs` pasa la config tal cual. */
    pub fn desde(config: &crate::config::RateLimitConfig) -> Arc<Self> {
        Self::nuevo(config.peticiones_por_segundo, config.rafaga)
    }

    pub fn nuevo(por_segundo: f64, rafaga: f64) -> Arc<Self> {
        Arc::new(Self {
            cubos: Mutex::new(HashMap::new()),
            ultima_poda: Mutex::new(Instant::now()),
            capacidad: rafaga.max(1.0),
            por_segundo: por_segundo.max(0.01),
        })
    }

    /* Descuenta una ficha de la clave. Devuelve los segundos que hay que
     * esperar para poder reintentar si el cubo está vacío. */
    fn consumir(&self, clave: &str) -> Result<(), u64> {
        let mut cubos = self.cubos.lock().unwrap_or_else(PoisonError::into_inner);
        self.podar(&mut cubos);

        let ahora = Instant::now();
        let cubo = cubos.entry(clave.to_string()).or_insert(Cubo {
            fichas: self.capacidad,
            ultima: ahora,
        });

        let transcurrido = ahora.duration_since(cubo.ultima).as_secs_f64();
        cubo.fichas = (cubo.fichas + transcurrido * self.por_segundo).min(self.capacidad);
        cubo.ultima = ahora;

        if cubo.fichas >= 1.0 {
            cubo.fichas -= 1.0;
            return Ok(());
        }

        /* `faltan` es positivo y finito (fichas >= 0, por_segundo > 0); el
         * tope de un día evita que un cubo raro desborde la conversión. */
        let faltan = (1.0 - cubo.fichas) / self.por_segundo;
        let espera = Duration::from_secs_f64(faltan.clamp(0.001, 86_400.0));
        Err(espera.as_secs().max(1))
    }

    /* Se llama dentro del lock. Si el mapa pasa del tope, se tiran los cubos
     * llenos y quietos; si aun así no baja, se vacía entero: es un limitador,
     * perder la cuenta de una IP solo le regala una ráfaga. */
    fn podar(&self, cubos: &mut HashMap<String, Cubo>) {
        let mut ultima = self.ultima_poda.lock().unwrap_or_else(PoisonError::into_inner);
        if ultima.elapsed() < INTERVALO_PODA && cubos.len() < MAX_ENTRADAS {
            return;
        }
        *ultima = Instant::now();
        drop(ultima);

        let lleno = self.capacidad;
        let quieto = Duration::from_secs(10);
        cubos.retain(|_, cubo| cubo.fichas < lleno || cubo.ultima.elapsed() < quieto);
        if cubos.len() >= MAX_ENTRADAS {
            cubos.clear();
        }
    }
}

fn es_mutante(metodo: &Method) -> bool {
    matches!(
        *metodo,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

/* La IP del cliente, o el peer si el proxy no dejó cabecera. */
fn ip_cliente(cabeceras: &HeaderMap, peer: IpAddr) -> String {
    for nombre in CABECERAS_IP {
        let Some(valor) = cabeceras.get(nombre).and_then(|v| v.to_str().ok()) else {
            continue;
        };
        let primera = valor.split(',').next().unwrap_or("").trim();
        if primera.is_empty() {
            continue;
        }
        /* `to_canonical` desmapea la forma IPv6 de una IPv4 (::ffff:1.2.3.4)
         * para que la misma máquina no cuente como dos clientes distintos. */
        if let Ok(ip) = primera.parse::<IpAddr>() {
            return ip.to_canonical().to_string();
        }
    }
    peer.to_string()
}

/* El peer de la conexión. Lo deja en las extensiones
 * `into_make_service_with_connect_info` (ver `main.rs`). Se lee de ahí en vez
 * de con el extractor `ConnectInfo` a propósito: el extractor falla —y la
 * petición acabaría en un 500— cuando el router se llama sin ese servicio,
 * como en una prueba con `oneshot`. Sin dato se usa una clave fija, que
 * simplemente reparte el mismo cubo entre todos. */
fn peer_de(extensiones: &Extensions) -> IpAddr {
    extensiones
        .get::<ConnectInfo<SocketAddr>>()
        .map_or(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED), |c| c.0.ip())
}

/* Capa de axum: `from_fn_with_state(limitador, limitar)`. */
pub async fn limitar(
    State(limitador): State<Arc<Limitador>>,
    peticion: Request,
    siguiente: Next,
) -> Response {
    if !es_mutante(peticion.method()) {
        return siguiente.run(peticion).await;
    }

    let clave = ip_cliente(peticion.headers(), peer_de(peticion.extensions()));
    if let Err(segundos) = limitador.consumir(&clave) {
        tracing::warn!(ip = %clave, "rate limit alcanzado");
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [("retry-after", segundos.to_string())],
            Json(serde_json::json!({
                "error": "demasiadas_peticiones",
                "reintentar_en_segundos": segundos,
            })),
        )
            .into_response();
    }

    siguiente.run(peticion).await
}

#[cfg(test)]
#[path = "rate_limit/tests.rs"]
mod tests;
