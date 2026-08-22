-- 任务中枢脱离 Obsidian 同步：删除同步队列与文档同步错误标记（设计见
-- docs/superpowers/specs/2026-08-22-tasks-decouple-obsidian-design.md）
DROP TABLE IF EXISTS task_sync_queue;

ALTER TABLE task_documents DROP COLUMN sync_error;
