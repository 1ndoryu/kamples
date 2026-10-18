use super::*;
use serde_json::json;

fn sample_fixture() -> AudioIaSample {
    AudioIaSample {
        id: 77,
        id_corto: "abc1234".to_owned(),
        titulo: "warm loop".to_owned(),
        descripcion: "original user description".to_owned(),
        tags: vec!["house".to_owned()],
        bpm: Some(124),
        music_key: Some("A".to_owned()),
        scale: Some("minor".to_owned()),
        duration_seconds: 8.0,
        metadata: json!({
            "origen_subida": "Packs/House/Drums",
            "carpeta_primaria": "Drums",
            "carpeta_secundaria": "Kicks"
        }),
        estado: "activo".to_owned(),
    }
}

fn metadata_fixture() -> AudioCreativeMetadata {
    AudioCreativeMetadata {
        nombre_archivo_base: "deep house kick".to_owned(),
        tags: vec!["warm".to_owned()],
        tags_es: vec!["calido".to_owned()],
        tipo: "loop".to_owned(),
        genero: vec!["house".to_owned()],
        emocion: vec!["uplifting".to_owned()],
        emocion_es: vec!["euforico".to_owned()],
        instrumentos: vec!["drums".to_owned()],
        artista_vibes: vec!["KAYTRANADA".to_owned()],
        descripcion_corta: "short desc".to_owned(),
        descripcion_corta_es: "descripcion corta".to_owned(),
        descripcion: "long description".to_owned(),
        descripcion_es: "descripcion larga".to_owned(),
        carpeta_primaria: "Loops".to_owned(),
        carpeta_secundaria: "House".to_owned(),
    }
}

#[test]
fn generated_title_appends_bpm_and_minor_key() {
    let title = build_generated_title(&sample_fixture(), &metadata_fixture())
        .expect("title should be generated");

    assert_eq!(title, "Deep House Kick 124bpm Am");
}

#[test]
fn prepared_update_preserves_existing_manual_folders() {
    let sample = sample_fixture();
    let analysis_result = AudioIaAnalysisResult {
        metadata: metadata_fixture(),
        provider: AudioIaProvider::Groq,
        model: "openai/gpt-oss-120b".to_owned(),
        attempt_count: 1,
        provider_key_index: Some(0),
    };

    let update = build_prepared_update(&sample, &analysis_result);

    assert_eq!(update.titulo.as_deref(), Some("Deep House Kick 124bpm Am"));
    assert_eq!(
        update.slug.as_deref(),
        Some("deep-house-kick-124bpm-am-abc1234")
    );
    assert_eq!(update.descripcion.as_deref(), Some("descripcion corta"));
    assert_eq!(update.metadata["carpeta_primaria"], json!("Drums"));
    assert_eq!(update.metadata["carpeta_secundaria"], json!("Kicks"));
    assert_eq!(update.metadata["ia_carpeta_primaria"], json!("Loops"));
    assert_eq!(update.metadata["ia_carpeta_secundaria"], json!("House"));
    assert_eq!(update.metadata["ia_pending"], json!(false));
}

#[test]
fn extraction_context_builds_from_metadata_block() {
    let metadata = json!({
        "origen_subida": "Extracciones",
        "extraccion": {
            "fuente_titulo": "Strings of Life",
            "fuente_artista": "Derrick May",
            "destino_titulo": "Inner City",
            "destino_artista": "Big Fun",
            "tipo_elemento": "drums",
            "votos_total": 17,
        }
    });

    let context = build_extraction_context(&metadata).expect("context should be built");

    assert_eq!(context.source_title.as_deref(), Some("Strings of Life"));
    assert_eq!(context.source_artist.as_deref(), Some("Derrick May"));
    assert_eq!(context.destination_title.as_deref(), Some("Inner City"));
    assert_eq!(context.destination_artist.as_deref(), Some("Big Fun"));
    assert_eq!(context.element_type.as_deref(), Some("drums"));
    assert_eq!(context.vote_count, Some(17));
}

#[test]
fn extraction_context_is_none_without_extraccion_block() {
    let metadata = json!({ "origen_subida": "Packs/House" });
    assert!(build_extraction_context(&metadata).is_none());
}

#[test]
fn extraction_context_is_none_when_block_lacks_useful_fields() {
    let metadata = json!({ "extraccion": { "votos_total": 5 } });
    assert!(build_extraction_context(&metadata).is_none());
}
