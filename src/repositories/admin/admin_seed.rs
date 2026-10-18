use sqlx::PgPool;

pub struct AdminSeedRepository;

pub struct InsertSeedUserInput<'a> {
    pub wp_user_id: i64,
    pub username: &'a str,
    pub email: &'a str,
    pub nombre_visible: &'a str,
}

impl AdminSeedRepository {
    pub async fn count_relations(pool: &PgPool) -> Result<i64, sqlx::Error> {
        let row = sqlx::query!("SELECT COUNT(*) AS total FROM relaciones_sample")
            .fetch_one(pool)
            .await?;
        Ok(row.total.unwrap_or(0))
    }

    pub async fn count_seed_users(pool: &PgPool) -> Result<i64, sqlx::Error> {
        let row = sqlx::query!("SELECT COUNT(*) AS total FROM usuarios_ext WHERE es_seed = TRUE")
            .fetch_one(pool)
            .await?;
        Ok(row.total.unwrap_or(0))
    }

    pub async fn username_exists(pool: &PgPool, username: &str) -> Result<bool, sqlx::Error> {
        let row = sqlx::query!(
            "SELECT EXISTS(SELECT 1 FROM usuarios_ext WHERE username = $1) AS exists",
            username
        )
        .fetch_one(pool)
        .await?;
        Ok(row.exists.unwrap_or(false))
    }

    pub async fn insert_seed_user(
        pool: &PgPool,
        input: InsertSeedUserInput<'_>,
    ) -> Result<bool, sqlx::Error> {
        let row = sqlx::query!(
            r#"INSERT INTO usuarios_ext (
                    wp_user_id, username, email, nombre_visible, es_seed, rol, estado
                )
                VALUES ($1, $2, $3, $4, TRUE, 'usuario', 'activo')
                ON CONFLICT (username) DO NOTHING
                RETURNING id"#,
            input.wp_user_id,
            input.username,
            input.email,
            input.nombre_visible
        )
        .fetch_optional(pool)
        .await?;
        Ok(row.is_some())
    }

    pub async fn list_seed_user_ids(pool: &PgPool) -> Result<Vec<i32>, sqlx::Error> {
        let rows = sqlx::query!("SELECT id FROM usuarios_ext WHERE es_seed = TRUE ORDER BY id")
            .fetch_all(pool)
            .await?;
        Ok(rows.into_iter().map(|row| row.id).collect())
    }

    pub async fn list_relation_ids_without_contributor(
        pool: &PgPool,
    ) -> Result<Vec<i32>, sqlx::Error> {
        let rows = sqlx::query!(
            "SELECT id FROM relaciones_sample WHERE contribuidor_id IS NULL ORDER BY id"
        )
        .fetch_all(pool)
        .await?;
        Ok(rows.into_iter().map(|row| row.id).collect())
    }

    pub async fn assign_relation_contributor(
        pool: &PgPool,
        relation_id: i32,
        seed_user_id: i32,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            r#"UPDATE relaciones_sample
               SET contribuidor_id = $1
               WHERE id = $2 AND contribuidor_id IS NULL"#,
            seed_user_id,
            relation_id
        )
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn reassign_seed_samples(
        pool: &PgPool,
        system_user_id: i32,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!(
            r#"UPDATE samples s
               SET creador_id = r.contribuidor_id
               FROM relaciones_sample r
               WHERE (r.sample_fuente_id = s.id OR r.sample_destino_id = s.id)
                 AND r.contribuidor_id IS NOT NULL
                 AND s.cancion_origen_id IS NOT NULL
                 AND s.creador_id = $1"#,
            system_user_id
        )
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }
}
