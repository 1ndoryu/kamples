/* Ventanas: cargan el front del backend, o el error de arranque.
 *
 * [por que] Una app que se abre en blanco no se puede diagnosticar. Si el
 * arranque falla, la ventana principal muestra el error y la ruta del log; el
 * tray y los comandos siguen existiendo, así que la app se cierra con "Salir".
 */

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

/// Crea las tres ventanas apuntando al backend.
///
/// [por que se crean aquí] La app sirve el front desde su propio backend, y
/// `frontendDist` solo sabe servir archivos empotrados. Con las ventanas
/// creadas en código, el origen es una URL http normal y `/api` y `/uploads`
/// resuelven en el mismo sitio que en producción.
pub fn apuntar(app: &tauri::App, base: &str) -> Result<(), Box<dyn std::error::Error>> {
    let url = |ruta: &str| -> WebviewUrl {
        WebviewUrl::External(format!("{base}/{ruta}").parse().expect("URL del backend válida"))
    };

    /* [por que] El front sin configurar apunta a `https://kamples.com`: busca
     * `window.__KAMPLES_CONFIG__` y, si no está, usa el dominio de producción.
     * Se inyecta antes de cargar la página para que todas las peticiones vayan
     * al backend empotrado; `location.origin` es el 127.0.0.1:4317 real. */
    let configurar_api = "window.__KAMPLES_CONFIG__ = { serverUrl: window.location.origin + '/wp-json' };";

    /* [por que la raíz y no `index.html`] El router del front no tiene ruta
     * `/index.html`: renderiza "Pagina no encontrada". En `tauri://localhost`
     * el bundle servía el archivo directamente; desde el backend es una URL
     * más, y hay que pedir `/`, que es la que el router sí conoce. */
    let principal = WebviewWindowBuilder::new(app, "main", url(""))
        .initialization_script(configurar_api)
        .title("Kamples")
        .inner_size(1280.0, 800.0)
        .min_inner_size(900.0, 600.0)
        .center()
        .build()?;
    /* Cerrar la ventana principal la oculta, no la destruye: el tray tiene que
     * poder volver a mostrarla. Para cerrar de verdad está "Salir". */
    let app_handle = app.handle().clone();
    principal.on_window_event(move |evento| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = evento {
            api.prevent_close();
            if let Some(w) = app_handle.get_webview_window("main") {
                let _ = w.hide();
            }
        }
    });

    /* Popup de sincronización: sin decoración, siempre encima y oculta al
     * principio, igual que estaba declarada en `tauri.conf.json`. */
    let sync = WebviewWindowBuilder::new(app, "sync-panel", url("sync.html"))
        .initialization_script(configurar_api)
        .title("Kamples Sync")
        .inner_size(380.0, 520.0)
        .decorations(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .transparent(true)
        .build()?;
    let handle_sync = app.handle().clone();
    sync.on_window_event(move |evento| {
        if let tauri::WindowEvent::Focused(false) = evento {
            let handle = handle_sync.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(220));
                if let Some(v) = handle.get_webview_window("sync-panel") {
                    if !v.is_focused().unwrap_or(true) {
                        let _ = v.hide();
                    }
                }
            });
        }
    });

    WebviewWindowBuilder::new(app, "config-sync", url("config.html"))
        .initialization_script(configurar_api)
        .title("Configuración")
        .inner_size(460.0, 500.0)
        .decorations(false)
        .resizable(false)
        .visible(false)
        .build()?;

    Ok(())
}

/// Sustituye la ventana principal por una página con el error de arranque.
pub fn mostrar_error(app: &tauri::App, error: &str) -> Result<(), Box<dyn std::error::Error>> {
    let ruta = escritorio_logs();
    let html = format!(
        "<!doctype html><html lang=es><meta charset=utf-8>\
         <title>Kamples no pudo arrancar</title>\
         <body style=\"font-family:Segoe UI,system-ui,sans-serif;background:#14161a;\
         color:#e6e8eb;padding:48px;line-height:1.6\">\
         <h1 style=\"font-size:22px;margin:0 0 16px\">Kamples no pudo arrancar</h1>\
         <p style=\"margin:0 0 12px\">{}</p>\
         <p style=\"margin:0 0 12px;color:#9aa4b2\">Los detalles están en:<br>\
         <code style=\"color:#e6e8eb\">{}</code></p>\
         <p style=\"margin:24px 0 0;color:#9aa4b2\">Cierra esta ventana desde el icono \
         de Kamples en la bandeja del sistema y vuelve a abrirla.</p></body></html>",
        escapar(error),
        escapar(&ruta),
    );

    /* Sin ventanas creadas todavía (el arranque falló antes), se usa la
     * principal para no dejar la app sin nada en pantalla. */
    let ventana = match app.get_webview_window("main") {
        Some(v) => v,
        None => WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
            .title("Kamples")
            .inner_size(880.0, 560.0)
            .center()
            .build()?,
    };
    ventana.navigate(tauri::Url::parse(&format!(
        "data:text/html;charset=utf-8,{}",
        percent_encoding(&html)
    ))?)?;
    ventana.show().ok();
    ventana.set_focus().ok();
    Ok(())
}

fn escritorio_logs() -> String {
    std::env::var_os("LOCALAPPDATA")
        .map(|p| {
            std::path::PathBuf::from(p)
                .join(super::ajustes::CARPETA_DATOS)
                .join("logs")
                .display()
                .to_string()
        })
        .unwrap_or_else(|| "la carpeta de logs de Kamples".to_string())
}

/// El mensaje puede traer rutas con `&` o `<`; sin esto la página de error se
/// rompe justo cuando más falta hace leerla.
fn escapar(texto: &str) -> String {
    texto
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Codificación pensada para `data:` URLs: todo lo que no sea
/// inofensivo en una URL va escapado. Se hace a mano para no añadir otra
/// dependencia solo por esto.
fn percent_encoding(texto: &str) -> String {
    let mut salida = String::with_capacity(texto.len() * 3);
    for byte in texto.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                salida.push(byte as char);
            }
            _ => salida.push_str(&format!("%{byte:02X}")),
        }
    }
    salida
}
