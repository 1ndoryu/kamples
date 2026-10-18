use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;

/* [174A-42] Cola separada para metadata creativa.
 * Se desacopla del pipeline técnico porque `cola_procesamiento_ia/analisis_audio`
 * ya quedó ocupada por DSP + embedding en Rust. Aquí solo vive scheduling IA. */

pub struct IaQueueRepository;

#[derive(Debug, Clone)]
pub struct QueuedIaJob {
    pub id: i32,
    pub sample_id: i32,
    pub attempts: i32,
    pub max_attempts: i32,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IaQueueFailureDisposition {
    RetryScheduled,
    FinalError,
}

impl IaQueueRepository {
    pub async fn enqueue_sample_analysis(
        pool: &PgPool,
        sample_id: i32,
        metadata: serde_json::Value,
    ) -> Result<bool, sqlx::Error> {
        let inserted = sqlx::query!(
            "INSERT INTO ia_queue (sample_id, metadata)
             VALUES ($1, $2)
             ON CONFLICT DO NOTHING",
            sample_id,
            metadata,
        )
        .execute(pool)
        .await?;

        Ok(inserted.rows_affected() > 0)
    }

    pub async fn claim_next_pending(pool: &PgPool) -> Result<Option<QueuedIaJob>, sqlx::Error> {
        sqlx::query_as!(
            QueuedIaJob,
            "WITH picked AS (
                SELECT id
                FROM ia_queue
                WHERE status = 'pending'
                   OR (status = 'retry_scheduled' AND (next_retry_at IS NULL OR next_retry_at <= NOW()))
                ORDER BY created_at ASC
                FOR UPDATE SKIP LOCKED
                LIMIT 1
            )
            UPDATE ia_queue AS queue
            SET status = 'processing',
                updated_at = NOW()
            FROM picked
            WHERE queue.id = picked.id
            RETURNING
                queue.id,
                queue.sample_id,
                queue.attempts,
                queue.max_attempts,
                queue.metadata as \"metadata!: serde_json::Value\",
                queue.created_at as \"created_at!: DateTime<Utc>\"",
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn mark_completed(pool: &PgPool, job_id: i32) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE ia_queue
             SET status = 'completed',
                 last_error = NULL,
                 next_retry_at = NULL,
                 processed_at = NOW(),
                 updated_at = NOW()
             WHERE id = $1",
            job_id,
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn mark_failed(
        pool: &PgPool,
        job: &QueuedIaJob,
        error_message: &str,
        retryable: bool,
        retry_after_seconds: Option<f32>,
    ) -> Result<IaQueueFailureDisposition, sqlx::Error> {
        let next_attempt = job.attempts + 1;
        let is_final = !retryable || next_attempt >= job.max_attempts;
        let disposition = if is_final {
            IaQueueFailureDisposition::FinalError
        } else {
            IaQueueFailureDisposition::RetryScheduled
        };
        let next_retry_at = if is_final {
            None
        } else {
            Some(Utc::now() + retry_backoff_duration(next_attempt, retry_after_seconds))
        };
        let next_status = if is_final {
            "failed"
        } else {
            "retry_scheduled"
        };
        let processed_at = if is_final { Some(Utc::now()) } else { None };

        sqlx::query!(
            "UPDATE ia_queue
             SET status = $2,
                 attempts = $3,
                 last_error = $4,
                 next_retry_at = $5,
                 processed_at = COALESCE($6, processed_at),
                 updated_at = NOW()
             WHERE id = $1",
            job.id,
            next_status,
            next_attempt,
            error_message,
            next_retry_at,
            processed_at,
        )
        .execute(pool)
        .await?;

        Ok(disposition)
    }
}

#[must_use]
pub fn retry_backoff_duration(attempt: i32, retry_after_seconds: Option<f32>) -> Duration {
    let exponential_seconds = match attempt.max(1) {
        1 => 60,
        2 => 180,
        3 => 600,
        _ => 1_800,
    };
    let provider_seconds = retry_after_seconds
        .filter(|value| value.is_finite() && *value > 0.0)
        .map_or(0, ceil_f32_seconds_to_i64);

    Duration::seconds(exponential_seconds.max(provider_seconds))
}

fn ceil_f32_seconds_to_i64(value: f32) -> i64 {
    value.ceil().to_string().parse::<i64>().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_respects_provider_retry_after_without_shortening_exponential() {
        assert_eq!(retry_backoff_duration(1, None), Duration::seconds(60));
        assert_eq!(
            retry_backoff_duration(2, Some(12.0)),
            Duration::seconds(180)
        );
        assert_eq!(
            retry_backoff_duration(3, Some(900.0)),
            Duration::seconds(900)
        );
        assert_eq!(
            retry_backoff_duration(8, Some(5.0)),
            Duration::seconds(1_800)
        );
    }
}
