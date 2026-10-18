use sqlx::{FromRow, Postgres, QueryBuilder};

#[derive(Debug, FromRow)]
pub(super) struct CountRow {
    pub(super) total: i64,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum UserOrder {
    Fecha,
    Actividad,
    Samples,
}

pub(super) fn push_user_filters<'args>(
    builder: &mut QueryBuilder<'args, Postgres>,
    search_like: Option<&'args str>,
    plan: Option<&'static str>,
) {
    builder.push(" WHERE 1=1");

    if let Some(search_like) = search_like {
        builder.push(" AND (");
        builder.push("u.username ILIKE ");
        builder.push_bind(search_like);
        builder.push(" OR u.nombre_visible ILIKE ");
        builder.push_bind(search_like);
        builder.push(" OR u.email ILIKE ");
        builder.push_bind(search_like);
        builder.push(")");
    }

    if let Some(plan) = plan {
        builder.push(" AND u.plan = ");
        builder.push_bind(plan);
    }
}

pub(super) fn push_scraper_filters<'args>(
    builder: &mut QueryBuilder<'args, Postgres>,
    search_like: Option<&'args str>,
    state_filter: Option<&'static str>,
) {
    builder.push(" WHERE 1=1");

    if let Some(search_like) = search_like {
        builder.push(" AND (");
        builder.push("s.url ILIKE ");
        builder.push_bind(search_like);
        builder.push(" OR s.tipo_pagina ILIKE ");
        builder.push_bind(search_like);
        builder.push(" OR COALESCE(s.error_mensaje, '') ILIKE ");
        builder.push_bind(search_like);
        builder.push(")");
    }

    if let Some(state_filter) = state_filter {
        builder.push(" AND s.estado = ");
        builder.push_bind(state_filter);
    }
}

pub(super) fn push_extraction_filters<'args>(
    builder: &mut QueryBuilder<'args, Postgres>,
    search_like: Option<&'args str>,
    states: &[&'static str],
    sides: &[&'static str],
) {
    builder.push(" WHERE 1=1");

    if let Some(search_like) = search_like {
        builder.push(" AND (");
        builder.push("c.youtube_id ILIKE ");
        builder.push_bind(search_like);
        builder.push(" OR c.spotify_id ILIKE ");
        builder.push_bind(search_like);
        builder.push(" OR COALESCE(c.error_mensaje, '') ILIKE ");
        builder.push_bind(search_like);
        builder.push(")");
    }

    if !states.is_empty() {
        builder.push(" AND c.estado IN (");
        {
            let mut separated = builder.separated(", ");
            for state in states {
                separated.push_bind(*state);
            }
        }
        builder.push(")");
    }

    if !sides.is_empty() {
        builder.push(" AND c.lado IN (");
        {
            let mut separated = builder.separated(", ");
            for side in sides {
                separated.push_bind(*side);
            }
        }
        builder.push(")");
    }
}

pub(super) fn like_pattern(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("%{value}%"))
}

pub(super) fn normalize_user_order(raw: Option<&str>) -> UserOrder {
    match raw.map(str::trim).unwrap_or_default() {
        "actividad" => UserOrder::Actividad,
        "samples" => UserOrder::Samples,
        _ => UserOrder::Fecha,
    }
}

pub(super) fn normalize_user_plan_filter(raw: Option<&str>) -> Option<&'static str> {
    match raw.map(str::trim).unwrap_or_default() {
        "free" => Some("free"),
        "pro" => Some("pro"),
        "premium" => Some("premium"),
        _ => None,
    }
}

pub(super) fn normalize_user_plan_payload(raw: &str) -> Option<&'static str> {
    match raw.trim() {
        "free" => Some("free"),
        "pro" => Some("pro"),
        "premium" => Some("premium"),
        _ => None,
    }
}

pub(super) fn normalize_user_role_payload(raw: &str) -> Option<&'static str> {
    match raw.trim() {
        "usuario" => Some("usuario"),
        "creador" => Some("creador"),
        "admin" => Some("admin"),
        _ => None,
    }
}

pub(super) fn normalize_scraper_state_filter(raw: Option<&str>) -> Option<&'static str> {
    match raw
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "pending" | "pendiente" => Some("pendiente"),
        "scraped" | "procesado" => Some("procesado"),
        "error" => Some("error"),
        "skipped" | "skip" => Some("skip"),
        _ => None,
    }
}

pub(super) fn normalize_scraper_sort(raw: Option<&str>) -> &'static str {
    match raw.map(str::trim).unwrap_or_default() {
        "id" => "s.id",
        "url" => "s.url",
        "tipo_pagina" => "s.tipo_pagina",
        "estado" => "s.estado",
        "intentos" => "s.intentos",
        "bytes_descargados" => "s.bytes_descargados",
        "error_mensaje" => "s.error_mensaje",
        "re_scrapeable" => "s.re_scrapeable",
        "veces_rescrapeado" => "s.veces_rescrapeado",
        "procesado_at" => "s.procesado_at",
        _ => "s.created_at",
    }
}

pub(super) fn normalize_queue_state_filter(raw: Option<&str>) -> Vec<&'static str> {
    split_csv(raw)
        .into_iter()
        .filter_map(|value| match value {
            "pendiente" => Some("pendiente"),
            "descargando" => Some("descargando"),
            "analizando" => Some("analizando"),
            "recortando" => Some("recortando"),
            "extraido" => Some("extraido"),
            "completado" => Some("completado"),
            "error" => Some("error"),
            "revision_humana" => Some("revision_humana"),
            "unificado" => Some("unificado"),
            _ => None,
        })
        .collect()
}

pub(super) fn normalize_queue_side_filter(raw: Option<&str>) -> Vec<&'static str> {
    split_csv(raw)
        .into_iter()
        .filter_map(|value| match value {
            "fuente" => Some("fuente"),
            "destino" => Some("destino"),
            _ => None,
        })
        .collect()
}

pub(super) fn normalize_extraction_sort(raw: Option<&str>) -> &'static str {
    match raw.map(str::trim).unwrap_or_default() {
        "id" => "c.id",
        "relacion_id" => "c.relacion_id",
        "youtube_id" => "c.youtube_id",
        "spotify_id" => "c.spotify_id",
        "estado" => "c.estado",
        "intentos" => "c.intentos",
        "lado" => "c.lado",
        "sample_id" => "c.sample_id",
        "timing_inicio_seg" => "c.timing_inicio_seg",
        "bpm_detectado" => "c.bpm_detectado",
        "error_mensaje" => "c.error_mensaje",
        "procesado_at" => "c.procesado_at",
        "proximo_intento_at" => "c.proximo_intento_at",
        _ => "c.created_at",
    }
}

pub(super) fn normalize_sort_dir(raw: Option<&str>) -> &'static str {
    if raw
        .map(str::trim)
        .is_some_and(|value| value.eq_ignore_ascii_case("ASC"))
    {
        "ASC"
    } else {
        "DESC"
    }
}

fn split_csv(raw: Option<&str>) -> Vec<&str> {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .collect()
        })
        .unwrap_or_default()
}
