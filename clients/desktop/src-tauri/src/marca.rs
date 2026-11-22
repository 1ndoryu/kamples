#![allow(dead_code)] // constantes de marca: cada crate usa unas pocas
/* GENERADO por scripts/marca/generar.mjs desde marca.json — no editar a mano.
 *
 * [por que] El nombre visible y lo que identifica a la app en disco son cosas
 * distintas: el nombre puede cambiar cuando se decida la marca definitiva, pero
 * `IDENTIDAD_*` no, porque la carpeta de datos, la base local y el
 * `identifier` de Tauri ya existen en los equipos con datos dentro.
 */

/// Nombre visible de la app: títulos, bandeja, correos.
pub const NOMBRE: &str = "Kamples";
/// Nombre corto, para donde el largo no cabe.
pub const NOMBRE_CORTO: &str = "Kamples";

/// Identidad fija: cambiarla exige migrar los datos existentes.
pub const IDENTIDAD_SLUG: &str = "kamples";
pub const IDENTIDAD_CARPETA_DATOS: &str = "KamplesDatos";
pub const IDENTIDAD_DOMINIO: &str = "kamples.com";
/// Sitio público, para enlaces en correos y valores por defecto.
pub const SITIO_URL: &str = "https://kamples.com";
