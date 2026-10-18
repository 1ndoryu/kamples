use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::errors::AppError;
#[allow(unused_imports)]
use crate::errors::ErrorResponse;
use crate::middleware::CurrentUser;
use crate::models::{
    AdminActivityQuery, AdminActivityResponse, AdminExtractionQueueQuery,
    AdminExtractionQueueResponse, AdminOkResponse, AdminProcessCookiesRequest,
    AdminProcessStartRequest, AdminProcessesResponse, AdminScrapersQuery, AdminScrapersResponse,
    AdminSummaryStats, AdminUserDeleteRequest, AdminUserSuspendRequest, AdminUserUpdateRequest,
    AdminUsersQuery, AdminUsersResponse, DeleteUserRequest, SuspendUserRequest,
};
use crate::repositories::{AdminPanelRepository, ModerationRepository};
use crate::services::algo_timing::{TimingEntry, ALGO_TIMING};
use crate::services::{AdminProcessService, AdminSamplesService};
use crate::AppState;

/* [174A-25] Endpoints admin: requiere rol admin (validado via require_admin). */

mod usuarios;
mod procesos;
mod diagnostico_muestras;

pub use usuarios::*;
pub use procesos::*;
pub use diagnostico_muestras::*;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/resumen", get(summary))
        .route("/admin/actividad", get(activity))
        .route("/admin/usuarios", get(list_users))
        .route("/admin/usuarios/:id", put(update_user_legacy))
        .route("/admin/usuarios/:id/suspender", post(suspend_user_legacy))
        .route(
            "/admin/usuarios/:id/desuspender",
            post(unsuspend_user_legacy),
        )
        .route("/admin/usuarios/:id/eliminar", post(mark_delete_legacy))
        .route(
            "/admin/usuarios/:id/cancelar-eliminacion",
            post(cancel_delete_legacy),
        )
        .route("/admin/scrapers", get(list_scrapers))
        .route("/admin/cola-extraccion", get(list_extraction_queue))
        .route("/admin/procesos", get(list_processes))
        .route(
            "/admin/procesos/cookies",
            get(process_cookies).post(save_process_cookies),
        )
        .route("/admin/procesos/:nombre", get(process_state))
        .route("/admin/procesos/:nombre/start", post(start_process))
        .route("/admin/procesos/:nombre/stop", post(stop_process))
        .route("/admin/samples/todos", delete(delete_all_samples))
        .route("/admin/users/:id/suspend", post(suspend))
        .route("/admin/users/:id/activate", post(activate))
        .route("/admin/users/:id/delete", post(mark_delete))
        .route(
            "/admin/algo-timing",
            get(algo_timing_history).delete(algo_timing_clear),
        )
}
