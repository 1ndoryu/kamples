use chrono::{Duration, Utc};
use sqlx::{PgPool, Postgres, QueryBuilder};

use super::shared::{
    like_pattern, normalize_user_order, normalize_user_plan_filter, normalize_user_plan_payload,
    normalize_user_role_payload, push_user_filters, CountRow, UserOrder,
};
use super::AdminPanelRepository;
use crate::errors::AppError;
use crate::models::{
    AdminUserListItem, AdminUserSuspendRequest, AdminUserUpdateRequest, AdminUsersQuery,
    AdminUsersResponse,
};

impl AdminPanelRepository {
    pub async fn list_users(
        pool: &PgPool,
        query: &AdminUsersQuery,
    ) -> Result<AdminUsersResponse, AppError> {
        let page = query.page.unwrap_or(1).max(1);
        let per_page = 20_i64;
        let offset = (page - 1) * per_page;
        let search_like = like_pattern(query.busqueda.as_deref());
        let plan = normalize_user_plan_filter(query.plan.as_deref());
        let order = normalize_user_order(query.orden.as_deref());

        let total = count_users(pool, search_like.as_deref(), plan).await?;
        let data = fetch_users(pool, search_like.as_deref(), plan, order, per_page, offset).await?;

        Ok(AdminUsersResponse { data, total, page })
    }

    pub async fn update_user(
        pool: &PgPool,
        actor_user_id: i32,
        user_id: i32,
        request: &AdminUserUpdateRequest,
    ) -> Result<bool, AppError> {
        if actor_user_id == user_id {
            if matches!(request.rol.as_deref(), Some(rol) if rol.trim() != "admin") {
                return Err(AppError::Forbidden(
                    "No puedes degradar tu propio rol admin".into(),
                ));
            }

            if matches!(request.ban_hasta, Some(Some(_))) {
                return Err(AppError::Forbidden("No puedes banearte a ti mismo".into()));
            }
        }

        let mut builder = QueryBuilder::<Postgres>::new("UPDATE usuarios_ext SET ");
        let mut separated = builder.separated(", ");
        let mut changed = false;

        if let Some(plan) = request.plan.as_deref() {
            let normalized = normalize_user_plan_payload(plan)
                .ok_or_else(|| AppError::Validation("Plan admin inválido".into()))?;
            separated.push("plan = ");
            separated.push_bind(normalized);
            changed = true;
        }

        if let Some(role) = request.rol.as_deref() {
            let normalized = normalize_user_role_payload(role)
                .ok_or_else(|| AppError::Validation("Rol admin inválido".into()))?;
            separated.push("rol = ");
            separated.push_bind(normalized);
            changed = true;
        }

        if let Some(verificado) = request.verificado {
            separated.push("verificado = ");
            separated.push_bind(verificado);
            changed = true;
        }

        if let Some(ban_hasta) = request.ban_hasta {
            if let Some(value) = ban_hasta {
                separated.push("baneado_hasta = ");
                separated.push_bind(value);
            } else {
                separated.push("baneado_hasta = NULL");
                separated.push("ban_razon = NULL");
            }
            changed = true;
        }

        if !changed {
            return Err(AppError::Validation(
                "Debes enviar al menos un cambio de usuario".into(),
            ));
        }

        separated.push("updated_at = NOW()");
        builder.push(" WHERE id = ");
        builder.push_bind(user_id);

        let result = builder.build().execute(pool).await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn suspend_user(
        pool: &PgPool,
        user_id: i32,
        request: &AdminUserSuspendRequest,
    ) -> Result<bool, AppError> {
        let until = Utc::now() + Duration::hours(i64::from(request.horas));

        let mut builder = QueryBuilder::<Postgres>::new("UPDATE usuarios_ext SET ");
        {
            let mut separated = builder.separated(", ");
            separated.push("estado = 'suspendido'");
            separated.push("suspendido_hasta = ");
            separated.push_bind(until);
            separated.push("suspension_razon = ");
            separated.push_bind(request.razon.trim());
            separated.push("updated_at = NOW()");
        }
        builder.push(" WHERE id = ");
        builder.push_bind(user_id);

        let result = builder.build().execute(pool).await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn unsuspend_user(pool: &PgPool, user_id: i32) -> Result<bool, AppError> {
        let mut builder = QueryBuilder::<Postgres>::new("UPDATE usuarios_ext SET ");
        {
            let mut separated = builder.separated(", ");
            separated.push("estado = CASE WHEN sera_eliminado_en IS NOT NULL THEN 'en_eliminacion' ELSE 'activo' END");
            separated.push("suspendido_hasta = NULL");
            separated.push("suspension_razon = NULL");
            separated.push("updated_at = NOW()");
        }
        builder.push(" WHERE id = ");
        builder.push_bind(user_id);

        let result = builder.build().execute(pool).await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn mark_user_for_deletion(pool: &PgPool, user_id: i32) -> Result<bool, AppError> {
        let marked_at = Utc::now();
        let delete_at = marked_at + Duration::days(30);

        let mut builder = QueryBuilder::<Postgres>::new("UPDATE usuarios_ext SET ");
        {
            let mut separated = builder.separated(", ");
            separated.push("estado = 'en_eliminacion'");
            separated.push("marcado_eliminacion_en = ");
            separated.push_bind(marked_at);
            separated.push("sera_eliminado_en = ");
            separated.push_bind(delete_at);
            separated.push("updated_at = NOW()");
        }
        builder.push(" WHERE id = ");
        builder.push_bind(user_id);

        let result = builder.build().execute(pool).await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn cancel_user_deletion(pool: &PgPool, user_id: i32) -> Result<bool, AppError> {
        let mut builder = QueryBuilder::<Postgres>::new("UPDATE usuarios_ext SET ");
        {
            let mut separated = builder.separated(", ");
            separated.push("estado = CASE WHEN estado = 'suspendido' AND suspendido_hasta IS NULL THEN 'suspendido' WHEN suspendido_hasta IS NOT NULL THEN 'suspendido' ELSE 'activo' END");
            separated.push("marcado_eliminacion_en = NULL");
            separated.push("sera_eliminado_en = NULL");
            separated.push("updated_at = NOW()");
        }
        builder.push(" WHERE id = ");
        builder.push_bind(user_id);

        let result = builder.build().execute(pool).await?;
        Ok(result.rows_affected() > 0)
    }
}

async fn count_users(
    pool: &PgPool,
    search_like: Option<&str>,
    plan: Option<&'static str>,
) -> Result<i64, AppError> {
    let mut builder = QueryBuilder::<Postgres>::new("SELECT COUNT(*) AS total FROM usuarios_ext u");
    push_user_filters(&mut builder, search_like, plan);
    let row = builder.build_query_as::<CountRow>().fetch_one(pool).await?;
    Ok(row.total)
}

async fn fetch_users(
    pool: &PgPool,
    search_like: Option<&str>,
    plan: Option<&'static str>,
    order: UserOrder,
    limit: i64,
    offset: i64,
) -> Result<Vec<AdminUserListItem>, AppError> {
    let mut builder = QueryBuilder::<Postgres>::new(
        r"SELECT
                u.id,
                u.username,
                COALESCE(NULLIF(u.nombre_visible, ''), u.username) AS nombre_visible,
                COALESCE(u.email, '') AS email,
                u.avatar_url,
                u.plan,
                u.rol,
                COALESCE(u.verificado, FALSE) AS verificado,
                u.baneado_hasta AS ban_hasta,
                u.estado,
                u.suspendido_hasta,
                u.suspension_razon,
                u.sera_eliminado_en,
                u.created_at,
                u.updated_at,
                (
                    SELECT COUNT(*)::bigint
                    FROM samples s
                    WHERE s.creador_id = u.id
                      AND s.estado = 'activo'
                      AND s.eliminado_en IS NULL
                ) AS total_samples,
                (
                    SELECT COUNT(*)::bigint
                    FROM descargas d
                    WHERE d.usuario_id = u.id
                ) AS total_descargas
           FROM usuarios_ext u",
    );

    push_user_filters(&mut builder, search_like, plan);

    match order {
        UserOrder::Fecha => builder.push(" ORDER BY u.created_at DESC, u.id DESC"),
        UserOrder::Actividad => builder.push(" ORDER BY u.updated_at DESC NULLS LAST, u.id DESC"),
        UserOrder::Samples => builder.push(" ORDER BY total_samples DESC, u.id DESC"),
    };

    builder.push(" LIMIT ");
    builder.push_bind(limit);
    builder.push(" OFFSET ");
    builder.push_bind(offset);

    let rows = builder
        .build_query_as::<AdminUserListItem>()
        .fetch_all(pool)
        .await?;
    Ok(rows)
}
