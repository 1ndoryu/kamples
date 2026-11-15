/* Kamples en el escritorio de Windows: app autocontenida.
 *
 * [por que] La usuaria usa KAMPLES con sus samples y datos reales, así que no
 * puede depender de la VPS 89 ni de un PostgreSQL instalado a mano. Este
 * proceso arranca su propio PostgreSQL portable (con pgvector) sobre una
 * carpeta de datos estable, aplica las migraciones la primera vez y lanza el
 * backend Axum empotrado, que sirve el front compilado y el pipeline de audio
 * con el ffmpeg que viaja al lado.
 *
 * Dónde vive cada cosa:
 *   programa  -> junto al .exe (PostgreSQL portable, ffmpeg, front, migraciones)
 *   datos     -> %LOCALAPPDATA%\KamplesDatos (base de datos, uploads, backups)
 * Los datos nunca se mezclan con el programa: actualizar la app no los toca.
 */

pub mod ajustes;
pub mod arranque;
pub mod backend;
pub mod datos;
pub mod ffmpeg;
pub mod migraciones;
pub mod postgres;
pub mod trabajo;
pub mod ventanas;
