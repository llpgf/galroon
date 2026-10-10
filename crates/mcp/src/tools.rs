//! MCP tools over the Core API. Reads, revision-checked catalog edits and organize previews only:
//! nothing here approves or executes a file plan, quarantines, merges/splits works or manages access.
use crate::client::{Core, UNREACHABLE};
use crate::config::{self, R};
use galroon_core::collection_query::normalized;
use serde_json::{json, Map, Value};
use std::sync::Arc;

const MAX_OUTPUT: usize = 100_000;
const STATUSES: [&str; 5] = ["backlog", "playing", "completed", "on_hold", "dropped"];

pub struct Tools {
    url: Option<String>,
    fixed: bool,
    core: tokio::sync::Mutex<Option<Arc<Core>>>,
}

impl Tools {
    /// Discovers the Core lazily and again after it restarts on a new port.
    pub fn new(url: Option<String>) -> Self {
        Self { url, fixed: false, core: Default::default() }
    }

    pub fn fixed(core: Core) -> Self {
        Self { url: None, fixed: true, core: tokio::sync::Mutex::new(Some(Arc::new(core))) }
    }

    async fn core(&self) -> R<Arc<Core>> {
        let mut slot = self.core.lock().await;
        if let Some(core) = slot.as_ref() {
            return Ok(core.clone());
        }
        let credential = config::load_credential()?;
        let endpoint = config::endpoint(self.url.as_deref()).await?;
        if let Some(found) = &endpoint.library_id {
            if !credential.library_id.is_empty() && *found != credential.library_id {
                return Err("Galroon has a different collection open than the one this MCP server was paired with. Switch collections in Galroon or pair again.".into());
            }
        }
        let core = Arc::new(Core::new(endpoint.url, credential.token, Some(credential.library_id))?);
        *slot = Some(core.clone());
        Ok(core)
    }

    pub async fn call(&self, name: &str, args: &Value) -> R<Value> {
        let core = self.core().await?;
        match dispatch(&core, name, args).await {
            // A refused connection never reached Core, so one retry against a rediscovered port is safe.
            Err(error) if error.starts_with(UNREACHABLE) && !self.fixed => {
                *self.core.lock().await = None;
                let core = self.core().await?;
                dispatch(&core, name, args).await
            }
            result => result,
        }
    }
}

fn tool(name: &str, title: &str, description: &str, read_only: bool, properties: Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": {"type": "object", "properties": properties, "required": required, "additionalProperties": false},
        "annotations": {"title": title, "readOnlyHint": read_only, "destructiveHint": false, "idempotentHint": read_only, "openWorldHint": name == "search_vndb"},
    })
}

pub fn definitions() -> Vec<Value> {
    let cursor = json!({"type": "string", "description": "Opaque `next` value from the previous page"});
    let revision = |what: &str| json!({"type": "integer", "description": format!("Current {what} revision, read just before editing")});
    vec![
        tool("galroon_overview", "Collection overview", "Collection name, counts (works, resources, unassigned resources, files), open issue count and this connection's permissions. Start here.", true, json!({}), &[]),
        tool("search_collection", "Search collection", "Search and filter works in the collection. Returns up to 60 compact works per page plus totals.", true, json!({
            "query": {"type": "string", "description": "Fuzzy title/alias/studio search"},
            "status": {"type": "string", "enum": ["all", "favorite", "backlog", "playing", "completed", "on_hold", "dropped"]},
            "sort": {"type": "string", "enum": ["added", "title", "newest", "oldest"]},
            "tags": {"type": "array", "items": {"type": "string"}, "description": "Match works with any of these tags (names). Personal tags use their custom:<id> key."},
            "exclude_tags": {"type": "array", "items": {"type": "string"}},
            "studio": {"type": "string"}, "year": {"type": "string"},
            "availability": {"type": "string", "enum": ["available", "missing", "unverified", "offline", "unknown", "no_resources"]},
            "cursor": cursor,
        }), &[]),
        tool("get_work", "Get work", "Full details of one work, with its resources (files/folders) and editions.", true, json!({"work_id": {"type": "string"}}), &["work_id"]),
        tool("list_resources", "List resources", "Scanned folders/files. Use status=unmatched to find resources not yet matched to a work.", true, json!({
            "status": {"type": "string", "enum": ["matched", "unmatched"]},
            "query": {"type": "string", "description": "Search by path/title"},
            "root_id": {"type": "string", "description": "Limit to one source folder"},
            "cursor": cursor,
        }), &[]),
        tool("list_resource_files", "List resource files", "File names, sizes and availability inside one resource (relative to its source folder), 60 per page. Use the names, brands, dates and IDs in them to identify the work before search_vndb.", true, json!({
            "resource_id": {"type": "string"},
            "query": {"type": "string", "description": "Filter by file name"},
            "cursor": cursor,
        }), &["resource_id"]),
        tool("list_issues", "List issues", "Matching and source issues that need attention (unmatched, offline sources, unscanned folders...).", true, json!({
            "state": {"type": "string", "enum": ["open", "deferred", "resolved"]},
            "cursor": {"type": "integer", "description": "`next` value from the previous page"},
        }), &[]),
        tool("list_jobs", "List background jobs", "Recent scan/match/organize jobs with their state and summaries.", true, json!({}), &[]),
        tool("search_vndb", "Search VNDB", "Search VNDB for match candidates. Pass resource_id to let Galroon use that resource's file names as hints.", true, json!({
            "query": {"type": "string"}, "resource_id": {"type": "string"},
        }), &["query"]),
        tool("match_resource", "Match resource to work", "Assign a scanned resource to a work. Give work_id for a work already in the collection, or vndb_id (e.g. v17) plus the query that found it; the work is added if needed. Reversible in Galroon.", false, json!({
            "resource_id": {"type": "string"},
            "revision": revision("resource"),
            "work_id": {"type": "string"},
            "vndb_id": {"type": "string"},
            "query": {"type": "string", "description": "The search_vndb query that returned vndb_id"},
            "edition": {"type": "string", "description": "Edition label, e.g. 'Steam' or 'Limited edition'. Defaults to 'Unclassified edition'."},
        }), &["resource_id", "revision"]),
        tool("update_work", "Update work", "Set play status, favorite or personal notes of a work.", false, json!({
            "work_id": {"type": "string"},
            "revision": revision("work"),
            "status": {"type": "string", "enum": STATUSES},
            "favorite": {"type": "boolean"},
            "notes": {"type": "string"},
        }), &["work_id", "revision"]),
        tool("list_lists", "List curated lists", "The user's manual lists.", true, json!({"cursor": cursor}), &[]),
        tool("get_list", "Get list", "A manual list with its entries (50 per page) and revision.", true, json!({
            "list_id": {"type": "string"},
            "cursor": {"type": "integer", "description": "`next_after` from the previous page"},
            "revision": {"type": "integer", "description": "The list `revision` from the first page; required with cursor"},
        }), &["list_id"]),
        tool("create_list", "Create list", "Create a new manual list.", false, json!({"name": {"type": "string"}}), &["name"]),
        tool("add_to_list", "Add works to list", "Add collection works to a manual list. Works already in the list are skipped.", false, json!({
            "list_id": {"type": "string"},
            "revision": revision("list"),
            "work_ids": {"type": "array", "items": {"type": "string"}, "minItems": 1, "maxItems": 60},
        }), &["list_id", "revision", "work_ids"]),
        tool("list_plans", "List file plans", "Organize/quarantine plans and their states.", true, json!({}), &[]),
        tool("preview_organize", "Preview organize", "Draft a plan that would move a matched resource into a managed folder as <Work [id]>/<Edition>. Files are hashed but NOT moved: the user must review, approve and execute the plan in Galroon. Hashing large resources can take minutes. Calling it again for the same resource and destination returns the existing ready plan instead of creating a duplicate.", false, json!({
            "resource_id": {"type": "string"},
            "destination": {"type": "string", "description": "Existing managed folder path"},
        }), &["resource_id", "destination"]),
    ]
}

/// A retried preview (for example after a client timeout while Core kept hashing) must not leave a
/// second plan: reuse a ready organize plan that covers exactly this resource's files under `destination`.
async fn ready_plan(core: &Core, resource: &str, destination: &str) -> R<Option<Value>> {
    let plans = core.get("/plans", &[]).await?;
    let candidates: Vec<&Value> = plans.as_array().map(|a| a.iter().filter(|p| p["kind"] == "organize" && p["state"] == "ready").collect()).unwrap_or_default();
    if candidates.is_empty() {
        return Ok(None);
    }
    let mut files = std::collections::BTreeSet::new();
    let mut cursor: Option<String> = None;
    for _ in 0..100 {
        let query: Vec<(&str, String)> = cursor.iter().map(|c| ("before", c.clone())).collect();
        let page = core.get(&format!("/resources/{resource}/members/page"), &query).await?;
        files.extend(page["items"].as_array().into_iter().flatten().filter_map(|f| f["id"].as_str().map(str::to_owned)));
        match page["next"].as_str() {
            Some(next) => cursor = Some(next.to_owned()),
            None => break,
        }
    }
    let root = std::path::Path::new(destination);
    Ok(candidates.into_iter().find(|plan| {
        let items = plan["items"].as_array().map(Vec::as_slice).unwrap_or_default();
        !items.is_empty()
            && items.len() == files.len()
            && items.iter().all(|i| i["file_id"].as_str().is_some_and(|id| files.contains(id)) && i["target"].as_str().is_some_and(|t| std::path::Path::new(t).starts_with(root)))
    }).cloned())
}

pub fn exists(name: &str) -> bool {
    definitions().iter().any(|t| t["name"] == name)
}

fn opt<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args[key].as_str().map(str::trim).filter(|s| !s.is_empty())
}

fn need<'a>(args: &'a Value, key: &str) -> R<&'a str> {
    opt(args, key).ok_or_else(|| format!("{key} is required"))
}

fn need_i64(args: &Value, key: &str) -> R<i64> {
    args[key].as_i64().ok_or_else(|| format!("{key} is required (integer)"))
}

fn segment(id: &str) -> R<&str> {
    if id.is_empty() || id.len() > 128 || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b':') {
        return Err(format!("Invalid id: {id}"));
    }
    Ok(id)
}

fn strings(args: &Value, key: &str) -> Vec<String> {
    args[key].as_array().map(|a| a.iter().filter_map(|v| v.as_str()).map(str::to_owned).collect()).unwrap_or_default()
}

fn pick(v: &Value, keys: &[&str]) -> Value {
    let mut out = Map::new();
    for key in keys {
        if let Some(value) = v.get(*key).filter(|x| !x.is_null()) {
            out.insert((*key).to_owned(), value.clone());
        }
    }
    Value::Object(out)
}

fn names(v: &Value, limit: usize) -> Value {
    json!(galroon_core::collection_query::tag_names(v).into_iter().take(limit).collect::<Vec<_>>())
}

fn clip(s: &str, limit: usize) -> String {
    if s.chars().count() <= limit {
        return s.to_owned();
    }
    format!("{}…", s.chars().take(limit).collect::<String>())
}

/// Bounds strings and arrays so one response cannot flood the model context.
fn bounded(v: &Value) -> Value {
    match v {
        Value::String(s) => json!(clip(s, 600)),
        Value::Array(items) => {
            let mut out: Vec<Value> = items.iter().take(80).map(bounded).collect();
            if items.len() > 80 {
                out.push(json!(format!("… {} more", items.len() - 80)));
            }
            Value::Array(out)
        }
        Value::Object(map) => Value::Object(map.iter().filter(|(k, _)| k.as_str() != "cover" && k.as_str() != "source_json").map(|(k, v)| (k.clone(), bounded(v))).collect()),
        other => other.clone(),
    }
}

fn work(v: &Value) -> Value {
    let mut out = pick(v, &["id", "title", "original_title", "vndb_id", "developer", "released", "status", "favorite", "resources", "revision"]);
    out["tags"] = names(&v["tags"], 12);
    if let Some(notes) = v["notes"].as_str().filter(|s| !s.is_empty()) {
        out["notes"] = json!(clip(notes, 200));
    }
    out
}

fn resource(v: &Value) -> Value {
    let mut out = pick(v, &["id", "title", "path", "kind", "files", "bytes", "missing", "work_id", "primary_work_title", "release", "root_id", "revision", "auto_match_reason"]);
    if v["bindings"].as_array().is_some_and(|b| b.len() > 1) {
        out["bindings"] = bounded(&v["bindings"]);
    }
    out
}

fn candidate(v: &Value) -> Value {
    let mut out = pick(v, &["id", "title", "alttitle", "released", "match"]);
    out["developers"] = json!(v["developers"].as_array().map(|d| d.iter().filter_map(|x| x["name"].as_str()).collect::<Vec<_>>()).unwrap_or_default());
    out["tags"] = names(&v["tags"], 8);
    if let Some(d) = v["description"].as_str() {
        out["description"] = json!(clip(d, 240));
    }
    out
}

fn items(v: &Value, f: fn(&Value) -> Value) -> Value {
    json!(v.as_array().map(|a| a.iter().map(f).collect::<Vec<_>>()).unwrap_or_default())
}

pub fn render(v: &Value) -> String {
    let text = serde_json::to_string(v).unwrap_or_default();
    if text.len() <= MAX_OUTPUT {
        return text;
    }
    let mut end = MAX_OUTPUT;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n[Truncated: response exceeded {MAX_OUTPUT} bytes. Narrow the query or use paging.]", &text[..end])
}

async fn dispatch(core: &Core, name: &str, args: &Value) -> R<Value> {
    match name {
        "galroon_overview" => {
            let (context, dashboard, me, issues) = tokio::try_join!(core.get("/context", &[]), core.get("/dashboard", &[]), core.get("/access/me", &[]), core.get("/issues", &[]))?;
            Ok(json!({
                "collection": pick(&context["library"], &["id", "name"]),
                "core_version": context["core"]["version"],
                "counts": pick(&dashboard, &["works", "resources", "unassigned", "files", "bytes", "roots"]),
                "open_issues_first_page": issues["items"].as_array().map_or(0, Vec::len),
                "more_issues": !issues["next"].is_null(),
                "can_write": me["can_write"],
            }))
        }
        "search_collection" => {
            let mut tags: Vec<String> = Vec::new();
            for tag in strings(args, "tags").iter().chain(strings(args, "exclude_tags").iter()) {
                let key = if tag.starts_with("custom:") { tag.clone() } else { format!("value:{}", normalized(tag)) };
                if !tags.contains(&key) {
                    tags.push(key);
                }
            }
            let excluded: Vec<String> = strings(args, "exclude_tags").iter().map(|t| if t.starts_with("custom:") { t.clone() } else { format!("value:{}", normalized(t)) }).collect();
            let value = |key: &str| opt(args, key).map(|v| format!("value:{}", normalized(v))).unwrap_or_default();
            let query = json!({
                "query": opt(args, "query").unwrap_or(""),
                "status": opt(args, "status").unwrap_or("all"),
                "sort": opt(args, "sort").unwrap_or("added"),
                "filters": {"tags": tags, "excludedTags": excluded, "studio": value("studio"), "year": value("year"), "availability": opt(args, "availability").unwrap_or("")},
            });
            let mut params = vec![("query", query.to_string())];
            if let Some(cursor) = opt(args, "cursor") {
                params.push(("before", cursor.to_owned()));
            }
            let page = core.get("/collection", &params).await?;
            Ok(json!({"total": page["total"], "matching": page["filtered"], "status_counts": page["status_counts"], "items": items(&page["items"], work), "next": page["next"]}))
        }
        "get_work" => {
            let id = segment(need(args, "work_id")?)?;
            let (detail_path, assets_path) = (format!("/works/{id}"), format!("/works/{id}/assets"));
            let (detail, assets) = tokio::try_join!(core.get(&detail_path, &[]), core.get(&assets_path, &[]))?;
            Ok(json!({"work": bounded(&detail), "assets": bounded(&assets)}))
        }
        "list_resources" => {
            let mut params = Vec::new();
            for (arg, key) in [("status", "status"), ("query", "query"), ("root_id", "root_id"), ("cursor", "before")] {
                if let Some(v) = opt(args, arg) {
                    params.push((key, v.to_owned()));
                }
            }
            let page = core.get("/resources/page", &params).await?;
            Ok(json!({"total": page["total"], "items": items(&page["items"], resource), "next": page["next"]}))
        }
        "list_resource_files" => {
            let id = segment(need(args, "resource_id")?)?;
            let mut params = Vec::new();
            for (arg, key) in [("query", "query"), ("cursor", "before")] {
                if let Some(v) = opt(args, arg) {
                    params.push((key, v.to_owned()));
                }
            }
            let page = core.get(&format!("/resources/{id}/members/page"), &params).await?;
            let file = |v: &Value| pick(v, &["relative", "size", "availability"]);
            Ok(json!({"total": page["total"], "items": bounded(&json!(page["items"].as_array().map(|a| a.iter().map(file).collect::<Vec<_>>()).unwrap_or_default())), "next": page["next"]}))
        }
        "list_issues" => {
            let mut params = vec![("state", opt(args, "state").unwrap_or("open").to_owned())];
            if let Some(before) = args["cursor"].as_i64() {
                params.push(("before", before.to_string()));
            }
            let page = core.get("/issues", &params).await?;
            Ok(json!({"items": items(&page["items"], |i| pick(i, &["id", "category", "kind", "title", "path", "reason", "source", "resource_id", "state", "revision"])), "next": page["next"]}))
        }
        "list_jobs" => {
            let jobs = core.get("/jobs", &[]).await?;
            let recent: Vec<Value> = jobs.as_array().map(|a| a.iter().take(20).map(|j| pick(j, &["id", "kind", "state", "message", "processed", "discovered", "errors", "summary", "created", "updated"])).collect()).unwrap_or_default();
            Ok(json!({"jobs": recent}))
        }
        "search_vndb" => {
            let mut body = json!({"query": need(args, "query")?});
            if let Some(rid) = opt(args, "resource_id") {
                body["resource_id"] = json!(segment(rid)?);
            }
            let found = core.post("/vndb/search", &body).await?;
            let mut out = pick(&found, &["query", "warning", "note"]);
            out["results"] = items(&found["results"], candidate);
            Ok(out)
        }
        "match_resource" => {
            let rid = segment(need(args, "resource_id")?)?;
            let revision = need_i64(args, "revision")?;
            let work_id = match (opt(args, "work_id"), opt(args, "vndb_id")) {
                (Some(id), None) => segment(id)?.to_owned(),
                (None, Some(vndb)) => {
                    let query = need(args, "query")?;
                    let found = core.post("/vndb/search", &json!({"query": query, "resource_id": rid})).await?;
                    let c = found["results"].as_array().and_then(|r| r.iter().find(|c| c["id"] == vndb)).ok_or_else(|| format!("{vndb} was not among the search_vndb results for {query:?}; search again"))?;
                    let join = |key: &str| c[key].as_array().map(|a| a.iter().filter_map(|x| x["name"].as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default();
                    // Same fields the desktop match dialog stores; Core reuses an existing work with this VNDB id.
                    let created = core.post("/works", &json!({
                        "title": c["title"], "original_title": c["alttitle"].as_str().filter(|s| !s.is_empty()).or(c["title"].as_str()),
                        "vndb_id": vndb, "description": c["description"].as_str().unwrap_or(""), "cover": c["image"]["url"].as_str().unwrap_or(""),
                        "developer": join("developers"), "released": c["released"].as_str().unwrap_or(""),
                        "tags": c["tags"].as_array().map(|a| a.iter().filter_map(|x| x["name"].as_str()).collect::<Vec<_>>()).unwrap_or_default(),
                    })).await?;
                    created["id"].as_str().ok_or("Core did not return the work id")?.to_owned()
                }
                _ => return Err("Give exactly one of work_id or vndb_id".into()),
            };
            let release = opt(args, "edition").unwrap_or("Unclassified edition");
            let result = core.post(&format!("/resources/{rid}/bind"), &json!({"work_id": work_id, "release": release, "revision": revision})).await?;
            Ok(json!({"matched": true, "resource_id": rid, "work_id": work_id, "edition": release, "result": bounded(&result)}))
        }
        "update_work" => {
            let id = segment(need(args, "work_id")?)?;
            // Core merges the whole body into the work's overrides, so send only these fields.
            let mut body = json!({"revision": need_i64(args, "revision")?});
            if let Some(status) = opt(args, "status") {
                if !STATUSES.contains(&status) {
                    return Err(format!("status must be one of {STATUSES:?}"));
                }
                body["status"] = json!(status);
            }
            if let Some(favorite) = args["favorite"].as_bool() {
                body["favorite"] = json!(favorite);
            }
            if let Some(notes) = args["notes"].as_str() {
                body["notes"] = json!(notes);
            }
            if body.as_object().is_some_and(|b| b.len() == 1) {
                return Err("Nothing to change: give status, favorite or notes".into());
            }
            core.post(&format!("/works/{id}"), &body).await
        }
        "list_lists" => {
            let params: Vec<_> = opt(args, "cursor").map(|c| ("after", c.to_owned())).into_iter().collect();
            core.get("/lists", &params).await.map(|v| bounded(&v))
        }
        "get_list" => {
            let id = segment(need(args, "list_id")?)?;
            // Core pins continuation pages to the first page's revision.
            let mut params = Vec::new();
            if let Some(after) = args["cursor"].as_i64() {
                params.push(("after", after.to_string()));
                params.push(("revision", need_i64(args, "revision").map_err(|_| "revision from the first page is required with cursor".to_string())?.to_string()));
            }
            core.get(&format!("/lists/{id}"), &params).await.map(|v| bounded(&v))
        }
        "create_list" => {
            let id = uuid::Uuid::new_v4().to_string();
            let edit = json!({"request_id": uuid::Uuid::new_v4().to_string(), "revision": 0, "edit": {"action": "create", "name": need(args, "name")?}});
            let result = core.post(&format!("/lists/{id}"), &edit).await?;
            Ok(json!({"list_id": id, "result": result}))
        }
        "add_to_list" => {
            let id = segment(need(args, "list_id")?)?;
            let ids = strings(args, "work_ids");
            if ids.is_empty() || ids.len() > 60 {
                return Err("work_ids must contain 1 to 60 ids".into());
            }
            let found = core.get("/works/lookup", &[("ids", json!(ids).to_string()), ("local", "true".into())]).await?;
            if found["missing"].as_array().is_some_and(|m| !m.is_empty()) {
                return Err(format!("Unknown or merged work ids: {}", found["missing"]));
            }
            let members: Vec<Value> = found["works"].as_array().or(found["items"].as_array()).ok_or("Unexpected lookup response")?.iter().map(|w| json!({"work_key": format!("local:{}", w["id"].as_str().unwrap_or("")), "title": w["title"]})).collect();
            let edit = json!({"request_id": uuid::Uuid::new_v4().to_string(), "revision": need_i64(args, "revision")?, "edit": {"action": "add", "members": members}});
            core.post(&format!("/lists/{id}"), &edit).await
        }
        "list_plans" => {
            let plans = core.get("/plans", &[]).await?;
            Ok(json!({"plans": plans.as_array().map(|a| a.iter().take(30).map(|p| pick(p, &["id", "kind", "state", "created"])).collect::<Vec<_>>()).unwrap_or_default()}))
        }
        "preview_organize" => {
            let (resource, destination) = (segment(need(args, "resource_id")?)?, need(args, "destination")?);
            let plan = match ready_plan(core, resource, destination).await? {
                Some(plan) => plan,
                None => core.post_long("/plans/organize", &json!({"resource_id": resource, "destination": destination})).await?,
            };
            let moves = items(&plan["items"], |i| pick(i, &["source", "target", "size"]));
            Ok(json!({
                "plan": {"id": plan["id"], "kind": plan["kind"], "state": plan["state"], "file_count": plan["items"].as_array().map_or(0, Vec::len), "moves": bounded(&moves)},
                "next_step": "Nothing has moved. Ask the user to review and approve this plan in Galroon > Organize/Plans.",
            }))
        }
        _ => Err(format!("Unknown tool: {name}")),
    }
}
