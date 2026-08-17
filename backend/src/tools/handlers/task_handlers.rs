//! Tool handlers for personal task management.

use std::sync::Arc;

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::error::BrainError;
use crate::models::task::*;
use crate::tools::definitions;
use crate::tools::traits::ToolHandler;
use crate::AppContext;

fn parse<T: DeserializeOwned>(args: Value) -> Result<T, BrainError> {
    serde_json::from_value(args)
        .map_err(|error| BrainError::TaskValidation(format!("参数解析失败: {error}")))
}

fn json<T: serde::Serialize>(value: T) -> Result<Value, BrainError> {
    serde_json::to_value(value)
        .map_err(|error| BrainError::Internal(format!("任务响应序列化失败: {error}")))
}

macro_rules! task_handler {
    ($handler:ident, $name:literal, $description:literal, $schema:ident, |$args:ident, $ctx:ident| $body:block) => {
        pub struct $handler;

        #[async_trait]
        impl ToolHandler for $handler {
            fn name(&self) -> &str {
                $name
            }

            fn description(&self) -> &str {
                $description
            }

            fn input_schema(&self) -> Value {
                definitions::$schema()
            }

            fn module(&self) -> &str {
                "tasks"
            }

            async fn handle(
                &self,
                $args: Value,
                $ctx: &Arc<AppContext>,
            ) -> Result<Value, BrainError> $body
        }
    };
}

task_handler!(
    CreateTaskHandler,
    "create_task",
    "创建短期待办或长期任务",
    create_task_schema,
    |args, ctx| {
        let request: TaskCreateRequest = parse(args)?;
        json(ctx.task_service.create_task(request).await?)
    }
);

task_handler!(
    ListTasksHandler,
    "list_tasks",
    "分页查询个人任务，支持类型、状态、重要程度、日期和关键词筛选",
    list_tasks_schema,
    |args, ctx| {
        let request: TaskQuery = parse(args)?;
        json(ctx.task_service.list_tasks(request).await?)
    }
);

#[derive(Deserialize)]
struct TaskIdArgs {
    task_id: Uuid,
}

task_handler!(
    GetTaskHandler,
    "get_task",
    "获取任务详情、任务树、进展与审计记录",
    get_task_schema,
    |args, ctx| {
        let request: TaskIdArgs = parse(args)?;
        json(ctx.task_service.get_task(request.task_id).await?)
    }
);

task_handler!(
    UpdateTaskHandler,
    "update_task",
    "编辑任务或子任务的标题、描述、日期和重要程度",
    update_task_schema,
    |args, ctx| {
        let request: TaskUpdateRequest = parse(args)?;
        json(ctx.task_service.update_task(request).await?)
    }
);

task_handler!(
    SetTaskStatusHandler,
    "set_task_status",
    "完成、取消、阻塞或重新打开任务，可保存关闭说明",
    set_task_status_schema,
    |args, ctx| {
        let request: TaskStatusRequest = parse(args)?;
        json(ctx.task_service.set_task_status(request).await?)
    }
);

task_handler!(
    AddSubtaskHandler,
    "add_subtask",
    "在长期任务的任意节点下添加子任务",
    add_subtask_schema,
    |args, ctx| {
        let request: SubtaskCreateRequest = parse(args)?;
        json(ctx.task_service.add_subtask(request).await?)
    }
);

task_handler!(
    MoveSubtaskHandler,
    "move_subtask",
    "移动长期子任务到同一根任务下的新父节点和位置",
    move_subtask_schema,
    |args, ctx| {
        let request: MoveSubtaskRequest = parse(args)?;
        json(ctx.task_service.move_subtask(request).await?)
    }
);

task_handler!(
    AddTaskProgressHandler,
    "add_task_progress",
    "为长期根任务或子任务追加进展与可选百分比",
    add_task_progress_schema,
    |args, ctx| {
        let request: ProgressCreateRequest = parse(args)?;
        json(ctx.task_service.add_progress(request).await?)
    }
);

task_handler!(
    GetTaskCalendarHandler,
    "get_task_calendar",
    "查询与日期范围重叠的任务日程",
    get_task_calendar_schema,
    |args, ctx| {
        let request: CalendarTaskQuery = parse(args)?;
        json(ctx.task_service.calendar_tasks(request).await?)
    }
);

task_handler!(
    ArchiveTaskHandler,
    "archive_task",
    "归档或恢复短期待办和长期根任务",
    archive_task_schema,
    |args, ctx| {
        let request: ArchiveTaskRequest = parse(args)?;
        json(ctx.task_service.archive_task(request).await?)
    }
);

#[derive(Debug, Default, Deserialize)]
struct SyncTasksArgs {
    #[serde(default)]
    dry_run: bool,
}

task_handler!(
    SyncTasksHandler,
    "sync_tasks",
    "从 Obsidian Tasks 文件夹增量刷新或预检任务索引",
    sync_tasks_schema,
    |args, ctx| {
        let request: SyncTasksArgs = parse(args)?;
        json(ctx.task_service.sync_tasks(request.dry_run).await?)
    }
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_handlers_expose_task_module_and_schemas() {
        let handlers: Vec<Box<dyn ToolHandler>> = vec![
            Box::new(CreateTaskHandler),
            Box::new(ListTasksHandler),
            Box::new(GetTaskHandler),
            Box::new(UpdateTaskHandler),
            Box::new(SetTaskStatusHandler),
            Box::new(AddSubtaskHandler),
            Box::new(MoveSubtaskHandler),
            Box::new(AddTaskProgressHandler),
            Box::new(GetTaskCalendarHandler),
            Box::new(ArchiveTaskHandler),
            Box::new(SyncTasksHandler),
        ];
        assert_eq!(handlers.len(), 11);
        assert!(handlers
            .iter()
            .all(|handler| handler.module() == "tasks" && handler.input_schema().is_object()));
    }
}
