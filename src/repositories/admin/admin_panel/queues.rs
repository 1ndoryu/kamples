use std::collections::BTreeMap;

use sqlx::{PgPool, Postgres, QueryBuilder};

use super::shared::{
    like_pattern, normalize_extraction_sort, normalize_queue_side_filter,
    normalize_queue_state_filter, normalize_scraper_sort, normalize_scraper_state_filter,
    normalize_sort_dir, push_extraction_filters, push_scraper_filters, CountRow,
};
use super::AdminPanelRepository;
use crate::errors::AppError;
use crate::models::{
    AdminExtractionQueueItem, AdminExtractionQueueQuery, AdminExtractionQueueResponse,
    AdminScraperItem, AdminScrapersQuery, AdminScrapersResponse,
};

impl AdminPanelRepository {
    pub async fn list_scrapers(
        pool: &PgPool,
        query: &AdminScrapersQuery,
    ) -> Result<AdminScrapersResponse, AppError> {
        let page = query.page.unwrap_or(1).max(1);
        let per_page = 25_i64;
        let offset = (page - 1) * per_page;
        let search_like = like_pattern(query.busqueda.as_deref());
        let state_filter = normalize_scraper_state_filter(query.estado.as_deref());
        let sort_column = normalize_scraper_sort(query.sort_col.as_deref());
        let sort_dir = normalize_sort_dir(query.sort_dir.as_deref());

        Ok(AdminScrapersResponse {
            data: fetch_scrapers(
                pool,
                search_like.as_deref(),
                state_filter,
                sort_column,
                sort_dir,
                per_page,
                offset,
            )
            .await?,
            total: count_scrapers(pool, search_like.as_deref(), state_filter).await?,
            page,
            estados_cuenta: scraper_state_counts(pool).await?,
        })
    }

    pub async fn list_extraction_queue(
        pool: &PgPool,
        query: &AdminExtractionQueueQuery,
    ) -> Result<AdminExtractionQueueResponse, AppError> {
        let page = query.page.unwrap_or(1).max(1);
        let per_page = 25_i64;
        let offset = (page - 1) * per_page;
        let search_like = like_pattern(query.busqueda.as_deref());
        let states = normalize_queue_state_filter(query.estado.as_deref());
        let sides = normalize_queue_side_filter(query.lado.as_deref());
        let sort_column = normalize_extraction_sort(query.sort_col.as_deref());
        let sort_dir = normalize_sort_dir(query.sort_dir.as_deref());

        Ok(AdminExtractionQueueResponse {
            data: fetch_extraction_queue(
                pool,
                PaginaCola {
                    search_like: search_like.as_deref(),
                    states: &states,
                    sides: &sides,
                    sort_column,
                    sort_dir,
                    limit: per_page,
                    offset,
                },
            )
            .await?,
            total: count_extraction_queue(pool, search_like.as_deref(), &states, &sides).await?,
            page,
            estados_cuenta: extraction_queue_state_counts(pool).await?,
        })
    }
}

async fn count_scrapers(
    pool: &PgPool,
    search_like: Option<&str>,
    state_filter: Option<&'static str>,
) -> Result<i64, AppError> {
    let mut builder = QueryBuilder::<Postgres>::new("SELECT COUNT(*) AS total FROM scraping_log s");
    push_scraper_filters(&mut builder, search_like, state_filter);
    let row = builder.build_query_as::<CountRow>().fetch_one(pool).await?;
    Ok(row.total)
}

async fn fetch_scrapers(
    pool: &PgPool,
    search_like: Option<&str>,
    state_filter: Option<&'static str>,
    sort_column: &'static str,
    sort_dir: &'static str,
    limit: i64,
    offset: i64,
) -> Result<Vec<AdminScraperItem>, AppError> {
    let mut builder = QueryBuilder::<Postgres>::new(
        r"SELECT
                s.id,
                s.url,
                s.tipo_pagina,
                CASE s.estado
                    WHEN 'pendiente' THEN 'pending'
                    WHEN 'procesado' THEN 'scraped'
                    WHEN 'skip' THEN 'skipped'
                    ELSE s.estado
                END AS estado,
                s.intentos::int4 AS intentos,
                s.bytes_descargados::int8 AS bytes_descargados,
                s.error_mensaje,
                COALESCE(s.re_scrapeable, FALSE) AS re_scrapeable,
                s.veces_rescrapeado::int4 AS veces_rescrapeado,
                s.procesado_at,
                s.created_at
           FROM scraping_log s",
    );

    push_scraper_filters(&mut builder, search_like, state_filter);
    builder.push(" ORDER BY ");
    builder.push(sort_column);
    builder.push(" ");
    builder.push(sort_dir);
    builder.push(", s.id DESC LIMIT ");
    builder.push_bind(limit);
    builder.push(" OFFSET ");
    builder.push_bind(offset);

    let rows = builder
        .build_query_as::<AdminScraperItem>()
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

async fn scraper_state_counts(pool: &PgPool) -> Result<BTreeMap<String, i64>, AppError> {
    let mut builder = QueryBuilder::<Postgres>::new(
        r"SELECT
                CASE estado
                    WHEN 'pendiente' THEN 'pending'
                    WHEN 'procesado' THEN 'scraped'
                    WHEN 'skip' THEN 'skipped'
                    ELSE estado
                END AS estado,
                COUNT(*)::bigint AS total
           FROM scraping_log
           GROUP BY estado
           ORDER BY total DESC, estado ASC",
    );

    let rows = builder
        .build_query_as::<AdminStateCountRow>()
        .fetch_all(pool)
        .await?;

    Ok(rows
        .into_iter()
        .map(|row| (row.estado, row.total))
        .collect())
}

async fn count_extraction_queue(
    pool: &PgPool,
    search_like: Option<&str>,
    states: &[&'static str],
    sides: &[&'static str],
) -> Result<i64, AppError> {
    let mut builder =
        QueryBuilder::<Postgres>::new("SELECT COUNT(*) AS total FROM cola_extraccion_samples c");
    push_extraction_filters(&mut builder, search_like, states, sides);
    let row = builder.build_query_as::<CountRow>().fetch_one(pool).await?;
    Ok(row.total)
}

/// Filtros, orden y página de la cola de extracción: agrupa lo que `fetch_extraction_queue` necesita.
#[derive(Clone, Copy)]
struct PaginaCola<'a> {
    search_like: Option<&'a str>,
    states: &'a [&'static str],
    sides: &'a [&'static str],
    sort_column: &'static str,
    sort_dir: &'static str,
    limit: i64,
    offset: i64,
}

async fn fetch_extraction_queue(
    pool: &PgPool,
    pagina: PaginaCola<'_>,
) -> Result<Vec<AdminExtractionQueueItem>, AppError> {
    let PaginaCola {
        search_like,
        states,
        sides,
        sort_column,
        sort_dir,
        limit,
        offset,
    } = pagina;
    let mut builder = QueryBuilder::<Postgres>::new(
        r"SELECT
                c.id,
                c.relacion_id,
                c.youtube_id,
                c.spotify_id,
                c.estado,
                c.intentos::int4 AS intentos,
                c.lado,
                c.error_mensaje,
                c.sample_id,
                c.timing_inicio_seg::int4 AS timing_inicio_seg,
                CAST(c.compas_inicio_seg AS double precision) AS compas_inicio_seg,
                CAST(c.compas_fin_seg AS double precision) AS compas_fin_seg,
                c.bpm_detectado::int4 AS bpm_detectado,
                c.procesado_at,
                c.created_at,
                c.proximo_intento_at
           FROM cola_extraccion_samples c",
    );

    push_extraction_filters(&mut builder, search_like, states, sides);
    builder.push(" ORDER BY ");
    builder.push(sort_column);
    builder.push(" ");
    builder.push(sort_dir);
    builder.push(", c.id DESC LIMIT ");
    builder.push_bind(limit);
    builder.push(" OFFSET ");
    builder.push_bind(offset);

    let rows = builder
        .build_query_as::<AdminExtractionQueueItem>()
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

async fn extraction_queue_state_counts(pool: &PgPool) -> Result<BTreeMap<String, i64>, AppError> {
    let mut builder = QueryBuilder::<Postgres>::new(
        r"SELECT estado, COUNT(*)::bigint AS total
           FROM cola_extraccion_samples
           GROUP BY estado
           ORDER BY total DESC, estado ASC",
    );

    let rows = builder
        .build_query_as::<AdminStateCountRow>()
        .fetch_all(pool)
        .await?;

    Ok(rows
        .into_iter()
        .map(|row| (row.estado, row.total))
        .collect())
}

#[derive(Debug, sqlx::FromRow)]
struct AdminStateCountRow {
    estado: String,
    total: i64,
}
