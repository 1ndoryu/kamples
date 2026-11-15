/* Manifiesto de permisos de los comandos propios de la app.
 *
 * [por que] En Tauri 2, un `invoke` desde una página que no es el propio
 * bundle (`tauri://localhost`) solo funciona si el comando está concedido en
 * una capability que declare `remote`. Las capabilities declaran plugins, no
 * comandos de aplicación: estos se publican con un manifiesto generado aquí y
 * se conceden como `<plugin>` (ver `capabilities/principal.json`).
 *
 * Sin esto, la ventana cargaba el front desde `http://127.0.0.1:4317` y moría
 * con "Command leer_deep_link_android_pendiente not allowed by ACL".
 */

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "obtener_version",
            "obtener_plataforma",
            "archivo_existe",
            "obtener_tamano_archivo",
            "leer_token_fcm_android",
            "leer_navegacion_fcm_pendiente",
            "leer_deep_link_android_pendiente",
            "obtener_espacio_disponible",
            "abrir_carpeta",
            "seleccionar_archivo",
            "mostrar_ventana_config",
            "toggle_ventana_sync",
            "toggle_devtools",
            "iniciar_oauth_google",
        ]),
    ))?;
    Ok(())
}
