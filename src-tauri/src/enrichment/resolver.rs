//! Field resolver — simple priority chain: user > vndb > dlsite > bangumi > filesystem.

use std::collections::HashMap;

use crate::domain::work::{FieldSource, Work};
use crate::enrichment::bangumi::BangumiSubject;
use crate::enrichment::dlsite::DlsiteProduct;
use crate::enrichment::vndb::{self, VndbVn};

pub fn resolve(
    work: &mut Work,
    vndb: Option<&VndbVn>,
    bangumi: Option<&BangumiSubject>,
    dlsite: Option<&DlsiteProduct>,
) {
    resolve_with_defaults(work, vndb, bangumi, dlsite, &HashMap::new());
}

pub fn resolve_with_defaults(
    work: &mut Work,
    vndb: Option<&VndbVn>,
    bangumi: Option<&BangumiSubject>,
    dlsite: Option<&DlsiteProduct>,
    provider_defaults: &HashMap<String, String>,
) {
    if resolved_field_source(work, "title") != Some("user_override") {
        if let Some((source, title, title_original, aliases)) =
            select_title_source(work, vndb, bangumi, dlsite, provider_defaults)
        {
            work.title = title;
            work.title_original = title_original;
            if !aliases.is_empty() {
                work.title_aliases = aliases;
            }
            work.title_source = field_source_enum(source);
            work.field_sources
                .insert("title".to_string(), source.to_string());
            work.field_sources
                .insert("title_aliases".to_string(), source.to_string());
        }
    }

    if let Some(source) = choose_provider_source(
        work,
        "developer",
        vndb.is_some(),
        bangumi.is_some(),
        dlsite.is_some(),
        provider_defaults,
    ) {
        if source == "vndb" {
            if let Some(vn) = vndb.and_then(|vn| vn.developers.first()) {
                work.developer = Some(vn.name.clone());
                work.field_sources
                    .insert("developer".to_string(), "vndb".to_string());
            }
        } else if source == "dlsite" {
            if let Some(value) = dlsite.and_then(|dl| dl.maker_name.clone()) {
                work.developer = Some(value);
                work.field_sources
                    .insert("developer".to_string(), "dlsite".to_string());
            }
        }
    }

    if let Some(source) = choose_provider_source(
        work,
        "description",
        vndb.is_some(),
        bangumi.is_some(),
        dlsite.is_some(),
        provider_defaults,
    ) {
        let description = match source {
            "vndb" => vndb.and_then(|vn| vn.description.clone()),
            "dlsite" => dlsite.and_then(|dl| dl.description.clone()),
            "bangumi" => bangumi.and_then(|bgm| bgm.summary.clone()),
            _ => None,
        };
        if let Some(description) = description.filter(|value| !value.trim().is_empty()) {
            work.description = Some(description);
            work.field_sources
                .insert("description".to_string(), source.to_string());
        }
    }

    if let Some(source) = choose_provider_source(
        work,
        "release_date",
        vndb.is_some(),
        bangumi.is_some(),
        dlsite.is_some(),
        provider_defaults,
    ) {
        let date = match source {
            "vndb" => vndb
                .and_then(|vn| vn.released.as_deref())
                .and_then(parse_date),
            "dlsite" => dlsite
                .and_then(|dl| dl.regist_date.as_deref())
                .and_then(parse_date),
            "bangumi" => bangumi
                .and_then(|bgm| bgm.air_date.as_deref())
                .and_then(parse_date),
            _ => None,
        };
        if let Some(date) = date {
            work.release_date = Some(date);
            work.field_sources
                .insert("release_date".to_string(), source.to_string());
        }
    }

    if let Some(source) = choose_provider_source(
        work,
        "rating",
        vndb.is_some(),
        bangumi.is_some(),
        dlsite.is_some(),
        provider_defaults,
    ) {
        match source {
            "vndb" => {
                if let Some(vn) = vndb {
                    if let Some(rating) = vn.rating {
                        work.rating = Some(rating);
                        work.vote_count = vn.votecount.map(|v| v as u32);
                        work.field_sources
                            .insert("rating".to_string(), "vndb".to_string());
                    }
                }
            }
            "dlsite" => {
                if let Some(dl) = dlsite {
                    if let Some(rating) = dl.rate_average {
                        work.rating = Some(rating);
                        work.vote_count = dl.rate_count;
                        work.field_sources
                            .insert("rating".to_string(), "dlsite".to_string());
                    }
                }
            }
            "bangumi" => {
                if let Some(rating) = bangumi.and_then(|bgm| bgm.rating.as_ref()) {
                    work.rating = Some(rating.score);
                    work.vote_count = Some(rating.total);
                    work.field_sources
                        .insert("rating".to_string(), "bangumi".to_string());
                }
            }
            _ => {}
        }
    }

    if let Some(source) = choose_provider_source(
        work,
        "tags",
        vndb.is_some(),
        bangumi.is_some(),
        dlsite.is_some(),
        provider_defaults,
    ) {
        let tags = match source {
            "vndb" => vndb
                .map(|vn| {
                    vn.tags
                        .iter()
                        .filter(|t| t.rating >= 2.0)
                        .map(|t| t.name.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            "dlsite" => dlsite.map(|dl| dl.genres.clone()).unwrap_or_default(),
            _ => Vec::new(),
        };
        if !tags.is_empty() {
            work.tags = tags;
            work.field_sources
                .insert("tags".to_string(), source.to_string());
        }
    }

    if let Some(source) = choose_provider_source(
        work,
        "cover_path",
        vndb.is_some(),
        bangumi.is_some(),
        dlsite.is_some(),
        provider_defaults,
    ) {
        let cover = match source {
            "vndb" => vndb.and_then(|vn| vn.image.as_ref().map(|image| image.url.clone())),
            "dlsite" => dlsite.and_then(|dl| dl.image_main.clone()),
            "bangumi" => bangumi.and_then(|bgm| {
                bgm.images.as_ref().and_then(|images| {
                    images
                        .large
                        .clone()
                        .or_else(|| images.medium.clone())
                        .or_else(|| images.small.clone())
                })
            }),
            _ => None,
        };
        if let Some(cover) = cover {
            work.cover_path = Some(cover);
            work.field_sources
                .insert("cover_path".to_string(), source.to_string());
        }
    }

    if let Some(vn) = vndb {
        work.vndb_id = Some(vn.id.clone());
    }
    if let Some(bgm) = bangumi {
        work.bangumi_id = Some(bgm.id.to_string());
    }
    if let Some(dl) = dlsite {
        work.dlsite_id = Some(dl.product_id.clone());
    }
}

fn select_title_source(
    work: &Work,
    vndb: Option<&VndbVn>,
    bangumi: Option<&BangumiSubject>,
    dlsite: Option<&DlsiteProduct>,
    provider_defaults: &HashMap<String, String>,
) -> Option<(&'static str, String, Option<String>, Vec<String>)> {
    let preferred = choose_provider_source(
        work,
        "title",
        vndb.is_some(),
        bangumi.is_some(),
        dlsite.is_some(),
        provider_defaults,
    );
    match preferred {
        Some("vndb") => vndb.map(|vn| {
            let preferred_title = vndb::preferred_display_title(vn);
            let original = vn
                .alttitle
                .clone()
                .filter(|title| title != &preferred_title)
                .or_else(|| {
                    if vn.title != preferred_title {
                        Some(vn.title.clone())
                    } else {
                        None
                    }
                });
            (
                "vndb",
                preferred_title,
                original,
                vndb::candidate_titles(vn),
            )
        }),
        Some("dlsite") => dlsite.and_then(|dl| {
            dl.product_name
                .clone()
                .map(|title| ("dlsite", title, None, Vec::new()))
        }),
        Some("bangumi") => bangumi.map(|bgm| {
            let title = bgm.name_cn.clone().unwrap_or_else(|| bgm.name.clone());
            let aliases = vec![bgm.name.clone()]
                .into_iter()
                .chain(bgm.name_cn.clone())
                .collect::<Vec<_>>();
            (
                "bangumi",
                title,
                bgm.name_cn
                    .clone()
                    .filter(|_| bgm.name_cn.as_deref() != Some(&bgm.name)),
                aliases,
            )
        }),
        _ => None,
    }
}

fn choose_provider_source(
    work: &Work,
    field: &str,
    has_vndb: bool,
    has_bangumi: bool,
    has_dlsite: bool,
    provider_defaults: &HashMap<String, String>,
) -> Option<&'static str> {
    if resolved_field_source(work, field) == Some("user_override") {
        return None;
    }
    if let Some(preferred) = preferred_field_source(work, field) {
        return match preferred {
            "vndb" if has_vndb => Some("vndb"),
            "dlsite" if has_dlsite => Some("dlsite"),
            "bangumi" if has_bangumi => Some("bangumi"),
            _ => fallback_provider(has_vndb, has_bangumi, has_dlsite),
        };
    }
    if let Some(preferred) = provider_defaults.get(field).map(String::as_str) {
        return match preferred {
            "vndb" if has_vndb => Some("vndb"),
            "dlsite" if has_dlsite => Some("dlsite"),
            "bangumi" if has_bangumi => Some("bangumi"),
            _ => fallback_provider(has_vndb, has_bangumi, has_dlsite),
        };
    }
    fallback_provider(has_vndb, has_bangumi, has_dlsite)
}

fn preferred_field_source<'a>(work: &'a Work, field: &str) -> Option<&'a str> {
    work.field_preferences.get(field).map(String::as_str)
}

fn resolved_field_source<'a>(work: &'a Work, field: &str) -> Option<&'a str> {
    work.field_sources.get(field).map(String::as_str)
}

fn fallback_provider(has_vndb: bool, has_bangumi: bool, has_dlsite: bool) -> Option<&'static str> {
    if has_vndb {
        Some("vndb")
    } else if has_dlsite {
        Some("dlsite")
    } else if has_bangumi {
        Some("bangumi")
    } else {
        None
    }
}

fn field_source_enum(source: &str) -> FieldSource {
    match source {
        "vndb" => FieldSource::Vndb,
        "dlsite" => FieldSource::Dlsite,
        "bangumi" => FieldSource::Bangumi,
        _ => FieldSource::Filesystem,
    }
}

fn parse_date(value: &str) -> Option<chrono::NaiveDate> {
    chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

#[cfg(test)]
mod tests {
    use super::resolve;
    use crate::domain::work::{FieldSource, Work};
    use crate::enrichment::bangumi::{BangumiImages, BangumiSubject};
    use crate::enrichment::dlsite::DlsiteProduct;

    #[test]
    fn resolve_uses_dlsite_when_primary_sources_missing() {
        let mut work = Work::from_discovery("C:/tmp".into(), "local title".to_string(), 0.0);
        let dlsite = DlsiteProduct {
            product_id: "RJ123456".to_string(),
            product_name: Some("DLsite Title".to_string()),
            maker_name: Some("Circle".to_string()),
            maker_id: None,
            price: None,
            work_type: None,
            age_category: None,
            regist_date: Some("2025-01-01".to_string()),
            image_main: Some("https://example.com/cover.jpg".to_string()),
            genres: vec!["ADV".to_string()],
            description: Some("desc".to_string()),
            dl_count: None,
            rate_average: Some(8.4),
            rate_count: Some(42),
        };

        resolve(&mut work, None, None, Some(&dlsite));

        assert_eq!(work.title, "DLsite Title");
        assert_eq!(work.title_source, FieldSource::Dlsite);
        assert_eq!(work.developer.as_deref(), Some("Circle"));
        assert_eq!(work.dlsite_id.as_deref(), Some("RJ123456"));
        assert_eq!(work.tags, vec!["ADV".to_string()]);
    }

    #[test]
    fn resolve_respects_field_preference_over_default_priority() {
        let mut work = Work::from_discovery("C:/tmp".into(), "local title".to_string(), 0.0);
        work.field_preferences
            .insert("cover_path".to_string(), "bangumi".to_string());

        let dlsite = DlsiteProduct {
            product_id: "RJ123456".to_string(),
            product_name: Some("DLsite Title".to_string()),
            maker_name: Some("Circle".to_string()),
            maker_id: None,
            price: None,
            work_type: None,
            age_category: None,
            regist_date: Some("2025-01-01".to_string()),
            image_main: Some("https://example.com/dlsite.jpg".to_string()),
            genres: vec!["ADV".to_string()],
            description: Some("desc".to_string()),
            dl_count: None,
            rate_average: Some(8.4),
            rate_count: Some(42),
        };
        let bangumi = BangumiSubject {
            id: 1,
            name: "Bangumi Title".to_string(),
            name_cn: None,
            summary: None,
            air_date: None,
            rating: None,
            images: Some(BangumiImages {
                large: Some("https://example.com/bangumi.jpg".to_string()),
                medium: None,
                grid: None,
                small: None,
            }),
            subject_type: Some(4),
        };

        resolve(&mut work, None, Some(&bangumi), Some(&dlsite));

        assert_eq!(
            work.cover_path.as_deref(),
            Some("https://example.com/bangumi.jpg")
        );
        assert_eq!(
            work.field_sources.get("cover_path").map(String::as_str),
            Some("bangumi")
        );
    }
}
