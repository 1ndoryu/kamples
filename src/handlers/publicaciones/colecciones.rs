/* [174A-64] Colecciones — port mínimo de ColeccionesCrudController.php.
 *
 * Endpoints:
 * - POST   /api/colecciones                            Crear
 * - GET    /api/colecciones                            Listar mías
 * - GET    /api/colecciones/:id                        Obtener (público o propio)
 * - PUT    /api/colecciones/:id                        Actualizar
 * - DELETE /api/colecciones/:id                        Soft-delete
 * - POST   /api/colecciones/:id/samples                Agregar sample
 * - DELETE /api/colecciones/:id/samples/:sample_id     Quitar sample
 * - GET    /api/colecciones/:id/samples                Listar samples (en orden)
 *
 * NO portado:
 * - Optimistic locking (version) — pendiente en el roadmap (18AA-2).
 * - Sync changelog.
 * - Subir imagen multipart.
 * - Admin override.
 * - Eliminación con opciones (manejo hijas, borrar samples).
 */

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::errors::AppError;
use crate::middleware::{CurrentUser, OptionalUser};
use crate::models::SampleSummary;
use crate::repositories::{
    Coleccion, ColeccionSample, ColeccionesRepository, CriterioScoring, SampleContextRow,
    SampleRepository, SavedColeccion, SavedCollectionsRepository,
};
use crate::services::build_sample_summary;
use crate::AppState;

pub mod legacy;
pub mod zip;
pub use zip::descargar_zip_coleccion;

mod crud;
mod sugerencias;
mod merge;
mod guardados;

pub use crud::*;
pub use sugerencias::*;
pub use merge::*;
pub use guardados::*;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/colecciones",
            post(create_coleccion).get(legacy::list_my_colecciones_legacy),
        )
        .route(
            "/colecciones/explorar",
            get(legacy::explore_colecciones_legacy),
        )
        .route(
            "/colecciones/por-slug/:slug",
            get(legacy::get_coleccion_by_slug_legacy),
        )
        .route(
            "/colecciones/relevantes/:sample_id",
            get(legacy::list_relevant_for_sample_legacy),
        )
        .route(
            "/colecciones/:id",
            get(legacy::get_coleccion_legacy)
                .put(update_coleccion)
                .delete(delete_coleccion),
        )
        .route(
            "/colecciones/:id/samples",
            post(add_sample).get(list_samples),
        )
        .route("/colecciones/:id/samples/:sample_id", delete(remove_sample))
        .route(
            "/colecciones/:id/combinacion-pendiente",
            get(combinacion_pendiente),
        )
        .route("/colecciones/:id/sugerencias", get(sugerencias_coleccion))
        .route("/colecciones/:id/merge", post(merge_coleccion))
        .route(
            "/colecciones/:id/save",
            post(save_coleccion).delete(unsave_coleccion),
        )
        .route(
            "/colecciones/:id/guardar",
            post(save_coleccion).delete(unsave_coleccion),
        )
        .route(
            "/colecciones/guardadas",
            get(legacy::list_saved_colecciones_legacy),
        )
        .route(
            "/me/colecciones-guardadas",
            get(legacy::list_saved_colecciones_legacy),
        )
        .route(
            "/colecciones/:id/descargar-zip",
            post(descargar_zip_coleccion),
        )
}
