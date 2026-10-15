//! One role boundary for all JSON responses, including nested messages and API errors.
use axum::{body::{Body,to_bytes},http::{StatusCode,header},response::{Response,IntoResponse},Json};
use serde_json::{Value,json};
use crate::App;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub enum WebAccess{Deny,Project,Unchanged}
pub const WEB_ROUTES:&[(&str,WebAccess)]=&[
 ("/api/health",WebAccess::Unchanged),
 ("/api/context",WebAccess::Unchanged),
 ("/api/updates",WebAccess::Unchanged),
 ("/api/updates/check",WebAccess::Deny),
 ("/api/updates/settings",WebAccess::Deny),
 ("/api/access/status",WebAccess::Unchanged),
 ("/api/access/me",WebAccess::Unchanged),
 ("/api/access/pairing-codes",WebAccess::Deny),
 ("/api/access/network",WebAccess::Deny),
 ("/api/access/devices",WebAccess::Deny),
 ("/api/changes",WebAccess::Unchanged),
 ("/api/dashboard",WebAccess::Unchanged),
 ("/api/roots",WebAccess::Project),
 ("/api/roots/{id}/watch",WebAccess::Project),
 ("/api/jobs",WebAccess::Project),
 ("/api/jobs/{id}",WebAccess::Project),
 ("/api/timeline",WebAccess::Deny),
 ("/api/issues/{id}/task",WebAccess::Deny),
 ("/api/resources/page",WebAccess::Project),
 ("/api/resources",WebAccess::Project),
 ("/api/suggestions",WebAccess::Project),
 ("/api/resources/{id}/suggestions",WebAccess::Project),
 ("/api/works",WebAccess::Unchanged),
 ("/api/works/lookup",WebAccess::Unchanged),
 ("/api/works/{id}/assets",WebAccess::Project),
 ("/api/collection",WebAccess::Unchanged),
 ("/api/issues",WebAccess::Deny),
 ("/api/diagnostics",WebAccess::Deny),
 ("/api/vns/{id}/relationship-decisions",WebAccess::Deny),
 ("/api/vns/{id}/relationship-history",WebAccess::Deny),
 ("/api/vns/{id}/relationship-corrections",WebAccess::Deny),
 ("/api/vns/{id}/relationship-corrections/history",WebAccess::Deny),
 ("/api/relationship-candidates/{id}",WebAccess::Deny),
 ("/api/entities/{id}/manual-works",WebAccess::Unchanged),
 ("/api/entities/{id}/field-decisions",WebAccess::Deny),
 ("/api/entities/{id}/field-history",WebAccess::Deny),
 ("/api/works/{id}",WebAccess::Unchanged),
 ("/api/works/{id}/edition-options",WebAccess::Unchanged),
 ("/api/releases/labels",WebAccess::Unchanged),
 ("/api/releases",WebAccess::Unchanged),
 ("/api/releases/{id}",WebAccess::Unchanged),
 ("/api/releases/{id}/membership",WebAccess::Unchanged),
 ("/api/resources/{id}/members",WebAccess::Project),
 ("/api/resources/{id}/members/page",WebAccess::Project),
 ("/api/resources/{id}/matching-history",WebAccess::Deny),
 ("/api/matching",WebAccess::Project),
 ("/api/works/{id}/metadata",WebAccess::Unchanged),
 ("/api/works/{id}/exploration",WebAccess::Unchanged),
 ("/api/people/{id}",WebAccess::Unchanged),
 ("/api/characters/{id}",WebAccess::Unchanged),
 ("/api/companies/{id}",WebAccess::Unchanged),
 ("/api/discover",WebAccess::Unchanged),
 ("/api/discover/manual",WebAccess::Unchanged),
 ("/api/artwork",WebAccess::Unchanged),
 ("/api/artwork/cache",WebAccess::Deny),
 ("/api/artwork/offline",WebAccess::Deny),
 ("/api/vns/{id}/exploration",WebAccess::Unchanged),
 ("/api/lists",WebAccess::Unchanged),
 ("/api/lists/{id}",WebAccess::Unchanged),
 ("/api/vns/{id}/related-tags",WebAccess::Unchanged),
 ("/api/works/{id}/related-tags",WebAccess::Unchanged),
 ("/api/entity-tags/{id}",WebAccess::Unchanged),
 ("/api/custom-tags",WebAccess::Unchanged),
 ("/api/custom-tags/{id}/membership",WebAccess::Deny),
 ("/api/smart-lists/options",WebAccess::Unchanged),
 ("/api/smart-lists/{id}/results",WebAccess::Project),
 ("/api/smart-lists",WebAccess::Unchanged),
 ("/api/smart-lists/{id}",WebAccess::Unchanged),
 ("/api/works/{id}/split-preview",WebAccess::Deny),
 ("/api/backups",WebAccess::Deny),
 ("/api/plans",WebAccess::Deny),
 ("/api/plans/{id}",WebAccess::Deny),
 ("/api/duplicate-checks/{id}",WebAccess::Deny),
 ("/api/quarantine",WebAccess::Deny),
 ("/api/resources/{id}/acquire-preview",WebAccess::Deny),
 ("/api/archive/capabilities",WebAccess::Unchanged),
 ];
pub fn access(route:&str)->WebAccess{WEB_ROUTES.iter().find(|(path,_)|*path==route).map(|(_,access)|*access).unwrap_or(WebAccess::Deny)}
pub fn denied()->Response{(StatusCode::FORBIDDEN,Json(json!({"error":"Desktop access required"}))).into_response()}
// Unknown absolute paths (including Windows paths on a Unix Core) fail closed. Redacting the
// entire string avoids leaking the tail of a filename containing spaces through an error message.
fn absolute(value:&str)->bool{
 static PATH:std::sync::OnceLock<regex::Regex>=std::sync::OnceLock::new();
 value.starts_with('/')||value.to_ascii_lowercase().contains("file://")||PATH.get_or_init(||regex::Regex::new(r#"(?:(?:^|[^A-Za-z0-9])[A-Za-z]:[\\/]|\\\\|(?:^|[\s\"'(:=])/(?:[^/]|$))"#).unwrap()).is_match(value)
}
fn scrub(value:&mut Value,roots:&[String]){
 match value{
  Value::String(text)=>{if roots.iter().any(|root|!root.is_empty()&&text.contains(root))||absolute(text){*text="[redacted path]".into();}},
  Value::Array(rows)=>for row in rows{scrub(row,roots);},
  Value::Object(row)=>{row.retain(|key,_|!absolute(key)&&!roots.iter().any(|root|!root.is_empty()&&key.contains(root)));for value in row.values_mut(){scrub(value,roots);}},_=>{}
 }
}
fn project(value:&mut Value,route:&str){
 match value{
  Value::Array(rows)=>for row in rows{project(row,route);},
  Value::Object(row)=>{
   row.remove("root_path");row.remove("current_path");
   if route=="/api/roots"{row.retain(|key,_|matches!(key.as_str(),"id"|"label"|"role"|"online"|"missing"|"unverified"));}
   if route=="/api/resources/{id}/members"&&row.contains_key("path"){if let Some(relative)=row.get("relative").cloned(){row.insert("path".into(),relative);}else{row.remove("path");}}
   if route=="/api/roots/{id}/watch"{row.remove("path");}
   for value in row.values_mut(){project(value,route);}
  },_=>{}
 }
}
pub async fn response(a:&App,route:&str,response:Response)->Response{
 let is_json=response.headers().get(header::CONTENT_TYPE).and_then(|v|v.to_str().ok()).is_some_and(|v|v.starts_with("application/json"));
 // Artwork is the only authenticated binary API; never interpret image bytes as text.
 if !is_json&&response.status().is_success()&&route=="/api/artwork"{return response;}
 let (mut parts,body)=response.into_parts();
 let Ok(bytes)=to_bytes(body,32*1024*1024).await else{return (StatusCode::INTERNAL_SERVER_ERROR,Json(json!({"error":"Browser response unavailable"}))).into_response();};
 if bytes.is_empty(){return Response::from_parts(parts,Body::empty());}
 let Ok(mut value)=serde_json::from_slice::<Value>(&bytes)else{return (parts.status,Json(json!({"error":"Browser response unavailable"}))).into_response();};
 let roots=(||->rusqlite::Result<Vec<String>>{let c=a.db.lock().unwrap();let mut q=c.prepare("SELECT path FROM roots")?;let rows=q.query_map([],|r|r.get(0))?.collect();rows})();
 let Ok(roots)=roots else{return (StatusCode::INTERNAL_SERVER_ERROR,Json(json!({"error":"Browser response unavailable"}))).into_response();};
 if access(route)==WebAccess::Project{project(&mut value,route);}
 scrub(&mut value,&roots);
 parts.headers.remove(header::CONTENT_LENGTH);parts.headers.insert(header::CONTENT_TYPE,header::HeaderValue::from_static("application/json"));
 Response::from_parts(parts,Body::from(value.to_string()))
}
#[cfg(test)]pub(crate) mod tests;
