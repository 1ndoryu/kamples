use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};
use utoipa::ToSchema;

use crate::errors::AppError;

#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
pub struct AdminIaQueueItem {
    pub id: i32,
    pub tipo: String,
    pub entidad_id: i32,
    pub operacion: String,
    pub estado: String,
    pub intentos: i32,
    pub max_intentos: i32,
    pub ultimo_error: Option<String>,
    pub proximo_intento: Option<DateTime<Utc>>,
    #[schema(value_type = Object)]
    pub metadata: Value,
    pub procesado_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
pub struct AdminIaQueueStats {
    pub total: i64,
    pub pendientes: i64,
    pub procesando: i64,
    pub completados_hoy: i64,
    pub en_reintento: i64,
    pub errores: i64,
    pub encolados_hoy: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct AdminIaQueueListParams<'a> {
    pub page: i64,
    pub limit: i64,
    pub estado: Option<&'a str>,
    pub tipo: Option<&'a str>,
    pub busqueda: Option<&'a str>,
    pub sort_col: Option<&'a str>,
    pub sort_dir: Option<&'a str>,
}

pub struct AdminIaQueueRepository;

impl AdminIaQueueRepository {
    pub async fn list(
        pool: &PgPool,
        params: AdminIaQueueListParams<'_>,
    ) -> Result<Vec<AdminIaQueueItem>, AppError> {
        let page = params.page.max(1);
        let limit = params.limit.clamp(5, 100);
        let offset = (page - 1) * limit;
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT id, tipo, entidad_id, operacion, estado, intentos, max_intentos, \
             ultimo_error, proximo_intento, COALESCE(metadata, '{}'::jsonb) AS metadata, \
             procesado_at, created_at FROM cola_procesamiento_ia",
        );
        push_filters(&mut builder, params.estado, params.tipo, params.busqueda);
        builder.push(" ORDER BY ");
        builder.push(resolve_sort_col(params.sort_col));
        builder.push(
            if params
                .sort_dir
                .is_some_and(|value| value.eq_ignore_ascii_case("ASC"))
            {
                " ASC"
            } else {
                " DESC"
            },
        );
        builder.push(", id DESC LIMIT ");
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        let rows = builder
            .build_query_as::<AdminIaQueueItem>()
            .fetch_all(pool)
            .await?;
        Ok(rows)
    }

    pub async fn stats(pool: &PgPool) -> Result<AdminIaQueueStats, AppError> {
        let row = sqlx::query_as!(
            AdminIaQueueStats,
            r#"SELECT
                COUNT(*) AS "total!",
                COUNT(*) FILTER (WHERE estado = 'pendiente') AS "pendientes!",
                COUNT(*) FILTER (WHERE estado = 'procesando') AS "procesando!",
                COUNT(*) FILTER (WHERE estado = 'completado' AND procesado_at >= CURRENT_DATE) AS "completados_hoy!",
                COUNT(*) FILTER (WHERE estado = 'error_reintento') AS "en_reintento!",
                COUNT(*) FILTER (WHERE estado = 'error_final') AS "errores!",
                COUNT(*) FILTER (WHERE created_at >= CURRENT_DATE) AS "encolados_hoy!"
             FROM cola_procesamiento_ia"#,
        )
        .fetch_one(pool)
        .await?;
        Ok(row)
    }

    pub async fn retry_one(pool: &PgPool, id: i32) -> Result<bool, AppError> {
        let updated = sqlx::query!(
            "UPDATE cola_procesamiento_ia
             SET estado = 'pendiente', intentos = 0, ultimo_error = NULL, proximo_intento = NULL
             WHERE id = $1",
            id,
        )
        .execute(pool)
        .await?
        .rows_affected();
        Ok(updated > 0)
    }

    pub async fn retry_all(pool: &PgPool) -> Result<u64, AppError> {
        let updated = sqlx::query!(
            "UPDATE cola_procesamiento_ia
             SET estado = 'pendiente', intentos = 0, ultimo_error = NULL, proximo_intento = NULL
             WHERE estado IN ('error_reintento', 'error_final')",
        )
        .execute(pool)
        .await?
        .rows_affected();
        Ok(updated)
    }
}

fn push_filters(
    builder: &mut QueryBuilder<Postgres>,
    estado: Option<&str>,
    tipo: Option<&str>,
    busqueda: Option<&str>,
) {
    let mut has_filter = false;
    if let Some(value) = estado.filter(|value| is_valid_estado(value)) {
        push_condition_prefix(builder, &mut has_filter);
        builder.push("estado = ").push_bind(value.to_owned());
    }
    if let Some(value) = tipo.filter(|value| is_valid_tipo(value)) {
        push_condition_prefix(builder, &mut has_filter);
        builder.push("tipo = ").push_bind(value.to_owned());
    }
    if let Some(value) = busqueda.map(str::trim).filter(|value| !value.is_empty()) {
        push_condition_prefix(builder, &mut has_filter);
        let pattern = format!("%{value}%");
        builder
            .push("(operacion ILIKE ")
            .push_bind(pattern.clone())
            .push(" OR COALESCE(ultimo_error, '') ILIKE ")
            .push_bind(pattern.clone())
            .push(" OR metadata::text ILIKE ")
            .push_bind(pattern)
            .push(")");
    }
}

fn push_condition_prefix(builder: &mut QueryBuilder<Postgres>, has_filter: &mut bool) {
    if *has_filter {
        builder.push(" AND ");
    } else {
        builder.push(" WHERE ");
        *has_filter = true;
    }
}

fn resolve_sort_col(sort_col: Option<&str>) -> &'static str {
    match sort_col {
        Some("id") => "id",
        Some("tipo") => "tipo",
        Some("entidad_id") => "entidad_id",
        Some("operacion") => "operacion",
        Some("estado") => "estado",
        Some("intentos") => "intentos",
        Some("max_intentos") => "max_intentos",
        Some("proximo_intento") => "proximo_intento",
        Some("procesado_at") => "procesado_at",
        _ => "created_at",
    }
}

fn is_valid_estado(value: &str) -> bool {
    matches!(
        value,
        "pendiente" | "procesando" | "completado" | "error_reintento" | "error_final"
    )
}

fn is_valid_tipo(value: &str) -> bool {
    matches!(value, "sample" | "comentario" | "publicacion")
}
