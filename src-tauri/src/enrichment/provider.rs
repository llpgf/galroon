//! Provider boundary for metadata sources.

use regex::Regex;

use crate::domain::work::Work;
use crate::enrichment::bangumi::{BangumiClient, BangumiSubject};
use crate::enrichment::dlsite::{DlsiteClient, DlsiteProduct};
use crate::enrichment::vndb::{self, VndbClient, VndbVn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderLinkState {
    NotLinked,
    Ready,
    Missing,
    AuthError,
    RateLimited,
    TransientError,
}

impl ProviderLinkState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotLinked => "not_linked",
            Self::Ready => "ready",
            Self::Missing => "missing",
            Self::AuthError => "auth_error",
            Self::RateLimited => "rate_limited",
            Self::TransientError => "transient_error",
        }
    }

    pub fn should_retry(&self) -> bool {
        matches!(
            self,
            Self::AuthError | Self::RateLimited | Self::TransientError
        )
    }
}

#[derive(Debug, Clone)]
pub struct LinkedProviderRecord {
    pub source: MetadataSource,
    pub external_id: Option<String>,
    pub record: Option<ProviderRecord>,
    pub state: ProviderLinkState,
    pub message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LinkedProviderRecords {
    pub vndb: LinkedProviderRecord,
    pub bangumi: LinkedProviderRecord,
    pub dlsite: LinkedProviderRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetadataField {
    Title,
    TitleAliases,
    Developer,
    ReleaseDate,
    Rating,
    Description,
    Tags,
    CoverImage,
}

impl MetadataField {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::TitleAliases => "title_aliases",
            Self::Developer => "developer",
            Self::ReleaseDate => "release_date",
            Self::Rating => "rating",
            Self::Description => "description",
            Self::Tags => "tags",
            Self::CoverImage => "cover_image",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Title => "Title",
            Self::TitleAliases => "Aliases",
            Self::Developer => "Developer",
            Self::ReleaseDate => "Release Date",
            Self::Rating => "Rating",
            Self::Description => "Description",
            Self::Tags => "Tags",
            Self::CoverImage => "Cover",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetadataSource {
    Vndb,
    Bangumi,
    Dlsite,
}

impl MetadataSource {
    pub fn all() -> [Self; 3] {
        [Self::Vndb, Self::Bangumi, Self::Dlsite]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Vndb => "vndb",
            Self::Bangumi => "bangumi",
            Self::Dlsite => "dlsite",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Vndb => "VNDB",
            Self::Bangumi => "Bangumi",
            Self::Dlsite => "DLsite",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "vndb" => Some(Self::Vndb),
            "bangumi" => Some(Self::Bangumi),
            "dlsite" => Some(Self::Dlsite),
            _ => None,
        }
    }

    pub fn supported_fields(&self) -> &'static [MetadataField] {
        match self {
            Self::Vndb => &[
                MetadataField::Title,
                MetadataField::TitleAliases,
                MetadataField::Developer,
                MetadataField::ReleaseDate,
                MetadataField::Rating,
                MetadataField::Description,
                MetadataField::Tags,
                MetadataField::CoverImage,
            ],
            Self::Bangumi => &[
                MetadataField::Title,
                MetadataField::ReleaseDate,
                MetadataField::Rating,
                MetadataField::Description,
                MetadataField::CoverImage,
            ],
            Self::Dlsite => &[
                MetadataField::Title,
                MetadataField::Developer,
                MetadataField::ReleaseDate,
                MetadataField::Rating,
                MetadataField::Description,
                MetadataField::Tags,
                MetadataField::CoverImage,
            ],
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProviderSearchResult {
    pub id: String,
    pub title: String,
    pub title_original: Option<String>,
    pub search_titles: Vec<String>,
    pub developer: Option<String>,
    pub rating: Option<f64>,
    pub source: MetadataSource,
    pub record: Option<ProviderRecord>,
}

#[derive(Debug, Clone)]
pub enum ProviderRecord {
    Vndb(VndbVn),
    Bangumi(BangumiSubject),
    Dlsite(DlsiteProduct),
}

impl ProviderRecord {
    pub fn source(&self) -> MetadataSource {
        match self {
            Self::Vndb(_) => MetadataSource::Vndb,
            Self::Bangumi(_) => MetadataSource::Bangumi,
            Self::Dlsite(_) => MetadataSource::Dlsite,
        }
    }

    pub fn id(&self) -> String {
        match self {
            Self::Vndb(vn) => vn.id.clone(),
            Self::Bangumi(subject) => subject.id.to_string(),
            Self::Dlsite(product) => product.product_id.clone(),
        }
    }

    pub fn title(&self) -> String {
        match self {
            Self::Vndb(vn) => vndb::preferred_display_title(vn),
            Self::Bangumi(subject) => subject.name.clone(),
            Self::Dlsite(product) => product
                .product_name
                .clone()
                .unwrap_or_else(|| product.product_id.clone()),
        }
    }

    pub fn title_original(&self) -> Option<String> {
        match self {
            Self::Vndb(vn) => vn.alttitle.clone(),
            Self::Bangumi(subject) => subject.name_cn.clone(),
            Self::Dlsite(_) => None,
        }
    }

    pub fn title_aliases(&self) -> Vec<String> {
        match self {
            Self::Vndb(vn) => vndb::candidate_titles(vn),
            Self::Bangumi(subject) => {
                let mut titles = vec![subject.name.clone()];
                if let Some(name_cn) = &subject.name_cn {
                    if !titles.iter().any(|existing| existing == name_cn) {
                        titles.push(name_cn.clone());
                    }
                }
                titles
            }
            Self::Dlsite(product) => {
                let mut titles = Vec::new();
                if let Some(name) = &product.product_name {
                    titles.push(name.clone());
                }
                titles.push(product.product_id.clone());
                titles
            }
        }
    }

    pub fn developer(&self) -> Option<String> {
        match self {
            Self::Vndb(vn) => vn.developers.first().map(|dev| dev.name.clone()),
            Self::Bangumi(_) => None,
            Self::Dlsite(product) => product.maker_name.clone(),
        }
    }

    pub fn rating(&self) -> Option<f64> {
        match self {
            Self::Vndb(vn) => vn.rating,
            Self::Bangumi(subject) => subject.rating.as_ref().map(|rating| rating.score),
            Self::Dlsite(product) => product.rate_average,
        }
    }

    pub fn release_date(&self) -> Option<String> {
        match self {
            Self::Vndb(vn) => vn.released.clone(),
            Self::Bangumi(subject) => subject.air_date.clone(),
            Self::Dlsite(product) => product.regist_date.clone(),
        }
    }

    pub fn description(&self) -> Option<String> {
        match self {
            Self::Vndb(vn) => vn.description.clone(),
            Self::Bangumi(subject) => subject.summary.clone(),
            Self::Dlsite(product) => product.description.clone(),
        }
    }

    pub fn cover_url(&self) -> Option<String> {
        match self {
            Self::Vndb(vn) => vn.image.as_ref().map(|image| image.url.clone()),
            Self::Bangumi(subject) => subject.images.as_ref().and_then(|images| {
                images
                    .large
                    .clone()
                    .or_else(|| images.medium.clone())
                    .or_else(|| images.small.clone())
            }),
            Self::Dlsite(product) => product.image_main.clone(),
        }
    }

    pub fn tags(&self) -> Vec<String> {
        match self {
            Self::Vndb(vn) => vn
                .tags
                .iter()
                .filter(|tag| tag.rating >= 2.0)
                .map(|tag| tag.name.clone())
                .collect(),
            Self::Bangumi(_) => Vec::new(),
            Self::Dlsite(product) => product.genres.clone(),
        }
    }

    pub fn search_titles(&self) -> Vec<String> {
        self.title_aliases()
    }

    pub fn field_value(&self, field: MetadataField) -> Option<String> {
        match field {
            MetadataField::Title => Some(self.title()),
            MetadataField::TitleAliases => {
                let aliases = self.title_aliases();
                if aliases.is_empty() {
                    None
                } else {
                    Some(aliases.join(" | "))
                }
            }
            MetadataField::Developer => self.developer(),
            MetadataField::ReleaseDate => self.release_date(),
            MetadataField::Rating => self.rating().map(|value| format!("{value:.1}")),
            MetadataField::Description => self.description(),
            MetadataField::Tags => {
                let tags = self.tags();
                if tags.is_empty() {
                    None
                } else {
                    Some(tags.join(", "))
                }
            }
            MetadataField::CoverImage => self.cover_url(),
        }
    }

    pub fn as_vndb(&self) -> Option<&VndbVn> {
        match self {
            Self::Vndb(vn) => Some(vn),
            _ => None,
        }
    }

    pub fn as_bangumi(&self) -> Option<&BangumiSubject> {
        match self {
            Self::Bangumi(subject) => Some(subject),
            _ => None,
        }
    }

    pub fn as_dlsite(&self) -> Option<&DlsiteProduct> {
        match self {
            Self::Dlsite(product) => Some(product),
            _ => None,
        }
    }
}

pub async fn search_provider(
    source: MetadataSource,
    vndb: &VndbClient,
    bangumi: &BangumiClient,
    dlsite: &DlsiteClient,
    query: &str,
    limit: u32,
) -> Result<Vec<ProviderSearchResult>, String> {
    match source {
        MetadataSource::Vndb => Ok(vndb
            .search_by_title(query, limit)
            .await?
            .into_iter()
            .map(|vn| ProviderSearchResult {
                id: vn.id.clone(),
                title: vndb::preferred_display_title(&vn),
                title_original: vn.alttitle.clone(),
                search_titles: vndb::candidate_titles(&vn),
                developer: vn.developers.first().map(|dev| dev.name.clone()),
                rating: vn.rating,
                source,
                record: Some(ProviderRecord::Vndb(vn)),
            })
            .collect()),
        MetadataSource::Bangumi => Ok(bangumi
            .search_by_title(query, limit)
            .await?
            .into_iter()
            .map(|subject| ProviderSearchResult {
                id: subject.id.to_string(),
                title: subject.name.clone(),
                title_original: subject.name_cn.clone(),
                search_titles: ProviderRecord::Bangumi(subject.clone()).search_titles(),
                developer: None,
                rating: subject.rating.as_ref().map(|rating| rating.score),
                source,
                record: Some(ProviderRecord::Bangumi(subject)),
            })
            .collect()),
        MetadataSource::Dlsite => {
            let Some(rj_code) = extract_rj_code(query) else {
                return Ok(Vec::new());
            };
            Ok(dlsite
                .get_by_rj_code(&rj_code)
                .await?
                .into_iter()
                .map(|product| ProviderSearchResult {
                    id: product.product_id.clone(),
                    title: product
                        .product_name
                        .clone()
                        .unwrap_or_else(|| product.product_id.clone()),
                    title_original: None,
                    search_titles: ProviderRecord::Dlsite(product.clone()).search_titles(),
                    developer: product.maker_name.clone(),
                    rating: product.rate_average,
                    source,
                    record: Some(ProviderRecord::Dlsite(product)),
                })
                .collect())
        }
    }
}

pub async fn fetch_record(
    source: MetadataSource,
    external_id: &str,
    vndb: &VndbClient,
    bangumi: &BangumiClient,
    dlsite: &DlsiteClient,
) -> Result<Option<ProviderRecord>, String> {
    match source {
        MetadataSource::Vndb => Ok(vndb.get_by_id(external_id).await?.map(ProviderRecord::Vndb)),
        MetadataSource::Bangumi => match external_id.parse::<u64>() {
            Ok(id) => Ok(bangumi.get_by_id(id).await?.map(ProviderRecord::Bangumi)),
            Err(_) => Ok(None),
        },
        MetadataSource::Dlsite => Ok(dlsite
            .get_by_rj_code(external_id)
            .await?
            .map(ProviderRecord::Dlsite)),
    }
}

pub async fn fetch_linked_records(
    work: &Work,
    vndb: &VndbClient,
    bangumi: &BangumiClient,
    dlsite: &DlsiteClient,
) -> Result<
    (
        Option<ProviderRecord>,
        Option<ProviderRecord>,
        Option<ProviderRecord>,
    ),
    String,
> {
    let detailed = fetch_linked_records_detailed(work, vndb, bangumi, dlsite).await;
    Ok((
        detailed.vndb.record,
        detailed.bangumi.record,
        detailed.dlsite.record,
    ))
}

pub async fn fetch_linked_records_detailed(
    work: &Work,
    vndb: &VndbClient,
    bangumi: &BangumiClient,
    dlsite: &DlsiteClient,
) -> LinkedProviderRecords {
    LinkedProviderRecords {
        vndb: fetch_linked_record(
            MetadataSource::Vndb,
            work.vndb_id.clone(),
            vndb,
            bangumi,
            dlsite,
        )
        .await,
        bangumi: fetch_linked_record(
            MetadataSource::Bangumi,
            work.bangumi_id.clone(),
            vndb,
            bangumi,
            dlsite,
        )
        .await,
        dlsite: fetch_linked_record(
            MetadataSource::Dlsite,
            work.dlsite_id.clone(),
            vndb,
            bangumi,
            dlsite,
        )
        .await,
    }
}

async fn fetch_linked_record(
    source: MetadataSource,
    external_id: Option<String>,
    vndb: &VndbClient,
    bangumi: &BangumiClient,
    dlsite: &DlsiteClient,
) -> LinkedProviderRecord {
    let Some(external_id) = external_id.filter(|value| !value.trim().is_empty()) else {
        return LinkedProviderRecord {
            source,
            external_id: None,
            record: None,
            state: ProviderLinkState::NotLinked,
            message: None,
        };
    };

    match fetch_record(source, &external_id, vndb, bangumi, dlsite).await {
        Ok(Some(record)) => LinkedProviderRecord {
            source,
            external_id: Some(external_id),
            record: Some(record),
            state: ProviderLinkState::Ready,
            message: None,
        },
        Ok(None) => LinkedProviderRecord {
            source,
            external_id: Some(external_id),
            record: None,
            state: ProviderLinkState::Missing,
            message: Some("Linked record no longer exists on the provider.".to_string()),
        },
        Err(message) => LinkedProviderRecord {
            source,
            external_id: Some(external_id),
            record: None,
            state: classify_provider_error(&message),
            message: Some(message),
        },
    }
}

pub fn classify_provider_error(message: &str) -> ProviderLinkState {
    let normalized = message.to_ascii_lowercase();
    if normalized.contains("401")
        || normalized.contains("403")
        || normalized.contains("oauth")
        || normalized.contains("auth")
        || normalized.contains("forbidden")
        || normalized.contains("unauthorized")
    {
        ProviderLinkState::AuthError
    } else if normalized.contains("429") || normalized.contains("rate limit") {
        ProviderLinkState::RateLimited
    } else {
        ProviderLinkState::TransientError
    }
}

fn extract_rj_code(value: &str) -> Option<String> {
    Regex::new(r"(?i)(RJ\d{6,8})")
        .ok()?
        .captures(value)
        .map(|cap| cap[1].to_uppercase())
}

#[cfg(test)]
mod tests {
    use super::{
        classify_provider_error, extract_rj_code, MetadataField, MetadataSource, ProviderLinkState,
    };

    #[test]
    fn extract_rj_code_finds_embedded_code() {
        assert_eq!(
            extract_rj_code("[RJ123456] bonus pack"),
            Some("RJ123456".to_string())
        );
        assert_eq!(extract_rj_code("no code here"), None);
    }

    #[test]
    fn metadata_field_labels_are_stable() {
        assert_eq!(MetadataField::Title.as_str(), "title");
        assert_eq!(MetadataField::Tags.display_name(), "Tags");
        assert_eq!(MetadataSource::Vndb.display_name(), "VNDB");
    }

    #[test]
    fn classify_provider_error_maps_common_failures() {
        assert_eq!(
            classify_provider_error("Bangumi OAuth error: 401 - expired"),
            ProviderLinkState::AuthError
        );
        assert_eq!(
            classify_provider_error("Rate limited by VNDB (429)"),
            ProviderLinkState::RateLimited
        );
        assert_eq!(
            classify_provider_error("request failed: connection reset"),
            ProviderLinkState::TransientError
        );
    }
}
