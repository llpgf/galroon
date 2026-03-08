//! Extended features — multi-root, completion, gap analysis, import queue, plugins, i18n, translate.

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row};
use std::path::PathBuf;
use tauri::State;

use crate::config::{AiProviderConfig, SharedConfig};
use crate::db::Database;
use crate::domain::error::AppError;

// ═══════════════════════════════════════════════
// 1. Multi-Root Management
// ═══════════════════════════════════════════════

#[tauri::command]
pub async fn list_library_roots(config: State<'_, SharedConfig>) -> Result<Vec<String>, AppError> {
    let cfg = config.read().await;
    Ok(cfg
        .library_roots
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

#[tauri::command]
pub async fn add_library_root(
    config: State<'_, SharedConfig>,
    path: String,
) -> Result<(), AppError> {
    let pb = PathBuf::from(&path);
    config
        .update(|cfg| {
            if !cfg.library_roots.contains(&pb) {
                cfg.library_roots.push(pb);
            }
        })
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn remove_library_root(
    config: State<'_, SharedConfig>,
    path: String,
) -> Result<(), AppError> {
    let pb = PathBuf::from(&path);
    config
        .update(|cfg| {
            cfg.library_roots.retain(|r| r != &pb);
        })
        .await?;
    Ok(())
}

// ═══════════════════════════════════════════════
// 2. Completion Tracking
// ═══════════════════════════════════════════════

#[derive(Serialize, Deserialize, FromRow)]
pub struct CompletionInfo {
    pub work_id: String,
    pub status: String,
    pub progress_pct: i32,
    pub playtime_min: i32,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub notes: String,
}

#[tauri::command]
pub async fn get_completion(
    db: State<'_, Database>,
    work_id: String,
) -> Result<Option<CompletionInfo>, AppError> {
    let row: Option<CompletionInfo> = sqlx::query_as(
        "SELECT work_id, status, progress_pct, playtime_min, started_at, completed_at, notes \
         FROM completion_tracking WHERE work_id = ?",
    )
    .bind(&work_id)
    .fetch_optional(db.read_pool())
    .await?;
    Ok(row)
}

#[tauri::command]
pub async fn update_completion(
    db: State<'_, Database>,
    work_id: String,
    status: String,
    progress_pct: Option<i32>,
    playtime_min: Option<i32>,
    notes: Option<String>,
) -> Result<(), AppError> {
    let allowed_status = [
        "not_started",
        "in_progress",
        "completed",
        "on_hold",
        "dropped",
    ];
    if !allowed_status.contains(&status.as_str()) {
        return Err(AppError::Validation(format!(
            "Unsupported completion status: {}",
            status
        )));
    }

    let pct = progress_pct.unwrap_or(0).clamp(0, 100);
    let time = playtime_min.unwrap_or(0).max(0);
    let n = notes.unwrap_or_default();

    let started = if status == "in_progress" || status == "completed" {
        "COALESCE(started_at, datetime('now'))"
    } else {
        "started_at"
    };
    let completed = if status == "completed" {
        "datetime('now')"
    } else {
        "NULL"
    };

    db.execute_write(
        format!(
            "INSERT INTO completion_tracking (work_id, status, progress_pct, playtime_min, notes, started_at, completed_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'), {completed}) \
             ON CONFLICT(work_id) DO UPDATE SET \
             status = ?2, progress_pct = ?3, playtime_min = ?4, notes = ?5, \
             started_at = {started}, completed_at = {completed}, updated_at = datetime('now')"
        ),
        vec![
            serde_json::Value::String(work_id),
            serde_json::Value::String(status),
            serde_json::Value::Number(serde_json::Number::from(pct as i64)),
            serde_json::Value::Number(serde_json::Number::from(time as i64)),
            serde_json::Value::String(n),
        ],
    )
    .await?;
    Ok(())
}

// ═══════════════════════════════════════════════
// 3. Gap Analysis
// ═══════════════════════════════════════════════

#[derive(Serialize)]
pub struct GapReport {
    pub brands_without_all_works: Vec<BrandGap>,
    pub works_missing_assets: Vec<AssetGap>,
    pub enrichment_diagnostics: Vec<EnrichmentDiagnostic>,
    pub ignored_diagnostics: Vec<EnrichmentDiagnostic>,
    pub unmatched_works: i64,
    pub works_without_cover: i64,
    pub posters_with_variants: i64,
}

#[derive(Serialize)]
pub struct BrandGap {
    pub brand: String,
    pub owned: i64,
    pub total_known: i64,
}

#[derive(Serialize)]
pub struct AssetGap {
    pub work_id: String,
    pub title: String,
    pub missing: Vec<String>,
    pub asset_types: Vec<String>,
    pub asset_count: i64,
}

#[derive(Serialize)]
pub struct EnrichmentDiagnostic {
    pub work_id: String,
    pub title: String,
    pub severity: String,
    pub category: String,
    pub reason: String,
    pub suggested_action: String,
    pub details: Vec<String>,
    pub linked_sources: Vec<String>,
    pub preferred_field: Option<String>,
    pub preferred_source: Option<String>,
}

#[tauri::command]
pub async fn get_gap_analysis(db: State<'_, Database>) -> Result<GapReport, AppError> {
    let pool = db.read_pool();
    let ignored_rows = sqlx::query("SELECT work_id, category FROM workshop_ignored_diagnostics")
        .fetch_all(pool)
        .await?;
    let ignored_keys: std::collections::HashSet<(String, String)> = ignored_rows
        .into_iter()
        .map(|row| {
            (
                row.get::<String, _>("work_id"),
                row.get::<String, _>("category"),
            )
        })
        .collect();

    let (unmatched,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM canonical_works WHERE enrichment_state != 'matched'")
            .fetch_one(pool)
            .await?;

    let (no_cover,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM canonical_works WHERE cover_path IS NULL OR cover_path = ''",
    )
    .fetch_one(pool)
    .await?;

    let (posters_with_variants,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM canonical_works WHERE variant_count > 1")
            .fetch_one(pool)
            .await?;

    let poster_rows = sqlx::query(
        "SELECT
            cw.canonical_key,
            cw.preferred_work_id AS work_id,
            cw.title,
            cw.cover_path,
            cw.developer,
            cw.enrichment_state,
            cw.variant_count,
            cw.asset_count,
            cw.asset_types,
            cw.primary_asset_type,
            cw.vndb_id,
            cw.bangumi_id,
            cw.dlsite_id,
            pw.field_preferences,
            COALESCE((
                SELECT COUNT(*)
                FROM assets a
                JOIN work_variants wv ON wv.work_id = a.work_id
                WHERE wv.canonical_key = cw.canonical_key AND a.asset_type = 'game'
            ), 0) AS game_asset_count,
            COALESCE((
                SELECT COUNT(*)
                FROM work_credits wc
                WHERE wc.work_id = cw.preferred_work_id
            ), 0) AS credit_count,
            COALESCE((
                SELECT COUNT(*)
                FROM work_characters wch
                WHERE wch.work_id = cw.preferred_work_id
            ), 0) AS character_count
         FROM canonical_works cw
         JOIN works pw ON pw.id = cw.preferred_work_id
         ORDER BY
            CASE WHEN cw.enrichment_state != 'matched' THEN 0 ELSE 1 END,
            CASE WHEN cw.cover_path IS NULL OR cw.cover_path = '' THEN 0 ELSE 1 END,
            cw.variant_count DESC,
            cw.title
         LIMIT 80",
    )
    .fetch_all(pool)
    .await?;

    let works_missing: Vec<AssetGap> = poster_rows
        .iter()
        .map(|r| {
            let asset_types = parse_json_array(r.get::<Option<String>, _>("asset_types"));
            let asset_count: i64 = r.get("asset_count");
            let game_asset_count: i64 = r.get("game_asset_count");
            let mut missing = Vec::new();
            if asset_count == 0 {
                missing.push("Any assets".to_string());
            }
            if asset_count > 0 && game_asset_count == 0 {
                missing.push("Game package".to_string());
            }
            AssetGap {
                work_id: r.get("work_id"),
                title: r.get("title"),
                missing,
                asset_types,
                asset_count,
            }
        })
        .filter(|gap| {
            !gap.missing.is_empty()
                && !ignored_keys.contains(&(gap.work_id.clone(), "assets".to_string()))
        })
        .take(30)
        .collect();

    let mut diagnostics = Vec::new();
    let mut ignored_diagnostics = Vec::new();
    for row in poster_rows {
        let work_id: String = row.get("work_id");
        let title: String = row.get("title");
        let developer: Option<String> = row.get("developer");
        let enrichment_state: String = row.get("enrichment_state");
        let cover_path: Option<String> = row.get("cover_path");
        let variant_count: i64 = row.get("variant_count");
        let asset_count: i64 = row.get("asset_count");
        let primary_asset_type: Option<String> = row.get("primary_asset_type");
        let vndb_id: Option<String> = row.get("vndb_id");
        let bangumi_id: Option<String> = row.get("bangumi_id");
        let dlsite_id: Option<String> = row.get("dlsite_id");
        let game_asset_count: i64 = row.get("game_asset_count");
        let credit_count: i64 = row.get("credit_count");
        let character_count: i64 = row.get("character_count");
        let asset_types = parse_json_array(row.get::<Option<String>, _>("asset_types"));
        let field_preferences = parse_json_map(row.get::<Option<String>, _>("field_preferences"));

        let provider_list = summarize_sources(&vndb_id, &bangumi_id, &dlsite_id);
        let linked_sources = collect_linked_sources(&vndb_id, &bangumi_id, &dlsite_id);
        let asset_summary = if asset_types.is_empty() {
            "No classified assets".to_string()
        } else {
            format!("Assets: {}", asset_types.join(", "))
        };

        if enrichment_state != "matched" {
            let pending_review = enrichment_state == "pending_review";
            push_diagnostic(
                &mut diagnostics,
                &mut ignored_diagnostics,
                &ignored_keys,
                EnrichmentDiagnostic {
                    work_id: work_id.clone(),
                    title: title.clone(),
                    severity: if pending_review { "warn" } else { "critical" }.to_string(),
                    category: "enrichment".to_string(),
                    reason: if pending_review {
                        "Provider candidates exist but this poster still needs a review decision."
                            .to_string()
                    } else {
                        "No confirmed metadata match is linked to this poster yet.".to_string()
                    },
                    suggested_action: if pending_review {
                        "Open Enrichment Review and accept the best candidate.".to_string()
                    } else {
                        "Run batch match or inspect provider queries for this poster.".to_string()
                    },
                    details: vec![asset_summary.clone(), provider_list.clone()],
                    linked_sources: linked_sources.clone(),
                    preferred_field: None,
                    preferred_source: None,
                },
            );
        }

        if cover_path
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .is_empty()
        {
            push_diagnostic(
                &mut diagnostics,
                &mut ignored_diagnostics,
                &ignored_keys,
                EnrichmentDiagnostic {
                    work_id: work_id.clone(),
                    title: title.clone(),
                    severity: "warn".to_string(),
                    category: "cover".to_string(),
                    reason: "This poster still has no resolved cover.".to_string(),
                    suggested_action: "Refresh provider metadata or inspect source cover mapping."
                        .to_string(),
                    details: vec![provider_list.clone(), asset_summary.clone()],
                    linked_sources: linked_sources.clone(),
                    preferred_field: Some("cover_path".to_string()),
                    preferred_source: field_preferences.get("cover_path").cloned(),
                },
            );
        }

        if asset_count == 0 || game_asset_count == 0 {
            push_diagnostic(
                &mut diagnostics,
                &mut ignored_diagnostics,
                &ignored_keys,
                EnrichmentDiagnostic {
                    work_id: work_id.clone(),
                    title: title.clone(),
                    severity: "warn".to_string(),
                    category: "assets".to_string(),
                    reason: if asset_count == 0 {
                        "No assets were classified under this poster.".to_string()
                    } else {
                        "This poster has assets, but none were classified as a game package."
                            .to_string()
                    },
                    suggested_action: "Inspect folder naming and asset classification rules."
                        .to_string(),
                    details: vec![
                        asset_summary.clone(),
                        format!(
                            "Primary asset: {}",
                            primary_asset_type.unwrap_or_else(|| "unknown".to_string())
                        ),
                    ],
                    linked_sources: linked_sources.clone(),
                    preferred_field: None,
                    preferred_source: None,
                },
            );
        }

        if bangumi_id.is_some() && credit_count == 0 && character_count == 0 {
            push_diagnostic(
                &mut diagnostics,
                &mut ignored_diagnostics,
                &ignored_keys,
                EnrichmentDiagnostic {
                    work_id: work_id.clone(),
                    title: title.clone(),
                    severity: "warn".to_string(),
                    category: "metadata-depth".to_string(),
                    reason: "Bangumi is linked, but no staff or character graph was materialized."
                        .to_string(),
                    suggested_action:
                        "Refresh this poster's metadata and verify Bangumi people sync.".to_string(),
                    details: vec![provider_list.clone()],
                    linked_sources: linked_sources.clone(),
                    preferred_field: Some("description".to_string()),
                    preferred_source: field_preferences.get("description").cloned(),
                },
            );
        }

        if variant_count > 1 {
            push_diagnostic(
                &mut diagnostics,
                &mut ignored_diagnostics,
                &ignored_keys,
                EnrichmentDiagnostic {
                    work_id: work_id.clone(),
                    title: title.clone(),
                    severity: "info".to_string(),
                    category: "variants".to_string(),
                    reason:
                        "Multiple source folders currently collapse into this canonical poster."
                            .to_string(),
                    suggested_action:
                        "Review the poster merge if these variants should stay split.".to_string(),
                    details: vec![
                        format!("{} source folders attached", variant_count),
                        asset_summary.clone(),
                    ],
                    linked_sources: linked_sources.clone(),
                    preferred_field: None,
                    preferred_source: None,
                },
            );
        }

        if is_placeholder_title(&title) {
            push_diagnostic(
                &mut diagnostics,
                &mut ignored_diagnostics,
                &ignored_keys,
                EnrichmentDiagnostic {
                    work_id: work_id.clone(),
                    title: title.clone(),
                    severity: "warn".to_string(),
                    category: "title-quality".to_string(),
                    reason:
                        "The canonical title still looks like a code, dump label, or placeholder."
                            .to_string(),
                    suggested_action:
                        "Inspect parsing rules or manually confirm the canonical title.".to_string(),
                    details: vec![
                        provider_list.clone(),
                        format!(
                            "Developer: {}",
                            developer.unwrap_or_else(|| "unknown".to_string())
                        ),
                    ],
                    linked_sources,
                    preferred_field: Some("title".to_string()),
                    preferred_source: field_preferences.get("title").cloned(),
                },
            );
        }
    }

    diagnostics.sort_by_key(|item| diagnostic_rank(&item.severity));
    diagnostics.truncate(40);
    ignored_diagnostics.sort_by_key(|item| diagnostic_rank(&item.severity));
    ignored_diagnostics.truncate(20);

    Ok(GapReport {
        brands_without_all_works: Vec::new(),
        works_missing_assets: works_missing,
        enrichment_diagnostics: diagnostics,
        ignored_diagnostics,
        unmatched_works: unmatched,
        works_without_cover: no_cover,
        posters_with_variants,
    })
}

fn parse_json_array(raw: Option<String>) -> Vec<String> {
    raw.and_then(|value| serde_json::from_str::<Vec<String>>(&value).ok())
        .unwrap_or_default()
}

fn parse_json_map(raw: Option<String>) -> std::collections::HashMap<String, String> {
    raw.and_then(|value| {
        serde_json::from_str::<std::collections::HashMap<String, String>>(&value).ok()
    })
    .unwrap_or_default()
}

fn collect_linked_sources(
    vndb_id: &Option<String>,
    bangumi_id: &Option<String>,
    dlsite_id: &Option<String>,
) -> Vec<String> {
    let mut sources = Vec::new();
    if vndb_id
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        sources.push("vndb".to_string());
    }
    if bangumi_id
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        sources.push("bangumi".to_string());
    }
    if dlsite_id
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        sources.push("dlsite".to_string());
    }
    sources
}

fn push_diagnostic(
    diagnostics: &mut Vec<EnrichmentDiagnostic>,
    ignored_diagnostics: &mut Vec<EnrichmentDiagnostic>,
    ignored_keys: &std::collections::HashSet<(String, String)>,
    diagnostic: EnrichmentDiagnostic,
) {
    if ignored_keys.contains(&(diagnostic.work_id.clone(), diagnostic.category.clone())) {
        ignored_diagnostics.push(diagnostic);
    } else {
        diagnostics.push(diagnostic);
    }
}

fn summarize_sources(
    vndb_id: &Option<String>,
    bangumi_id: &Option<String>,
    dlsite_id: &Option<String>,
) -> String {
    let mut parts = Vec::new();
    if let Some(value) = vndb_id.as_deref().filter(|value| !value.trim().is_empty()) {
        parts.push(format!("VNDB {value}"));
    }
    if let Some(value) = bangumi_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        parts.push(format!("Bangumi {value}"));
    }
    if let Some(value) = dlsite_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        parts.push(format!("DLsite {value}"));
    }

    if parts.is_empty() {
        "Sources: none linked".to_string()
    } else {
        format!("Sources: {}", parts.join(", "))
    }
}

fn is_placeholder_title(title: &str) -> bool {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return true;
    }

    let ascii_alnum_only = trimmed.chars().all(|ch| ch.is_ascii_alphanumeric());
    let digit_count = trimmed.chars().filter(|ch| ch.is_ascii_digit()).count();
    let upper_count = trimmed.chars().filter(|ch| ch.is_ascii_uppercase()).count();

    if ascii_alnum_only && digit_count >= 5 {
        return true;
    }

    if trimmed.starts_with("VJ") && ascii_alnum_only && digit_count >= 5 && upper_count >= 2 {
        return true;
    }

    false
}

fn diagnostic_rank(severity: &str) -> u8 {
    match severity {
        "critical" => 0,
        "warn" => 1,
        "info" => 2,
        _ => 3,
    }
}

// ═══════════════════════════════════════════════
// 4. AI Translation Proxy
// ═══════════════════════════════════════════════

#[derive(Serialize, Deserialize)]
pub struct TranslateRequest {
    pub text: String,
    pub source_lang: String,
    pub target_lang: String,
}

#[derive(Serialize)]
pub struct TranslateResult {
    pub translated: String,
    pub source_lang: String,
    pub target_lang: String,
}

#[tauri::command]
pub async fn translate_text(
    db: State<'_, Database>,
    config: State<'_, SharedConfig>,
    text: String,
    source_lang: String,
    target_lang: String,
    api_key: Option<String>,
    provider: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
    work_id: Option<String>,
    field_name: Option<String>,
) -> Result<TranslateResult, AppError> {
    let configured_ai = {
        let cfg = config.read().await;
        cfg.ai.clone()
    };
    let resolved = resolve_ai_request(configured_ai, provider, base_url, model, api_key)?;
    let prompt = format!(
        "Translate the following text from {} to {}. Return ONLY the translation, nothing else.\n\n{}",
        source_lang, target_lang, text
    );
    let url = format!(
        "{}/chat/completions",
        resolved.base_url.trim_end_matches('/')
    );
    let body = serde_json::json!({
        "model": resolved.model,
        "messages": [{"role": "user", "content": prompt}],
        "temperature": 0.1
    });

    let client = reqwest::Client::new();
    let mut request = client.post(&url).header("Content-Type", "application/json");
    if !resolved.api_key.is_empty() {
        request = request.header("Authorization", format!("Bearer {}", resolved.api_key));
    }
    let resp = request
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?;

    let json: serde_json::Value = resp
        .json::<serde_json::Value>()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?;

    let translated = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();

    if let (Some(work_id), Some(field_name)) = (work_id.as_ref(), field_name.as_ref()) {
        let source = format!("ai_translation:{}", resolved.provider);
        sqlx::query(
            "INSERT INTO work_texts (work_id, field_name, lang, source, content) \
             VALUES (?, ?, ?, ?, ?) \
             ON CONFLICT(work_id, field_name, lang, source) \
             DO UPDATE SET content = excluded.content, created_at = datetime('now')",
        )
        .bind(work_id)
        .bind(field_name)
        .bind(&target_lang)
        .bind(source)
        .bind(&translated)
        .execute(db.read_pool())
        .await?;
    }

    Ok(TranslateResult {
        translated,
        source_lang,
        target_lang,
    })
}

struct ResolvedAiRequest {
    provider: String,
    base_url: String,
    model: String,
    api_key: String,
}

fn resolve_ai_request(
    configured: Option<AiProviderConfig>,
    provider: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
) -> Result<ResolvedAiRequest, AppError> {
    let current = configured.unwrap_or_default();
    let resolved_provider = provider
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| current.provider.clone());
    let resolved_base_url = base_url
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| preset_ai_base_url(&resolved_provider, &current.base_url));
    let resolved_model = model
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| current.model.clone());
    let resolved_api_key = api_key
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or(current.api_key.clone())
        .or_else(|| {
            if resolved_provider == "ollama" {
                Some(String::new())
            } else {
                None
            }
        })
        .ok_or_else(|| AppError::Validation("AI API key is not configured".to_string()))?;

    Ok(ResolvedAiRequest {
        provider: resolved_provider,
        base_url: resolved_base_url,
        model: resolved_model,
        api_key: resolved_api_key,
    })
}

fn preset_ai_base_url(provider: &str, configured: &str) -> String {
    if !configured.trim().is_empty() {
        return configured.trim().trim_end_matches('/').to_string();
    }

    match provider {
        "openai" => "https://api.openai.com/v1".to_string(),
        "openrouter" => "https://openrouter.ai/api/v1".to_string(),
        "ollama" => "http://127.0.0.1:11434/v1".to_string(),
        "openai-compatible" | "litellm" => "http://127.0.0.1:4000/v1".to_string(),
        _ => "http://127.0.0.1:4000/v1".to_string(),
    }
}

// ═══════════════════════════════════════════════
// 5. Import Queue
// ═══════════════════════════════════════════════

#[derive(Serialize, FromRow)]
pub struct ImportItem {
    pub id: String,
    pub source_path: String,
    pub file_name: String,
    pub file_size: i64,
    pub detected_type: String,
    pub status: String,
    pub target_work: Option<String>,
    pub error_msg: Option<String>,
}

#[tauri::command]
pub async fn list_import_queue(db: State<'_, Database>) -> Result<Vec<ImportItem>, AppError> {
    let rows: Vec<ImportItem> = sqlx::query_as(
        "SELECT id, source_path, file_name, file_size, detected_type, status, target_work, error_msg \
         FROM import_queue ORDER BY created_at DESC LIMIT 100",
    )
    .fetch_all(db.read_pool())
    .await?;
    Ok(rows)
}

#[tauri::command]
pub async fn add_to_import_queue(
    db: State<'_, Database>,
    source_path: String,
    file_name: String,
    file_size: i64,
    detected_type: Option<String>,
) -> Result<(), AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    let dt = detected_type.unwrap_or_else(|| "unknown".to_string());

    db.execute_write(
        "INSERT INTO import_queue (id, source_path, file_name, file_size, detected_type) VALUES (?1, ?2, ?3, ?4, ?5)"
            .to_string(),
        vec![
            serde_json::Value::String(id),
            serde_json::Value::String(source_path),
            serde_json::Value::String(file_name),
            serde_json::Value::Number(serde_json::Number::from(file_size)),
            serde_json::Value::String(dt),
        ],
    )
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn clear_import_queue(db: State<'_, Database>) -> Result<(), AppError> {
    db.execute_write(
        "DELETE FROM import_queue WHERE status IN ('done', 'error')".to_string(),
        vec![],
    )
    .await?;
    Ok(())
}

// ═══════════════════════════════════════════════
// 6. Source Plugins
// ═══════════════════════════════════════════════

#[derive(Serialize, FromRow)]
pub struct SourcePlugin {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub priority: i32,
    pub config_json: String,
    pub last_sync: Option<String>,
}

#[tauri::command]
pub async fn list_source_plugins(db: State<'_, Database>) -> Result<Vec<SourcePlugin>, AppError> {
    let rows: Vec<SourcePlugin> = sqlx::query_as(
        "SELECT id, name, enabled, priority, config_json, last_sync \
         FROM source_plugins ORDER BY priority DESC",
    )
    .fetch_all(db.read_pool())
    .await?;
    Ok(rows)
}

#[tauri::command]
pub async fn toggle_source_plugin(
    db: State<'_, Database>,
    id: String,
    enabled: bool,
) -> Result<(), AppError> {
    db.execute_write(
        "UPDATE source_plugins SET enabled = ?1 WHERE id = ?2".to_string(),
        vec![
            serde_json::Value::Bool(enabled),
            serde_json::Value::String(id),
        ],
    )
    .await?;
    Ok(())
}

// ═══════════════════════════════════════════════
// 7. i18n / Locale Switching
// ═══════════════════════════════════════════════

#[tauri::command]
pub async fn get_locale(config: State<'_, SharedConfig>) -> Result<String, AppError> {
    let cfg = config.read().await;
    Ok(cfg.locale.clone())
}

#[tauri::command]
pub async fn set_locale(config: State<'_, SharedConfig>, locale: String) -> Result<(), AppError> {
    let allowed = ["ja", "en", "zh-Hans", "zh-Hant"];
    if !allowed.contains(&locale.as_str()) {
        return Err(AppError::Validation(format!(
            "Unsupported locale: {}",
            locale
        )));
    }
    config
        .update(|cfg| {
            cfg.locale = locale;
        })
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn get_localized_text(
    db: State<'_, Database>,
    work_id: String,
    field: String,
    locale: String,
) -> Result<Option<String>, AppError> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT content FROM work_texts \
             WHERE work_id = ? AND field_name = ? AND lang = ? \
             ORDER BY CASE source WHEN 'user' THEN 0 ELSE 1 END, created_at DESC \
             LIMIT 1",
    )
    .bind(&work_id)
    .bind(&field)
    .bind(&locale)
    .fetch_optional(db.read_pool())
    .await?;
    Ok(row.map(|r| r.0))
}

// ═══════════════════════════════════════════════
// 8. Performance — Batch Preload
// ═══════════════════════════════════════════════

#[derive(Serialize, FromRow)]
pub struct WorkMinimal {
    pub id: String,
    pub title: String,
    pub cover_path: Option<String>,
    pub developer: Option<String>,
    pub rating: Option<f64>,
    pub library_status: String,
}

#[tauri::command]
pub async fn bulk_load_works(
    db: State<'_, Database>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<WorkMinimal>, AppError> {
    let lim = limit.unwrap_or(10000).min(50000);
    let off = offset.unwrap_or(0);

    let rows: Vec<WorkMinimal> = sqlx::query_as(
        "SELECT id, title, cover_path, developer, rating, library_status \
         FROM works ORDER BY title LIMIT ? OFFSET ?",
    )
    .bind(lim)
    .bind(off)
    .fetch_all(db.read_pool())
    .await?;
    Ok(rows)
}
