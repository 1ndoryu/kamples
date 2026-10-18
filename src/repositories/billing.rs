use chrono::{DateTime, Utc};
use sqlx::PgPool;

/* [174A-79] BillingRepository concentra las columnas Stripe/pagos del dominio Kamples.
 * El wrapper usa este repositorio para evitar que checkout, portal y Connect accedan
 * a `usuarios_ext`/`suscripciones` con SQL duplicado. 174A-82 suma además
 * lookup por customer, actualización de plan y transacciones idempotentes. */

#[derive(Debug, Clone)]
pub struct StripeUserProfile {
    pub user_id: i32,
    pub username: String,
    pub email: Option<String>,
    pub display_name: String,
    pub plan: String,
    pub stripe_customer_id: Option<String>,
    pub stripe_connect_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SubscriptionRecord {
    pub id: i32,
    pub user_id: i32,
    pub plan: String,
    pub status: String,
    pub stripe_subscription_id: Option<String>,
    pub start_at: Option<DateTime<Utc>>,
    pub end_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct UpsertStripeSubscriptionRecord {
    pub user_id: i32,
    pub plan: String,
    pub status: String,
    pub stripe_subscription_id: String,
    pub start_at: Option<DateTime<Utc>>,
    pub end_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct SampleCheckoutCandidate {
    pub sample_id: i32,
    pub sample_title: String,
    pub slug: String,
    pub creator_id: i32,
    pub is_premium: bool,
    pub price: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct CompletedSamplePurchaseInsert {
    pub buyer_id: i32,
    pub creator_id: i32,
    pub sample_id: i32,
    pub amount_cents: i64,
    pub creator_amount_cents: i64,
    pub platform_fee_cents: i64,
    pub stripe_payment_id: Option<String>,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CompletedDownloadRevenueShareInsert {
    pub buyer_id: i32,
    pub creator_id: i32,
    pub sample_id: i32,
    pub amount_cents: i64,
    pub creator_amount_cents: i64,
    pub platform_fee_cents: i64,
}

#[derive(Debug, Clone)]
pub struct CreatorPayoutInsert {
    pub creator_id: i32,
    pub amount_cents: i64,
    pub currency: String,
    pub status: String,
    pub stripe_payout_id: String,
}

pub struct BillingRepository;

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

    pub async fn find_sample_checkout_candidate(
        pool: &PgPool,
        sample_id: i32,
    ) -> Result<Option<SampleCheckoutCandidate>, sqlx::Error> {
        sqlx::query_as!(
            SampleCheckoutCandidate,
            r#"
            SELECT
                id AS sample_id,
                titulo AS "sample_title!",
                slug AS "slug!",
                creador_id AS "creator_id!",
                es_premium AS "is_premium!",
                CAST(precio AS double precision) AS "price?"
            FROM samples
            WHERE id = $1
              AND eliminado_en IS NULL
              AND estado = 'activo'
            LIMIT 1
            "#,
            sample_id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn has_completed_sample_purchase(
        pool: &PgPool,
        buyer_id: i32,
        sample_id: i32,
    ) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM transacciones
                WHERE comprador_id = $1
                  AND sample_id = $2
                  AND tipo = 'compra_sample'
                  AND estado IN ('completada', 'completed')
            ) AS "exists!"
            "#,
            buyer_id,
            sample_id
        )
        .fetch_one(pool)
        .await
    }

    /* [274A-4] Lista los sample_ids comprados (compra individual completada) por
     * un usuario, ordenados por compra más reciente primero. Replica
     * `TransaccionesRepository::listarSamplesComprados` del legacy PHP. */
    pub async fn list_purchased_sample_ids(
        pool: &PgPool,
        buyer_id: i32,
        limit: i64,
    ) -> Result<Vec<i32>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"
            SELECT
                sample_id AS "sample_id!"
            FROM transacciones
            WHERE comprador_id = $1
              AND tipo = 'compra_sample'
              AND estado IN ('completada', 'completed')
              AND sample_id IS NOT NULL
            GROUP BY sample_id
            ORDER BY MAX(created_at) DESC
            LIMIT $2
            "#,
            buyer_id,
            limit
        )
        .fetch_all(pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.sample_id).collect())
    }

    pub async fn insert_completed_sample_purchase(
        pool: &PgPool,
        insert: &CompletedSamplePurchaseInsert,
    ) -> Result<bool, sqlx::Error> {
        let amount = cents_to_decimal_string(insert.amount_cents);
        let creator_amount = cents_to_decimal_string(insert.creator_amount_cents);
        let platform_fee = cents_to_decimal_string(insert.platform_fee_cents);

        Ok(sqlx::query!(
            r#"
            INSERT INTO transacciones (
                comprador_id,
                creador_id,
                sample_id,
                tipo,
                monto,
                moneda,
                pago_creador,
                comision_plataforma,
                estado,
                stripe_payment_id,
                idempotency_key
            )
            VALUES (
                $1,
                $2,
                $3,
                'compra_sample',
                CAST($4 AS text)::numeric,
                'USD',
                CAST($5 AS text)::numeric,
                CAST($6 AS text)::numeric,
                'completada',
                $7,
                $8
            )
            ON CONFLICT DO NOTHING
            RETURNING id
            "#,
            insert.buyer_id,
            insert.creator_id,
            insert.sample_id,
            amount,
            creator_amount,
            platform_fee,
            insert.stripe_payment_id.as_deref(),
            insert.idempotency_key.as_deref()
        )
        .fetch_optional(pool)
        .await?
        .is_some())
    }

    pub async fn insert_completed_download_revenue_share(
        pool: &PgPool,
        insert: &CompletedDownloadRevenueShareInsert,
    ) -> Result<(), sqlx::Error> {
        let amount = cents_to_decimal_string(insert.amount_cents);
        let creator_amount = cents_to_decimal_string(insert.creator_amount_cents);
        let platform_fee = cents_to_decimal_string(insert.platform_fee_cents);

        sqlx::query!(
            r#"
            INSERT INTO transacciones (
                comprador_id,
                creador_id,
                sample_id,
                tipo,
                monto,
                moneda,
                pago_creador,
                comision_plataforma,
                estado
            )
            VALUES (
                $1,
                $2,
                $3,
                'descarga',
                CAST($4 AS text)::numeric,
                'USD',
                CAST($5 AS text)::numeric,
                CAST($6 AS text)::numeric,
                'completada'
            )
            "#,
            insert.buyer_id,
            insert.creator_id,
            insert.sample_id,
            amount,
            creator_amount,
            platform_fee,
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn has_pending_creator_payout(
        pool: &PgPool,
        creator_id: i32,
    ) -> Result<bool, sqlx::Error> {
        let exists = sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM transacciones
                WHERE creador_id = $1
                  AND tipo = 'payout'
                  AND estado = 'pendiente'
            ) AS "exists!"
            "#,
            creator_id
        )
        .fetch_one(pool)
        .await?;

        Ok(exists)
    }

    pub async fn insert_creator_payout(
        pool: &PgPool,
        insert: &CreatorPayoutInsert,
    ) -> Result<(), sqlx::Error> {
        let amount = cents_to_decimal_string(insert.amount_cents);
        let currency = insert.currency.to_ascii_uppercase();
        let idempotency_key = format!("stripe-payout:{}", insert.stripe_payout_id);

        sqlx::query!(
            r"
            INSERT INTO transacciones (
                comprador_id,
                creador_id,
                sample_id,
                tipo,
                monto,
                moneda,
                pago_creador,
                comision_plataforma,
                estado,
                stripe_payment_id,
                idempotency_key
            )
            VALUES (
                $1,
                $1,
                NULL,
                'payout',
                CAST($2 AS text)::numeric,
                $3,
                0,
                0,
                $4,
                $5,
                $6
            )
            ON CONFLICT DO NOTHING
            ",
            insert.creator_id,
            amount,
            currency,
            &insert.status,
            &insert.stripe_payout_id,
            idempotency_key
        )
        .execute(pool)
        .await?;

        Ok(())
    }
}

fn cents_to_decimal_string(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let cents = cents.abs();
    let units = cents / 100;
    let fractional = cents % 100;
    format!("{sign}{units}.{fractional:02}")
}
