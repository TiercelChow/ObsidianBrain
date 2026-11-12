//! Attachments use stable identities, independent of task document row replacement.
use crate::{error::BrainError, infra::sqlite_store::SqliteStore};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{path::PathBuf, sync::Arc};

pub const MAX_ATTACHMENT_BYTES: u64 = 50 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Clone, Debug, Serialize)]
pub struct TaskAttachment {
    pub id: String,
    pub task_id: String,
    pub progress_id: Option<String>,
    pub filename: String,
    pub size_bytes: u64,
    pub created_at: String,
}
pub struct TaskAttachmentStore {
    db: Arc<SqliteStore>,
}
fn invalid(message: &str) -> BrainError {
    BrainError::TaskValidation(message.into())
}
fn owner(c: &rusqlite::Connection, task: &str, progress: Option<&str>) -> Result<(), BrainError> {
    let valid: bool = c.query_row("SELECT EXISTS(SELECT 1 FROM task_nodes WHERE id=?1) AND (?2 IS NULL OR EXISTS(SELECT 1 FROM task_progress WHERE id=?2 AND task_id=?1))", params![task,progress], |r|r.get(0))?;
    if valid {
        Ok(())
    } else {
        Err(invalid("任务或进展不存在，或进展不属于此任务"))
    }
}
fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<TaskAttachment> {
    Ok(TaskAttachment {
        id: r.get(0)?,
        task_id: r.get(1)?,
        progress_id: r.get(2)?,
        filename: r.get(3)?,
        size_bytes: r.get(4)?,
        created_at: r.get(5)?,
    })
}
impl TaskAttachmentStore {
    pub fn new(db: Arc<SqliteStore>) -> Self {
        Self { db }
    }
    pub fn directory(&self) -> PathBuf {
        self.db
            .database_path()
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join("tasks")
            .join("attachments")
    }
    pub fn file_path(&self, id: &str) -> Result<PathBuf, BrainError> {
        let id = uuid::Uuid::parse_str(id).map_err(|_| invalid("附件 ID 无效"))?;
        Ok(self.directory().join(id.to_string()))
    }
    pub fn list(
        &self,
        task: &str,
        progress: Option<&str>,
    ) -> Result<Vec<TaskAttachment>, BrainError> {
        self.cleanup()?;
        self.db.with_connection(|c| {
            owner(c,task,progress)?;
            let mut q=c.prepare("SELECT id,task_id,progress_id,filename,size_bytes,created_at FROM task_attachments WHERE task_id=?1 AND progress_id IS ?2 AND state='active' ORDER BY created_at,id")?;
            let result = q.query_map(params![task,progress],row)?.collect::<Result<Vec<_>,_>>()?;
            Ok(result)
        })
    }
    pub fn reserve(
        &self,
        task: &str,
        progress: Option<&str>,
        filename: &str,
    ) -> Result<TaskAttachment, BrainError> {
        self.reserve_inner(task, progress, filename, None)
            .map(|(item, _)| item)
    }
    pub fn reserve_with_key(
        &self,
        task: &str,
        progress: Option<&str>,
        filename: &str,
        key: &str,
    ) -> Result<(TaskAttachment, bool), BrainError> {
        if key.is_empty() || key.len() > 200 || key.chars().any(char::is_control) {
            return Err(invalid("上传标识无效"));
        }
        self.reserve_inner(task, progress, filename, Some(key))
    }
    fn reserve_inner(
        &self,
        task: &str,
        progress: Option<&str>,
        filename: &str,
        key: Option<&str>,
    ) -> Result<(TaskAttachment, bool), BrainError> {
        if filename.trim().is_empty()
            || filename.len() > 512
            || filename.contains(['/', '\\'])
            || filename.chars().any(char::is_control)
        {
            return Err(invalid("附件文件名无效"));
        }
        self.cleanup()?;
        let id = if let Some(key) = key {
            let mut hash = Sha256::new();
            for part in [task, progress.unwrap_or(""), key] {
                hash.update((part.len() as u64).to_be_bytes());
                hash.update(part.as_bytes());
            }
            let hash = hash.finalize();
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(&hash[..16]);
            uuid::Uuid::from_bytes(bytes).to_string()
        } else {
            uuid::Uuid::new_v4().to_string()
        };
        self.db.with_connection(|c| {
            let tx=c.unchecked_transaction()?;
            owner(&tx,task,progress)?;
            if key.is_some(){
                let previous=tx.query_row("SELECT id,task_id,progress_id,filename,size_bytes,created_at,state FROM task_attachments WHERE id=?1",[&id],|r|Ok((row(r)?,r.get::<_,String>(6)?))).optional()?;
                if let Some((item,state))=previous{
                    if item.task_id!=task || item.progress_id.as_deref()!=progress || item.filename!=filename{return Err(invalid("上传标识已用于其他文件"));}
                    if state!="active"{return Err(invalid("上一次上传仍在处理，请稍后重试"));}
                    tx.commit()?;return Ok((item,false));
                }
            }
            let count:i64=tx.query_row("SELECT COUNT(*) FROM task_attachments WHERE task_id=?1 AND progress_id IS ?2 AND state<>'deleted'",params![task,progress],|r|r.get(0))?;
            let total:u64=tx.query_row("SELECT COALESCE(SUM(size_bytes),0) FROM task_attachments",[],|r|r.get(0))?;
            if count>=20 || total+MAX_ATTACHMENT_BYTES>MAX_TOTAL_BYTES { return Err(invalid("每个附件区最多 20 个文件，附件总空间最多 1 GiB")); }
            let item=TaskAttachment{id,task_id:task.into(),progress_id:progress.map(Into::into),filename:filename.into(),size_bytes:MAX_ATTACHMENT_BYTES,created_at:chrono::Utc::now().to_rfc3339()};
            tx.execute("INSERT INTO task_attachments(id,task_id,progress_id,filename,size_bytes,state,created_at) VALUES (?1,?2,?3,?4,?5,'uploading',?6)",params![item.id,task,progress,filename,MAX_ATTACHMENT_BYTES,item.created_at])?;
            tx.commit()?;
            Ok((item,true))
        })
    }
    pub fn finish(&self, id: &str, size: u64) -> Result<(), BrainError> {
        if size == 0 || size > MAX_ATTACHMENT_BYTES {
            return Err(invalid("附件不能为空，最大 50 MiB"));
        }
        self.db.with_connection(|c| {
            let tx=c.unchecked_transaction()?;
            let (task,progress):(String,Option<String>)=tx.query_row("SELECT task_id,progress_id FROM task_attachments WHERE id=?1 AND state='uploading'",[id],|r|Ok((r.get(0)?,r.get(1)?)))?;
            owner(&tx,&task,progress.as_deref())?;
            tx.execute("UPDATE task_attachments SET state='active',size_bytes=?2 WHERE id=?1 AND state='uploading'",params![id,size])?;
            tx.commit()?;
            Ok(())
        })
    }
    pub fn get(&self, id: &str) -> Result<TaskAttachment, BrainError> {
        self.file_path(id)?;
        self.db.with_connection(|c| {
            let item=c.query_row("SELECT id,task_id,progress_id,filename,size_bytes,created_at FROM task_attachments WHERE id=?1 AND state='active'",[id],row).optional()?.ok_or_else(||invalid("附件不存在或未上传完成"))?;
            owner(c,&item.task_id,item.progress_id.as_deref())?;
            Ok(item)
        })
    }
    pub fn delete(&self, id: &str) -> Result<(), BrainError> {
        self.file_path(id)?;
        self.db.with_connection(|c| {
            c.execute(
                "UPDATE task_attachments SET state='deleted' WHERE id=?1",
                [id],
            )?;
            Ok(())
        })?;
        self.cleanup()
    }
    pub fn recover_interrupted(&self) -> Result<(), BrainError> {
        self.db.with_connection(|c| { c.execute("UPDATE task_attachments SET state='deleted' WHERE state='uploading' OR NOT EXISTS(SELECT 1 FROM task_nodes n WHERE n.id=task_attachments.task_id) OR (progress_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM task_progress p WHERE p.id=task_attachments.progress_id AND p.task_id=task_attachments.task_id))",[])?; Ok(()) })?;
        self.cleanup()
    }
    fn cleanup(&self) -> Result<(), BrainError> {
        let ids = self.db.with_connection(|c| {
            let mut q = c.prepare("SELECT id FROM task_attachments WHERE state='deleted'")?;
            let ids = q
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ids)
        })?;
        for id in ids {
            self.cleanup_one(&id)?;
        }
        Ok(())
    }
    fn cleanup_one(&self, id: &str) -> Result<(), BrainError> {
        self.db.with_connection(|c| {
            // Hold the writer lock through unlink + row deletion. A second
            // cleaner may have an old ID list; it must not unlink a new retry.
            let tx =
                rusqlite::Transaction::new_unchecked(c, rusqlite::TransactionBehavior::Immediate)?;
            let deleted: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM task_attachments WHERE id=?1 AND state='deleted')",
                [id],
                |r| r.get(0),
            )?;
            if !deleted {
                return Ok(());
            }
            match std::fs::remove_file(self.file_path(id)?) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    tracing::warn!(%id,error=%e,"附件清理待重试");
                    return Ok(());
                }
            }
            tx.execute(
                "DELETE FROM task_attachments WHERE id=?1 AND state='deleted'",
                [id],
            )?;
            tx.commit()?;
            Ok(())
        })
    }
}
#[cfg(test)]
#[path = "task_attachment_tests.rs"]
mod tests;
