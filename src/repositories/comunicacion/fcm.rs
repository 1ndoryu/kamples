use sqlx::PgPool;

use crate::errors::AppError;

pub struct FcmTokenRepository;

pub struct RegisterFcmTokenRecord<'a> {
    pub user_id: i32,
    pub token: &'a str,
    pub platform: &'a str,
}

#[derive(Debug, Clone)]
pub struct FcmTokenRecord {
    pub token: String,
    pub platform: String,
}

#[derive(Debug)]
struct FcmTokenRow {
    token: String,
    platform: String,
}

/* [174A-76] Persistencia FCM Android.
 * Se conserva el upsert por token del legado, pero el desregistro ahora se
 * limita al usuario autenticado para evitar bajas cruzadas entre cuentas. */

impl FcmTokenRepository {
    pub async fn upsert(pool: &PgPool, record: RegisterFcmTokenRecord<'_>) -> Result<(), AppError> {
        sqlx::query!(
            r#"INSERT INTO fcm_tokens (
                    usuario_id,
                    token,
                    plataforma,
                    activo
               )
               VALUES ($1, $2, $3, TRUE)
               ON CONFLICT (token)
               DO UPDATE SET
                    usuario_id = EXCLUDED.usuario_id,
                    plataforma = EXCLUDED.plataforma,
                    activo = TRUE,
                    updated_at = NOW()"#,
            record.user_id,
            record.token,
            record.platform,
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn delete_for_user_by_token(
        pool: &PgPool,
        user_id: i32,
        token: &str,
    ) -> Result<bool, AppError> {
        let result = sqlx::query!(
            r#"DELETE FROM fcm_tokens
               WHERE usuario_id = $1 AND token = $2"#,
            user_id,
            token,
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn list_active_by_user(
        pool: &PgPool,
        user_id: i32,
    ) -> Result<Vec<FcmTokenRecord>, AppError> {
        let rows: Vec<FcmTokenRow> = sqlx::query_as!(
            FcmTokenRow,
            r#"SELECT token AS "token!",
                      plataforma AS "platform!"
               FROM fcm_tokens
               WHERE usuario_id = $1 AND activo = TRUE
               ORDER BY id DESC"#,
            user_id,
        )
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| FcmTokenRecord {
                token: row.token,
                platform: row.platform,
            })
            .collect())
    }

    pub async fn mark_inactive(pool: &PgPool, token: &str) -> Result<(), AppError> {
        sqlx::query!(
            r#"UPDATE fcm_tokens
               SET activo = FALSE,
                   updated_at = NOW()
               WHERE token = $1"#,
            token,
        )
        .execute(pool)
        .await?;

        Ok(())
    }
}
