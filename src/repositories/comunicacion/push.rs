use sqlx::PgPool;

use crate::errors::AppError;

pub struct PushSubscriptionRepository;

pub struct RegisterPushSubscriptionRecord<'a> {
    pub user_id: i32,
    pub endpoint: &'a str,
    pub p256dh: &'a str,
    pub auth: &'a str,
    pub platform: &'a str,
}

#[derive(Debug, Clone)]
pub struct PushSubscriptionRecord {
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
    pub platform: String,
}

#[derive(Debug)]
struct PushSubscriptionRow {
    endpoint: String,
    p256dh: String,
    auth: String,
    platform: String,
}

/* [174A-75] Persistencia Web Push VAPID.
 * Se conserva el upsert por endpoint del legado, pero el desregistro ahora
 * exige usuario + endpoint para no permitir bajas cruzadas entre cuentas. */

impl PushSubscriptionRepository {
    pub async fn upsert(
        pool: &PgPool,
        record: RegisterPushSubscriptionRecord<'_>,
    ) -> Result<(), AppError> {
        sqlx::query!(
            r#"INSERT INTO push_subscriptions (
                    usuario_id,
                    endpoint,
                    p256dh,
                    auth,
                    plataforma,
                    activa
               )
               VALUES ($1, $2, $3, $4, $5, TRUE)
               ON CONFLICT (endpoint)
               DO UPDATE SET
                    usuario_id = EXCLUDED.usuario_id,
                    p256dh = EXCLUDED.p256dh,
                    auth = EXCLUDED.auth,
                    plataforma = EXCLUDED.plataforma,
                    activa = TRUE,
                    updated_at = NOW()"#,
            record.user_id,
            record.endpoint,
            record.p256dh,
            record.auth,
            record.platform,
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn delete_for_user_by_endpoint(
        pool: &PgPool,
        user_id: i32,
        endpoint: &str,
    ) -> Result<bool, AppError> {
        let result = sqlx::query!(
            r#"DELETE FROM push_subscriptions
               WHERE usuario_id = $1 AND endpoint = $2"#,
            user_id,
            endpoint,
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn list_active_by_user(
        pool: &PgPool,
        user_id: i32,
    ) -> Result<Vec<PushSubscriptionRecord>, AppError> {
        let rows: Vec<PushSubscriptionRow> = sqlx::query_as!(
            PushSubscriptionRow,
            r#"SELECT endpoint AS "endpoint!",
                      p256dh AS "p256dh!",
                      auth AS "auth!",
                      plataforma AS "platform!"
               FROM push_subscriptions
               WHERE usuario_id = $1 AND activa = TRUE
               ORDER BY id DESC"#,
            user_id,
        )
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| PushSubscriptionRecord {
                endpoint: row.endpoint,
                p256dh: row.p256dh,
                auth: row.auth,
                platform: row.platform,
            })
            .collect())
    }

    pub async fn mark_inactive(pool: &PgPool, endpoint: &str) -> Result<(), AppError> {
        sqlx::query!(
            r#"UPDATE push_subscriptions
               SET activa = FALSE,
                   updated_at = NOW()
               WHERE endpoint = $1"#,
            endpoint,
        )
        .execute(pool)
        .await?;

        Ok(())
    }
}
