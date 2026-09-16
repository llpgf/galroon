use axum::{extract::{Path, Query, State}, http::HeaderMap, Json};
use rusqlite::{params, Connection};
use serde_json::{json, Value};

#[derive(Default, serde::Deserialize)]
pub struct PageQuery {
    #[serde(default)]
    paged: bool,
    before: Option<String>,
}

// A fixed insertion ceiling keeps later jobs (including backdated jobs) out of
// an older-page traversal. Creation time + rowid preserves the legacy ordering.
#[derive(serde::Serialize, serde::Deserialize)]
struct Cursor { ceiling: i64, created: i64, row: i64 }

pub fn page(c: &Connection, before: Option<&str>) -> std::result::Result<Value, String> {
    let cursor = before.map(|raw| {
        if raw.len() > 200 { return Err("Invalid task cursor".to_owned()); }
        let p: Cursor = serde_json::from_str(raw).map_err(|_| "Invalid task cursor".to_owned())?;
        if p.ceiling <= 0 || p.row <= 0 || p.row > p.ceiling { return Err("Invalid task cursor".into()); }
        Ok(p)
    }).transpose()?;
    let ceiling = match &cursor { Some(p) => p.ceiling, None => c.query_row("SELECT coalesce(max(rowid),0) FROM jobs", [], |r| r.get(0)).map_err(|e|e.to_string())? };
    let boundary = if cursor.is_some() { " AND (created,rowid)<(?2,?3)" } else { " AND ?2 IS NULL AND ?3 IS NULL" };
    let sql=format!("SELECT rowid,id,kind,state,processed,discovered,bytes,errors,current_path,message,created,updated,coalesce(json_extract(spec,'$.automatic'),0),coalesce(json_extract(spec,'$.reuse_existing'),0) FROM jobs WHERE rowid<=?1{boundary} ORDER BY created DESC,rowid DESC LIMIT 101");
    let mut statement = c.prepare(&sql).map_err(|e|e.to_string())?;
    let rows = statement.query_map(params![ceiling,cursor.as_ref().map(|p|p.created),cursor.as_ref().map(|p|p.row)], |r| Ok((r.get::<_,i64>(0)?, json!({
        "id":r.get::<_,String>(1)?,"kind":r.get::<_,String>(2)?,"state":r.get::<_,String>(3)?,"processed":r.get::<_,i64>(4)?,"discovered":r.get::<_,i64>(5)?,"bytes":r.get::<_,i64>(6)?,"errors":r.get::<_,i64>(7)?,"current_path":r.get::<_,String>(8)?,"message":r.get::<_,String>(9)?,"created":r.get::<_,i64>(10)?,"updated":r.get::<_,i64>(11)?,"automatic":r.get::<_,bool>(12)?,"reuse_existing":r.get::<_,bool>(13)?
    })))).map_err(|e|e.to_string())?.collect::<rusqlite::Result<Vec<_>>>().map_err(|e|e.to_string())?;
    let more = rows.len() > 100;
    let next = if more { let (row, item) = &rows[99]; Some(serde_json::to_string(&Cursor {ceiling, created:item["created"].as_i64().unwrap(),row:*row}).map_err(|e|e.to_string())?) } else {None};
    let items=rows.into_iter().take(100).map(|(_,mut item)|{item["summary"]=crate::job_summary::read(c,item["id"].as_str().unwrap()).map_err(|e|e.to_string())?;Ok(item)}).collect::<std::result::Result<Vec<_>,String>>()?;
    Ok(json!({"items":items,"next":next}))
}

pub async fn list(State(a): State<crate::App>, h: HeaderMap, Query(query): Query<PageQuery>) -> crate::Result<Value> {
    crate::auth(&a,&h)?;
    if query.before.is_some() && !query.paged { return Err(crate::ApiError("Task cursor requires paged=true".into())); }
    let c = a.db.lock().map_err(|e|crate::ApiError(e.to_string()))?;
    let result = page(&c,query.before.as_deref()).map_err(crate::ApiError)?;
    // Existing clients keep the array response; new clients opt into paging.
    Ok(Json(if query.paged {result} else {result["items"].clone()}))
}

pub async fn detail(State(a):State<crate::App>, h:HeaderMap, Path(id):Path<String>)->crate::Result<Value>{
    crate::auth(&a,&h)?;
    let c=a.db.lock().map_err(|e|crate::ApiError(e.to_string()))?;
    let mut item=c.query_row("SELECT id,kind,state,processed,discovered,bytes,errors,current_path,message,created,updated,coalesce(json_extract(spec,'$.automatic'),0),coalesce(json_extract(spec,'$.reuse_existing'),0) FROM jobs WHERE id=?1",[id],|r|Ok(json!({
        "id":r.get::<_,String>(0)?,"kind":r.get::<_,String>(1)?,"state":r.get::<_,String>(2)?,"processed":r.get::<_,i64>(3)?,"discovered":r.get::<_,i64>(4)?,"bytes":r.get::<_,i64>(5)?,"errors":r.get::<_,i64>(6)?,"current_path":r.get::<_,String>(7)?,"message":r.get::<_,String>(8)?,"created":r.get::<_,i64>(9)?,"updated":r.get::<_,i64>(10)?,"automatic":r.get::<_,bool>(11)?,"reuse_existing":r.get::<_,bool>(12)?
    })))?;
    item["summary"]=crate::job_summary::read(&c,item["id"].as_str().unwrap())?;
    Ok(Json(item))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tied_dates_insertions_live_updates_and_restore_preserve_traversal() {
        let temp=tempfile::tempdir().unwrap();
        let db=crate::db::open(&temp.path().join("library.sqlite")).unwrap();
        let mut ids=Vec::new();
        {
            let c=db.lock().unwrap();
            for i in 0..205 {c.execute("INSERT INTO jobs(id,kind,state,spec,created,updated) VALUES(?1,'scan','completed','{\"password\":\"generated-secret\"}',?2,1)",params![format!("job-{i}"), i/110]).unwrap();}
            let first=page(&c,None).unwrap();
            assert_eq!(first["items"].as_array().unwrap().len(),100);
            assert_eq!(first["items"][0]["id"],"job-204");
            assert!(!first.to_string().contains("generated-secret"));
            ids.extend(first["items"].as_array().unwrap().iter().map(|i|i["id"].as_str().unwrap().to_owned()));
            c.execute_batch("INSERT INTO jobs(id,kind,state,spec,created,updated) VALUES('new-head','scan','completed','{}',999,1),('backdated','scan','completed','{}',-1,1); UPDATE jobs SET state='failed' WHERE id='job-104';").unwrap();
            let second=page(&c,first["next"].as_str()).unwrap();
            assert_eq!(second["items"][0]["id"],"job-104");
            assert_eq!(second["items"][0]["state"],"failed");
            ids.extend(second["items"].as_array().unwrap().iter().map(|i|i["id"].as_str().unwrap().to_owned()));
            let third=page(&c,second["next"].as_str()).unwrap();
            assert_eq!(third["items"].as_array().unwrap().len(),5);
            assert!(third["next"].is_null());
            ids.extend(third["items"].as_array().unwrap().iter().map(|i|i["id"].as_str().unwrap().to_owned()));
            assert_eq!(ids,(0..205).rev().map(|i|format!("job-{i}")).collect::<Vec<_>>());
            assert_eq!(page(&c,None).unwrap()["items"][0]["id"],"new-head");
            for raw in ["bad","{}","{\"ceiling\":2,\"created\":0,\"row\":3}"] {assert!(page(&c,Some(raw)).is_err());}
        }
        let backup=crate::backup::export(&db,temp.path()).unwrap();
        let restored=crate::backup::restore_new(&backup,&temp.path().join("restored"),None).unwrap();
        let copy=crate::db::open(&restored.join("library.sqlite")).unwrap();
        assert_eq!(page(&db.lock().unwrap(),None).unwrap(),page(&copy.lock().unwrap(),None).unwrap());
    }
}
