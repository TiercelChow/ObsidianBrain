use crate::{
    error::BrainError, infra::reader_bookmark_store::ReaderBookmarkStore,
    models::reader_bookmark::BookmarkWrite, tools::traits::ToolHandler, AppContext,
};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

pub struct ListReaderBookmarksHandler;
pub struct SaveReaderBookmarkHandler;
pub struct DeleteReaderBookmarkHandler;

fn string_arg(args: &Value, key: &str) -> Result<String, BrainError> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty() && v.len() <= 200)
        .map(str::to_owned)
        .ok_or_else(|| BrainError::KnowledgeValidation(format!("书签参数 {key} 不合法")))
}

#[async_trait]
impl ToolHandler for ListReaderBookmarksHandler {
    fn name(&self) -> &str {
        "list_reader_bookmarks"
    }
    fn description(&self) -> &str {
        "获取指定书籍的全部位置书签与 Markdown 笔记"
    }
    fn module(&self) -> &str {
        "reader"
    }
    fn input_schema(&self) -> Value {
        json!({"type":"object","additionalProperties":false,"properties":{"book_id":{"type":"string","minLength":1,"maxLength":200}},"required":["book_id"]})
    }
    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let book_id = string_arg(&args, "book_id")?;
        let db = ctx.db.clone();
        let bookmarks =
            tokio::task::spawn_blocking(move || ReaderBookmarkStore::new(db).list(&book_id))
                .await
                .map_err(|e| BrainError::Internal(e.to_string()))??;
        Ok(json!({"bookmarks":bookmarks}))
    }
}

#[async_trait]
impl ToolHandler for SaveReaderBookmarkHandler {
    fn name(&self) -> &str {
        "save_reader_bookmark"
    }
    fn description(&self) -> &str {
        "创建或按版本更新书签，不修改原书和阅读进度"
    }
    fn module(&self) -> &str {
        "reader"
    }
    fn input_schema(&self) -> Value {
        let unit = json!({"type":"number","minimum":0,"maximum":1});
        json!({"type":"object","additionalProperties":false,"properties":{
            "id":{"type":"string","minLength":1,"maxLength":200},"revision":{"type":"integer","minimum":1},
            "book_id":{"type":"string","minLength":1,"maxLength":200},"file_path":{"type":"string","minLength":1,"maxLength":8192},
            "title":{"type":"string","minLength":1,"maxLength":240},"note_md":{"type":"string","maxLength":131072},
            "location":{"oneOf":[
                {"type":"object","additionalProperties":false,"properties":{"kind":{"const":"md"},"ratio":unit,"block_index":{"type":"integer","minimum":0,"maximum":1000000},"text":{"type":"string","maxLength":240},"offset":unit},"required":["kind","ratio","block_index","text","offset"]},
                {"type":"object","additionalProperties":false,"properties":{"kind":{"const":"pdf"},"page":{"type":"integer","minimum":1,"maximum":1000000},"offset":unit,"x":unit},"required":["kind","page","offset","x"]}
            ]}
        },"required":["book_id","file_path","title","note_md","location"]})
    }
    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let draft: BookmarkWrite = serde_json::from_value(args)
            .map_err(|e| BrainError::KnowledgeValidation(format!("书签格式错误: {e}")))?;
        let db = ctx.db.clone();
        let bookmark =
            tokio::task::spawn_blocking(move || ReaderBookmarkStore::new(db).save(draft))
                .await
                .map_err(|e| BrainError::Internal(e.to_string()))??;
        Ok(json!({"bookmark":bookmark}))
    }
}

#[async_trait]
impl ToolHandler for DeleteReaderBookmarkHandler {
    fn name(&self) -> &str {
        "delete_reader_bookmark"
    }
    fn description(&self) -> &str {
        "按书籍 ID 和版本删除一个书签及其笔记"
    }
    fn module(&self) -> &str {
        "reader"
    }
    fn input_schema(&self) -> Value {
        json!({"type":"object","additionalProperties":false,"properties":{"book_id":{"type":"string","minLength":1,"maxLength":200},"id":{"type":"string","minLength":1,"maxLength":200},"revision":{"type":"integer","minimum":1}},"required":["book_id","id","revision"]})
    }
    async fn handle(&self, args: Value, ctx: &Arc<AppContext>) -> Result<Value, BrainError> {
        let book_id = string_arg(&args, "book_id")?;
        let id = string_arg(&args, "id")?;
        let revision = args
            .get("revision")
            .and_then(Value::as_i64)
            .filter(|v| *v > 0)
            .ok_or_else(|| BrainError::KnowledgeValidation("缺少书签版本".into()))?;
        let db = ctx.db.clone();
        tokio::task::spawn_blocking(move || {
            ReaderBookmarkStore::new(db).delete(&book_id, &id, revision)
        })
        .await
        .map_err(|e| BrainError::Internal(e.to_string()))??;
        Ok(json!({"deleted":true}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonschema::JSONSchema;

    #[test]
    fn test_bookmark_tool_schemas_accept_md_pdf_and_reject_unknown_fields() {
        let schema = SaveReaderBookmarkHandler.input_schema();
        let compiled = JSONSchema::compile(&schema).unwrap();
        for location in [
            json!({"kind":"md","ratio":0.3,"block_index":2,"text":"段落","offset":0.5}),
            json!({"kind":"pdf","page":12,"offset":0.4,"x":0}),
        ] {
            let mut value = json!({"book_id":"a","file_path":"/books/a/file.md","title":"书签","note_md":"**正文**","location":location});
            assert!(compiled.is_valid(&value));
            assert!(serde_json::from_value::<BookmarkWrite>(value.clone()).is_ok());
            value["location"]["unexpected"] = json!(true);
            assert!(!compiled.is_valid(&value));
        }
        for handler in [
            ListReaderBookmarksHandler.input_schema(),
            DeleteReaderBookmarkHandler.input_schema(),
        ] {
            assert!(JSONSchema::compile(&handler).is_ok());
        }
    }
}
