/* [174A-98] Sitemap XML dinámico + robots.txt.
 *
 * Reemplaza al SeoSitemapProvider PHP del legado. Genera un único sitemap.xml
 * con URLs de samples activos, canciones, artistas y perfiles activos.
 *
 * Para sites con más de 50K URLs (límite Google) hay que migrar a
 * sitemapindex con sub-sitemaps paginados  por ahora, LIMIT 10K por
 * categoría es suficiente y cabe holgado en el límite individual.
 *
 * El base_url se toma de `AppConfig::public_base_url`. */

use std::fmt::Write;

use axum::extract::{Query, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Json;
use axum::Router;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;

use crate::repositories::SeoRepository;
use crate::AppState;

const SITEMAP_LIMIT: i64 = 10_000;

pub async fn sitemap(State(state): State<AppState>) -> impl IntoResponse {
    let pool = &state.pool;
    let base = state
        .public_base_url
        .as_deref()
        .unwrap_or("")
        .trim_end_matches('/')
        .to_string();

    let mut body = String::with_capacity(64 * 1024);
    body.push_str(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
"#,
    );

    write_static_urls(&mut body, &base);
    append_sample_urls(&mut body, &base, pool).await;
    append_song_urls(&mut body, &base, pool).await;
    append_artist_urls(&mut body, &base, pool).await;
    append_profile_urls(&mut body, &base, pool).await;
    append_blog_urls(&mut body, &base, pool).await;

    body.push_str("</urlset>\n");

    (
        [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
        body,
    )
}

fn write_static_urls(body: &mut String, base: &str) {
    write_url(body, &format!("{base}/"), None, "daily", "1.0");
    write_url(body, &format!("{base}/explorar"), None, "daily", "0.9");
    write_url(body, &format!("{base}/blog"), None, "weekly", "0.7");
}

async fn append_sample_urls(body: &mut String, base: &str, pool: &PgPool) {
    if let Ok(rows) = SeoRepository::sitemap_samples(pool, SITEMAP_LIMIT).await {
        for row in rows {
            let lastmod: Option<String> = row
                .updated_at
                .map(|t: chrono::DateTime<chrono::Utc>| t.to_rfc3339());
            write_url(
                body,
                &format!("{base}/sample/{}", row.slug),
                lastmod.as_deref(),
                "weekly",
                "0.8",
            );
        }
    }
}

async fn append_song_urls(body: &mut String, base: &str, pool: &PgPool) {
    if let Ok(rows) = SeoRepository::sitemap_songs(pool, SITEMAP_LIMIT).await {
        for row in rows {
            write_url(
                body,
                &format!("{base}/cancion/{}", row.slug),
                None,
                "monthly",
                "0.6",
            );
        }
    }
}

async fn append_artist_urls(body: &mut String, base: &str, pool: &PgPool) {
    if let Ok(rows) = SeoRepository::sitemap_artists(pool, SITEMAP_LIMIT).await {
        for row in rows {
            write_url(
                body,
                &format!("{base}/artista/{}", row.slug),
                None,
                "monthly",
                "0.6",
            );
        }
    }
}

async fn append_profile_urls(body: &mut String, base: &str, pool: &PgPool) {
    if let Ok(rows) = SeoRepository::sitemap_profiles(pool, SITEMAP_LIMIT).await {
        for row in rows {
            let lastmod: Option<String> = row
                .updated_at
                .map(|t: chrono::DateTime<chrono::Utc>| t.to_rfc3339());
            write_url(
                body,
                &format!("{base}/perfil/{}", row.username),
                lastmod.as_deref(),
                "weekly",
                "0.7",
            );
        }
    }
}

async fn append_blog_urls(body: &mut String, base: &str, pool: &PgPool) {
    if let Ok(rows) = SeoRepository::sitemap_blog(pool, SITEMAP_LIMIT).await {
        for row in rows {
            let lastmod: Option<String> = Some(row.updated_at.to_rfc3339());
            write_url(
                body,
                &format!("{base}/blog/{}", row.slug),
                lastmod.as_deref(),
                "monthly",
                "0.6",
            );
        }
    }
}

fn write_url(
    body: &mut String,
    loc: &str,
    lastmod: Option<&str>,
    changefreq: &str,
    priority: &str,
) {
    let _ = writeln!(body, "  <url>");
    let _ = writeln!(body, "    <loc>{}</loc>", xml_escape(loc));
    if let Some(lm) = lastmod {
        let _ = writeln!(body, "    <lastmod>{lm}</lastmod>");
    }
    let _ = writeln!(body, "    <changefreq>{changefreq}</changefreq>");
    let _ = writeln!(body, "    <priority>{priority}</priority>");
    let _ = writeln!(body, "  </url>");
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub async fn robots(State(state): State<AppState>) -> impl IntoResponse {
    let base = state
        .public_base_url
        .as_deref()
        .unwrap_or("")
        .trim_end_matches('/')
        .to_string();
    let body = format!(
        "User-agent: *\nAllow: /\nDisallow: /api/\nDisallow: /admin/\n\nSitemap: {base}/sitemap.xml\n"
    );
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], body)
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/sitemap.xml", get(sitemap))
        .route("/robots.txt", get(robots))
}

/* [174A-99] Endpoint metadata SEO para SPA.
 *
 * GET /api/seo/metadata?path=/sample/{slug}
 * GET /api/seo/metadata?path=/perfil/{username}
 * GET /api/seo/metadata?path=/blog/{slug}
 *
 * Devuelve JSON con title, description, og_image, canonical y json_ld.
 * El SPA inyecta esto en <head> al hacer client-side routing.
 *
 * Reemplaza al SeoKamples PHP del legado, simplificado al subset usable
 * desde React (sin schema markup completo de MusicRecording por ahora). */

#[derive(Debug, Deserialize)]
pub struct MetadataQuery {
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct SeoMetadata {
    pub title: String,
    pub description: String,
    pub canonical: String,
    pub og_image: Option<String>,
    pub og_type: String,
    pub robots: String,
    pub json_ld: Option<Value>,
}

pub async fn metadata(
    State(state): State<AppState>,
    Query(q): Query<MetadataQuery>,
) -> Result<Json<SeoMetadata>, StatusCode> {
    let pool = &state.pool;
    let base = state
        .public_base_url
        .as_deref()
        .unwrap_or("")
        .trim_end_matches('/')
        .to_string();
    let path = q.path.trim();
    let canonical = format!("{base}{path}");

    /* /sample/{slug} */
    if let Some(slug) = path.strip_prefix("/sample/") {
        let slug = slug.trim_end_matches('/');
        if let Ok(Some(row)) = SeoRepository::sample_metadata(pool, slug).await {
            let title = format!("{} | Sample gratis para descargar", row.titulo);
            let description = row
                .descripcion
                .unwrap_or_else(|| format!("Descarga el sample {} en alta calidad.", row.titulo));
            return Ok(Json(SeoMetadata {
                title,
                description: description.chars().take(160).collect(),
                canonical: canonical.clone(),
                og_image: None,
                og_type: "music.song".into(),
                robots: "index,follow".into(),
                json_ld: Some(json!({
                    "@context": "https://schema.org",
                    "@type": "MusicRecording",
                    "name": row.titulo,
                    "url": canonical,
                })),
            }));
        }
    }

    /* /perfil/{username} */
    if let Some(username) = path.strip_prefix("/perfil/") {
        let username = username.trim_end_matches('/');
        if let Ok(Some(row)) = SeoRepository::profile_metadata(pool, username).await {
            let display = row.nombre_visible;
            let bio = row.bio.unwrap_or_default();
            let title = format!("{display} | Perfil en Kamples");
            let description = if bio.is_empty() {
                format!("Descubre los samples de {display} en Kamples.")
            } else {
                bio
            };
            return Ok(Json(SeoMetadata {
                title,
                description: description.chars().take(160).collect(),
                canonical: canonical.clone(),
                og_image: row.avatar_url.clone(),
                og_type: "profile".into(),
                robots: "index,follow".into(),
                json_ld: Some(json!({
                    "@context": "https://schema.org",
                    "@type": "Person",
                    "name": display,
                    "url": canonical,
                    "image": row.avatar_url,
                })),
            }));
        }
    }

    /* /blog/{slug} */
    if let Some(slug) = path.strip_prefix("/blog/") {
        let slug = slug.trim_end_matches('/');
        if let Ok(Some(row)) = SeoRepository::article_metadata(pool, slug).await {
            let title = format!("{} | Blog Kamples", row.titulo);
            let extracto = row.extracto;
            let description = if extracto.is_empty() {
                row.titulo.clone()
            } else {
                extracto
            };
            return Ok(Json(SeoMetadata {
                title,
                description: description.chars().take(160).collect(),
                canonical: canonical.clone(),
                og_image: None,
                og_type: "article".into(),
                robots: "index,follow".into(),
                json_ld: Some(json!({
                    "@context": "https://schema.org",
                    "@type": "Article",
                    "headline": row.titulo,
                    "url": canonical,
                })),
            }));
        }
    }

    /* Fallback genérico para rutas desconocidas. */
    Ok(Json(SeoMetadata {
        title: "Kamples | Samples gratis y herramientas para productores".into(),
        description: "Descarga samples, packs y conecta con productores en Kamples.".into(),
        canonical,
        og_image: None,
        og_type: "website".into(),
        robots: "index,follow".into(),
        json_ld: None,
    }))
}

pub fn routes_api() -> Router<AppState> {
    Router::new().route("/seo/metadata", get(metadata))
}
