use sqlx::PgPool;

use super::{
    BillingRepository, StripeUserProfile, SubscriptionRecord, UpsertStripeSubscriptionRecord,
};

impl BillingRepository {
    pub async fn find_stripe_user_profile(
        pool: &PgPool,
        user_id: i32,
    ) -> Result<Option<StripeUserProfile>, sqlx::Error> {
        sqlx::query_as!(
            StripeUserProfile,
            r#"
            SELECT
                id AS user_id,
                username,
                email,
                nombre_visible AS "display_name!",
                plan,
                stripe_customer_id,
                stripe_connect_id
            FROM usuarios_ext
            WHERE id = $1
            LIMIT 1
            "#,
            user_id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn save_stripe_customer_id(
        pool: &PgPool,
        user_id: i32,
        customer_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE usuarios_ext
            SET stripe_customer_id = $2
            WHERE id = $1
            "#,
            user_id,
            customer_id
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn find_user_profile_by_customer_id(
        pool: &PgPool,
        customer_id: &str,
    ) -> Result<Option<StripeUserProfile>, sqlx::Error> {
        sqlx::query_as!(
            StripeUserProfile,
            r#"
            SELECT
                id AS user_id,
                username,
                email,
                nombre_visible AS "display_name!",
                plan,
                stripe_customer_id,
                stripe_connect_id
            FROM usuarios_ext
            WHERE stripe_customer_id = $1
            LIMIT 1
            "#,
            customer_id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn save_stripe_connect_id(
        pool: &PgPool,
        user_id: i32,
        connect_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE usuarios_ext
            SET stripe_connect_id = $2
            WHERE id = $1
            "#,
            user_id,
            connect_id
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn promote_user_to_creator(pool: &PgPool, user_id: i32) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE usuarios_ext
            SET rol = 'creador'
            WHERE id = $1
              AND rol = 'usuario'
            "#,
            user_id
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn update_user_plan(
        pool: &PgPool,
        user_id: i32,
        plan: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE usuarios_ext
            SET plan = $2
            WHERE id = $1
            "#,
            user_id,
            plan
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn find_active_subscription(
        pool: &PgPool,
        user_id: i32,
    ) -> Result<Option<SubscriptionRecord>, sqlx::Error> {
        sqlx::query_as!(
            SubscriptionRecord,
            r#"
            SELECT
                id,
                usuario_id AS user_id,
                plan,
                estado AS "status!",
                stripe_subscription_id,
                inicio_at AS start_at,
                fin_at AS end_at,
                created_at AS "created_at!"
            FROM suscripciones
            WHERE usuario_id = $1
              AND estado IN ('activa', 'periodo_prueba')
            ORDER BY COALESCE(fin_at, NOW()) DESC, created_at DESC
            LIMIT 1
            "#,
            user_id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn upsert_subscription_from_stripe(
        pool: &PgPool,
        record: &UpsertStripeSubscriptionRecord,
    ) -> Result<SubscriptionRecord, sqlx::Error> {
        if let Some(updated) = sqlx::query_as!(
            SubscriptionRecord,
            r#"
            UPDATE suscripciones
            SET usuario_id = $1,
                plan = $2,
                estado = $3,
                inicio_at = $4,
                fin_at = $5
            WHERE stripe_subscription_id = $6
            RETURNING
                id,
                usuario_id AS user_id,
                plan,
                estado AS "status!",
                stripe_subscription_id,
                inicio_at AS start_at,
                fin_at AS end_at,
                created_at AS "created_at!"
            "#,
            record.user_id,
            record.plan,
            record.status,
            record.start_at,
            record.end_at,
            record.stripe_subscription_id
        )
        .fetch_optional(pool)
        .await?
        {
            return Ok(updated);
        }

        sqlx::query_as!(
            SubscriptionRecord,
            r#"
            INSERT INTO suscripciones (
                usuario_id,
                plan,
                estado,
                stripe_subscription_id,
                inicio_at,
                fin_at
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING
                id,
                usuario_id AS user_id,
                plan,
                estado AS "status!",
                stripe_subscription_id,
                inicio_at AS start_at,
                fin_at AS end_at,
                created_at AS "created_at!"
            "#,
            record.user_id,
            record.plan,
            record.status,
            record.stripe_subscription_id,
            record.start_at,
            record.end_at,
        )
        .fetch_one(pool)
        .await
    }
}
