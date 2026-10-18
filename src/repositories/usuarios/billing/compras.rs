use sqlx::PgPool;

use super::{
    cents_to_decimal_string, BillingRepository, CompletedDownloadRevenueShareInsert,
    CompletedSamplePurchaseInsert, CreatorPayoutInsert, SampleCheckoutCandidate,
};

impl BillingRepository {
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
