//! Related people extraction for metadata sources.

use std::collections::{BTreeSet, HashMap};

use crate::db::queries::people::{
    UpsertCharacterInput, UpsertPersonInput, WorkCharacterLinkInput, WorkCreditInput,
};
use crate::enrichment::bangumi::{
    BangumiCharacterRelation, BangumiCharacterRelationActor, BangumiImages, BangumiPersonRelation,
};

#[derive(Debug, Default)]
pub struct WorkPeopleBundle {
    pub persons: Vec<UpsertPersonInput>,
    pub characters: Vec<UpsertCharacterInput>,
    pub character_links: Vec<WorkCharacterLinkInput>,
    pub credits: Vec<WorkCreditInput>,
}

#[derive(Debug, Default)]
struct PersonAccumulator {
    id: String,
    name: String,
    image_url: Option<String>,
    description: Option<String>,
    roles: BTreeSet<String>,
    bangumi_id: Option<String>,
}

impl PersonAccumulator {
    fn into_input(self) -> UpsertPersonInput {
        let roles = order_roles(self.roles.into_iter().collect());
        UpsertPersonInput {
            id: self.id,
            name: self.name,
            name_original: None,
            vndb_id: None,
            bangumi_id: self.bangumi_id,
            roles_json: serde_json::to_string(&roles).unwrap_or_else(|_| "[]".to_string()),
            image_url: self.image_url,
            description: self.description,
        }
    }
}

#[derive(Debug, Default)]
struct CharacterAccumulator {
    id: String,
    name: String,
    description: Option<String>,
    image_url: Option<String>,
    role: String,
    voice_actor: Option<String>,
}

impl CharacterAccumulator {
    fn into_input(self) -> UpsertCharacterInput {
        UpsertCharacterInput {
            id: self.id,
            vndb_id: None,
            name: self.name,
            name_original: None,
            gender: "unknown".to_string(),
            birthday: None,
            bust: None,
            height: None,
            description: self.description,
            image_url: self.image_url,
            role: self.role,
            voice_actor: self.voice_actor,
            traits_json: "[]".to_string(),
        }
    }
}

pub fn extract_bangumi_people(
    persons: &[BangumiPersonRelation],
    characters: &[BangumiCharacterRelation],
) -> WorkPeopleBundle {
    let mut person_map: HashMap<String, PersonAccumulator> = HashMap::new();
    let mut character_map: HashMap<String, CharacterAccumulator> = HashMap::new();
    let mut character_links: HashMap<String, String> = HashMap::new();
    let mut credits: HashMap<(String, String, String), WorkCreditInput> = HashMap::new();

    for person in persons {
        let person_id = canonical_person_id(person.id);
        let role = normalize_bangumi_role(&person.relation, &person.career);
        merge_person(
            &mut person_map,
            &person_id,
            &person.name,
            best_image(person.images.as_ref()),
            None,
            person.id,
            &role,
        );

        credits.insert(
            (person_id.clone(), role.clone(), String::new()),
            WorkCreditInput {
                person_id,
                role,
                character_name: None,
                notes: None,
            },
        );
    }

    for character in characters {
        let character_id = canonical_character_id(character.id);
        let role = normalize_character_role(&character.relation);
        let voice_actor = character.actors.first().map(|actor| actor.name.clone());
        character_links
            .entry(character_id.clone())
            .or_insert_with(|| role.clone());

        let entry = character_map
            .entry(character_id.clone())
            .or_insert_with(|| CharacterAccumulator {
                id: character_id.clone(),
                name: character.name.clone(),
                description: sanitize_text(character.summary.clone()),
                image_url: best_image(character.images.as_ref()),
                role: role.clone(),
                voice_actor: voice_actor.clone(),
            });

        if entry.description.is_none() {
            entry.description = sanitize_text(character.summary.clone());
        }
        if entry.image_url.is_none() {
            entry.image_url = best_image(character.images.as_ref());
        }
        if entry.voice_actor.is_none() {
            entry.voice_actor = voice_actor.clone();
        }

        for actor in &character.actors {
            let person_id = canonical_person_id(actor.id);
            merge_actor(&mut person_map, &person_id, actor);
            credits.insert(
                (
                    person_id.clone(),
                    "voice_actor".to_string(),
                    character.name.clone(),
                ),
                WorkCreditInput {
                    person_id,
                    role: "voice_actor".to_string(),
                    character_name: Some(character.name.clone()),
                    notes: None,
                },
            );
        }
    }

    WorkPeopleBundle {
        persons: person_map
            .into_values()
            .map(PersonAccumulator::into_input)
            .collect(),
        characters: character_map
            .into_values()
            .map(CharacterAccumulator::into_input)
            .collect(),
        character_links: character_links
            .into_iter()
            .map(|(character_id, role)| WorkCharacterLinkInput { character_id, role })
            .collect(),
        credits: credits.into_values().collect(),
    }
}

fn merge_actor(
    person_map: &mut HashMap<String, PersonAccumulator>,
    person_id: &str,
    actor: &BangumiCharacterRelationActor,
) {
    merge_person(
        person_map,
        person_id,
        &actor.name,
        best_image(actor.images.as_ref()),
        sanitize_text(actor.short_summary.clone()),
        actor.id,
        "voice_actor",
    );
}

fn merge_person(
    person_map: &mut HashMap<String, PersonAccumulator>,
    person_id: &str,
    name: &str,
    image_url: Option<String>,
    description: Option<String>,
    bangumi_id: u64,
    role: &str,
) {
    let entry = person_map
        .entry(person_id.to_string())
        .or_insert_with(|| PersonAccumulator {
            id: person_id.to_string(),
            name: name.to_string(),
            image_url: image_url.clone(),
            description: description.clone(),
            roles: BTreeSet::new(),
            bangumi_id: Some(bangumi_id.to_string()),
        });

    if entry.image_url.is_none() {
        entry.image_url = image_url;
    }
    if entry.description.is_none() {
        entry.description = description;
    }
    entry.roles.insert(role.to_string());
}

fn normalize_bangumi_role(relation: &str, career: &[String]) -> String {
    let value = relation.to_lowercase();
    if value.contains("声优") || value.contains("聲優") || career.iter().any(|item| item == "seiyu")
    {
        "voice_actor".to_string()
    } else if value.contains("原画")
        || value.contains("插画")
        || value.contains("插畫")
        || value.contains("作画")
        || value.contains("畫")
        || career
            .iter()
            .any(|item| item == "illustrator" || item == "mangaka" || item == "artist")
    {
        "artist".to_string()
    } else if value.contains("剧本")
        || value.contains("劇本")
        || value.contains("脚本")
        || career.iter().any(|item| item == "writer")
    {
        "writer".to_string()
    } else if value.contains("音乐") || value.contains("音樂") || value.contains("作曲") {
        "composer".to_string()
    } else if value.contains("导演") || value.contains("監督") || value.contains("导") {
        "director".to_string()
    } else {
        "staff".to_string()
    }
}

fn normalize_character_role(relation: &str) -> String {
    if relation.contains("主角") || relation.contains("主要") {
        "main".to_string()
    } else {
        "side".to_string()
    }
}

fn best_image(images: Option<&BangumiImages>) -> Option<String> {
    images.and_then(|images| {
        images
            .large
            .clone()
            .filter(|url| !url.is_empty())
            .or_else(|| images.medium.clone().filter(|url| !url.is_empty()))
            .or_else(|| images.grid.clone().filter(|url| !url.is_empty()))
            .or_else(|| images.small.clone().filter(|url| !url.is_empty()))
    })
}

fn canonical_person_id(bangumi_id: u64) -> String {
    format!("bgm-person-{bangumi_id}")
}

fn canonical_character_id(bangumi_id: u64) -> String {
    format!("bgm-character-{bangumi_id}")
}

fn sanitize_text(value: Option<String>) -> Option<String> {
    value
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn order_roles(mut roles: Vec<String>) -> Vec<String> {
    roles.sort_by_key(|role| match role.as_str() {
        "voice_actor" => 0,
        "director" => 1,
        "writer" => 2,
        "artist" => 3,
        "composer" => 4,
        _ => 5,
    });
    roles
}

#[cfg(test)]
mod tests {
    use super::extract_bangumi_people;
    use crate::enrichment::bangumi::{
        BangumiCharacterRelation, BangumiCharacterRelationActor, BangumiImages,
        BangumiPersonRelation,
    };

    #[test]
    fn extract_bangumi_people_builds_people_and_character_links() {
        let persons = vec![BangumiPersonRelation {
            images: Some(BangumiImages {
                large: Some("https://example.com/p.jpg".to_string()),
                medium: None,
                small: None,
                grid: None,
            }),
            name: "Artist".to_string(),
            relation: "原画".to_string(),
            career: vec!["illustrator".to_string()],
            person_type: 1,
            id: 10,
            eps: None,
        }];
        let characters = vec![BangumiCharacterRelation {
            images: Some(BangumiImages {
                large: Some("https://example.com/c.jpg".to_string()),
                medium: None,
                small: None,
                grid: None,
            }),
            name: "Heroine".to_string(),
            summary: Some("desc".to_string()),
            relation: "主角".to_string(),
            actors: vec![BangumiCharacterRelationActor {
                images: None,
                name: "VA".to_string(),
                short_summary: None,
                career: vec!["seiyu".to_string()],
                id: 12,
                actor_type: 1,
                locked: false,
            }],
            character_type: 1,
            id: 11,
        }];

        let bundle = extract_bangumi_people(&persons, &characters);

        assert_eq!(bundle.persons.len(), 2);
        assert_eq!(bundle.characters.len(), 1);
        assert_eq!(bundle.character_links.len(), 1);
        assert_eq!(bundle.credits.len(), 2);
        assert!(bundle
            .persons
            .iter()
            .any(|person| person.roles_json.contains("artist")));
        assert!(bundle
            .persons
            .iter()
            .any(|person| person.roles_json.contains("voice_actor")));
    }
}
