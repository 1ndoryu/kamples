// [deploy-fase0] Middleware de compatibilidad: reescribe /wp-json/kamples/v1/X → /api/X
//
// La app Tauri desktop tiene un adaptador client-side (wpJsonRustAdapter.ts) que
// reescribe URLs antes de hacer fetch. Este middleware añade la capa server-side
// para que:
//   1. Si el adaptador client-side no está activo (build viejo, deep link directo),
//      el backend igual responde correctamente.
//   2. El endpoint de updater de Tauri (que hace requests directas sin el adapter)
//      funcione contra el backend Rust.
//
// La reescritura es transparente: no hay redirect HTTP, se reescribe el URI
// internamente antes de que el router lo procese.

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;

/// Prefijo legacy que la app Tauri y el antiguo WordPress usaban.
const WP_PREFIX: &str = "/wp-json/kamples/v1";

pub async fn wp_rewrite_middleware(mut req: Request, next: Next) -> Response {
    let path = req.uri().path();

    if let Some(rest) = path.strip_prefix(WP_PREFIX) {
        // rest puede ser "" (si path == WP_PREFIX exacto) o "/algo/..."
        let new_path = if rest.is_empty() {
            "/api".to_string()
        } else {
            // rest ya empieza con "/" porque strip_prefix corta "/wp-json/kamples/v1"
            // y lo que queda es "/sync/changelog" etc.
            format!("/api{rest}")
        };

        // Preservar query string si existe
        let new_uri = if let Some(qs) = req.uri().query() {
            format!("{new_path}?{qs}")
        } else {
            new_path
        };

        tracing::debug!(
            original = %req.uri(),
            rewritten = %new_uri,
            "WP-JSON rewrite applied"
        );

        /* Reescribir el URI en el request. Si no parsea se deja el original y
         * se registra: mandar la peticion a /api seria enrutarla a otro sitio. */
        match new_uri.parse() {
            Ok(uri) => *req.uri_mut() = uri,
            Err(error) => tracing::error!(%new_uri, %error, "no se pudo parsear el URI reescrito"),
        }
    }

    next.run(req).await
}
