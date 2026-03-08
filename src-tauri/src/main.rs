#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use galroon_lib::api;
use galroon_lib::config::{AppConfig, LauncherConfig, SharedConfig};
use galroon_lib::db::queries;
use galroon_lib::db::Database;
use galroon_lib::enrichment::bangumi::BangumiClient;
use galroon_lib::enrichment::dlsite::DlsiteClient;
use galroon_lib::enrichment::queue::EnrichmentWorker;
use galroon_lib::enrichment::rate_limit::RateLimiter;
use galroon_lib::enrichment::vndb::VndbClient;
use galroon_lib::jobs::{backup_scheduler_loop, should_auto_check_updates, AppJobWorker};
use galroon_lib::observability;
use galroon_lib::scanner::watcher;

#[tokio::main]
async fn main() {
    observability::init_logging();

    tracing::info!("Galroon v0.5.0 starting");

    let mut launcher = LauncherConfig::load().expect("Failed to load launcher config");
    let workspace_dir = resolve_workspace(&launcher);

    let config = match workspace_dir {
        Some(ref ws) => {
            if !AppConfig::is_workspace(ws) {
                AppConfig::init_workspace(ws).expect("Failed to initialize workspace")
            } else {
                AppConfig::load_from(ws).expect("Failed to load workspace config")
            }
        }
        None => {
            let default_dir = directories::ProjectDirs::from("com", "galroon", "Galroon")
                .expect("Cannot determine app data directory")
                .data_dir()
                .to_path_buf();
            AppConfig::init_workspace(&default_dir).expect("Failed to create default workspace")
        }
    };

    launcher.last_workspace = Some(config.workspace_dir.clone());
    let _ = launcher.save();

    tracing::info!(workspace = %config.workspace_dir.display(), "Workspace loaded");

    let bangumi_auth = config.bangumi.clone();
    let library_roots = config.library_roots.clone();
    let db_path = config.db_path.clone();
    let shared_config = SharedConfig::new(config);

    let db = Database::new(&db_path)
        .await
        .expect("Failed to initialize database");

    tracing::info!(db_path = %db_path.display(), "Database initialized");

    queries::canonical::rebuild(db.read_pool())
        .await
        .expect("Failed to rebuild canonical works");

    let rate_limiter = RateLimiter::new();
    let vndb = VndbClient::new(rate_limiter.clone());
    let bangumi = BangumiClient::new(
        rate_limiter.clone(),
        bangumi_auth,
        Some(shared_config.clone()),
    );
    let dlsite = DlsiteClient::new(rate_limiter.clone());
    let bangumi_oauth = api::settings::BangumiOAuthManager::default();
    let (worker_shutdown_tx, worker_shutdown_rx) = tokio::sync::watch::channel(false);
    let app_worker_shutdown_rx = worker_shutdown_tx.subscribe();
    let backup_scheduler_shutdown_rx = worker_shutdown_tx.subscribe();

    let recent_writes = watcher::RecentWrites::new();
    if !library_roots.is_empty() {
        let roots = library_roots;
        let rw = recent_writes.clone();
        tokio::spawn(async move {
            match watcher::start_watcher(roots, watcher::WatcherConfig::default(), rw) {
                Ok(mut rx) => {
                    tracing::info!("Filesystem watcher started");
                    while let Some(dirty_roots) = rx.recv().await {
                        tracing::info!(count = dirty_roots.len(), "Watcher detected changes");
                    }
                }
                Err(e) => tracing::warn!(error = %e, "Failed to start filesystem watcher"),
            }
        });
    }

    {
        let worker = EnrichmentWorker::from_clients(
            std::sync::Arc::new(db.clone()),
            vndb.clone(),
            bangumi.clone(),
            dlsite.clone(),
        );
        tokio::spawn(async move {
            worker.run(worker_shutdown_rx).await;
        });
    }

    {
        let worker = AppJobWorker::new(std::sync::Arc::new(db.clone()), shared_config.clone());
        tokio::spawn(async move {
            worker.run(app_worker_shutdown_rx).await;
        });
    }

    {
        let scheduler_config = shared_config.clone();
        let scheduler_db = std::sync::Arc::new(db.clone());
        tokio::spawn(async move {
            backup_scheduler_loop(scheduler_config, scheduler_db, backup_scheduler_shutdown_rx).await;
        });
    }

    if should_auto_check_updates(&shared_config.snapshot().await.updates) {
        let _ = queries::app_jobs::enqueue_job(
            db.read_pool(),
            "update_check",
            "Check for updates on launch",
            None,
            Some("update:check"),
            false,
            false,
            true,
        )
        .await;
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .manage(shared_config)
        .manage(db)
        .manage(rate_limiter)
        .manage(vndb)
        .manage(bangumi)
        .manage(dlsite)
        .manage(bangumi_oauth)
        .manage(worker_shutdown_tx)
        .invoke_handler(tauri::generate_handler![
            api::works::list_works,
            api::works::get_work,
            api::works::list_work_credits,
            api::works::list_work_variants,
            api::works::list_work_asset_groups,
            api::works::update_work_field,
            api::works::reset_work_field_override,
            api::scanner::trigger_scan,
            api::scanner::get_scan_status,
            api::settings::get_settings,
            api::settings::update_settings,
            api::settings::get_bangumi_auth_status,
            api::settings::update_bangumi_auth,
            api::settings::clear_bangumi_auth,
            api::settings::get_ai_provider_status,
            api::settings::update_ai_provider_settings,
            api::settings::clear_ai_provider_settings,
            api::settings::probe_ai_provider,
            api::settings::start_bangumi_oauth,
            api::settings::get_bangumi_oauth_status,
            api::settings::cancel_bangumi_oauth,
            api::settings::probe_bangumi_auth,
            api::settings::get_workspace_info,
            api::settings::get_recent_workspaces,
            api::settings::check_workspace_status,
            api::settings::init_workspace,
            api::settings::relocate_workspace,
            api::settings::backup_workspace,
            api::settings::list_trash,
            api::settings::purge_trash,
            api::settings::empty_trash,
            api::jobs::list_app_jobs,
            api::jobs::pause_app_job,
            api::jobs::resume_app_job,
            api::jobs::cancel_app_job,
            api::jobs::enqueue_backup_job,
            api::jobs::enqueue_update_check,
            api::jobs::enqueue_library_enrichment,
            api::jobs::get_enrichment_queue_status,
            api::jobs::pause_enrichment_queue,
            api::jobs::resume_enrichment_queue,
            api::jobs::get_backup_schedule,
            api::jobs::update_backup_schedule,
            api::jobs::get_update_settings,
            api::jobs::update_update_settings,
            api::jobs::check_native_update,
            api::jobs::install_native_update,
            api::dashboard::get_dashboard_stats,
            api::dashboard::toggle_sfw,
            api::search::search_works,
            api::thumbnails::get_thumbnail,
            api::enrichment::get_unmatched_works,
            api::enrichment::get_enrichment_review_item,
            api::enrichment::search_enrichment_candidates,
            api::enrichment::confirm_enrichment_match,
            api::enrichment::set_work_field_preference,
            api::enrichment::reject_enrichment,
            api::brands::list_brands,
            api::brands::get_brand_detail,
            api::brands::list_creators,
            api::brands::get_creator_detail,
            api::characters::list_characters,
            api::characters::get_character,
            api::characters::search_characters,
            api::duplicates::find_duplicates,
            api::collections::list_collections,
            api::collections::create_collection,
            api::collections::delete_collection,
            api::collections::add_to_collection,
            api::collections::remove_from_collection,
            api::collections::get_collection_works,
            api::collections::list_wishlist,
            api::collections::add_wishlist,
            api::collections::remove_wishlist,
            api::collections::random_pick,
            api::collections::get_activity_feed,
            api::collections::export_library,
            api::workshop::bulk_update_field,
            api::workshop::set_canonical_representative,
            api::workshop::split_work_variant,
            api::workshop::clear_work_variant_override,
            api::workshop::merge_poster_groups,
            api::workshop::merge_works,
            api::workshop::reset_enrichment,
            api::workshop::refresh_work_provider_link,
            api::workshop::ignore_workshop_diagnostic,
            api::workshop::restore_workshop_diagnostic,
            api::workshop::batch_ignore_workshop_diagnostics,
            api::workshop::batch_restore_workshop_diagnostics,
            api::workshop::batch_apply_diagnostic_preferences,
            api::workshop::batch_refresh_work_provider_links,
            api::workshop::list_provider_field_defaults,
            api::workshop::set_provider_field_default,
            api::workshop::get_year_in_review,
            api::extended::list_library_roots,
            api::extended::add_library_root,
            api::extended::remove_library_root,
            api::extended::get_completion,
            api::extended::update_completion,
            api::extended::get_gap_analysis,
            api::extended::translate_text,
            api::extended::list_import_queue,
            api::extended::add_to_import_queue,
            api::extended::clear_import_queue,
            api::extended::list_source_plugins,
            api::extended::toggle_source_plugin,
            api::extended::get_locale,
            api::extended::set_locale,
            api::extended::get_localized_text,
            api::extended::bulk_load_works,
            api::tags::list_user_tags,
            api::tags::add_user_tag,
            api::tags::delete_user_tag,
            api::tags::rename_user_tag,
            api::tags::tag_work,
            api::tags::untag_work,
            api::tags::get_work_tags,
            api::tags::search_tags,
            api::tags::bulk_tag_works,
            api::collections::evaluate_smart_collection,
            api::collections::reorder_collection,
            api::collections::multi_source_match,
            api::collections::batch_multi_source_match,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn resolve_workspace(launcher: &LauncherConfig) -> Option<std::path::PathBuf> {
    if let Some(arg) = std::env::args().nth(1) {
        let path = std::path::PathBuf::from(arg);
        if path.is_dir() {
            return Some(path);
        }
    }

    if let Ok(dir) = std::env::var("GALROON_WORKSPACE") {
        let path = std::path::PathBuf::from(dir);
        if path.is_dir() || !path.exists() {
            return Some(path);
        }
    }

    launcher.last_workspace.clone()
}
