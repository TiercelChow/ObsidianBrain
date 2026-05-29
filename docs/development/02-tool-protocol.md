# Tool Protocol & API 层 — 开发设计文档

> **版本**: v0.1-draft | **最后更新**: 2026-05-29 | **状态**: 设计中  
> **上游依赖**: [顶层设计文档](../top_design.md) | [需求设计文档](../requirement/02-tool-protocol.md)  
> **对应模块**: `src/api/` + `src/tools/`

---

## 1. 技术架构详细设计

### 1.1 分层架构

协议层采用四层架构，从上到下依次为：

```
┌───────────────────────────────────────────────────────────────┐
│                      传输层 (Transport)                        │
│   ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐   │
│   │ stdio 传输    │  │ SSE 传输      │  │ Axum HTTP 传输   │   │
│   │ (stdin/out)  │  │ (可选)        │  │ (REST API)       │   │
│   └──────┬───────┘  └──────┬───────┘  └────────┬─────────┘   │
└──────────┼─────────────────┼───────────────────┼─────────────┘
           │                 │                   │
           ▼                 ▼                   ▼
┌───────────────────────────────────────────────────────────────┐
│                      协议层 (Protocol)                         │
│   ┌────────────────────────────────────────────────────────┐  │
│   │  MCP Protocol Handler (JSON-RPC 2.0)                   │  │
│   │  ├── initialize / initialized                          │  │
│   │  ├── tools/list                                        │  │
│   │  ├── tools/call                                        │  │
│   │  └── ping                                              │  │
│   └────────────────────────────────────────────────────────┘  │
│   ┌────────────────────────────────────────────────────────┐  │
│   │  HTTP Protocol Handler (Axum Handlers)                 │  │
│   │  ├── GET  /v1/tools                                    │  │
│   │  ├── POST /v1/tools/call                               │  │
│   │  └── GET  /v1/health                                   │  │
│   └────────────────────────────────────────────────────────┘  │
└───────────────────────────┬───────────────────────────────────┘
                            │
                            ▼
┌───────────────────────────────────────────────────────────────┐
│                      注册与路由层 (Registry & Dispatch)        │
│   ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐   │
│   │ ToolRegistry │  │ ParamValidator│ │  CallDispatcher   │   │
│   │ (工具注册表)  │  │ (参数校验)    │  │  (调用分发)       │   │
│   └──────────────┘  └──────────────┘  └──────────────────┘   │
└───────────────────────────┬───────────────────────────────────┘
                            │
                            ▼
┌───────────────────────────────────────────────────────────────┐
│                      执行层 (Handlers)                         │
│   ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌──────────┐  │
│   │ search_*   │ │ memory_*   │ │ repo_*     │ │ radar_*  │  │
│   │ handlers   │ │ handlers   │ │ handlers   │ │ handlers │  │
│   └────────────┘ └────────────┘ └────────────┘ └──────────┘  │
│   ┌────────────┐ ┌────────────┐                               │
│   │ timeline_* │ │ inspiration│                               │
│   │ handlers   │ │ handlers   │                               │
│   └────────────┘ └────────────┘                               │
└───────────────────────────────────────────────────────────────┘
```

### 1.2 核心数据流

```
外部请求 (JSON bytes)
    │
    ▼
传输层：接收原始字节，解析为协议消息
    │
    ▼
协议层：提取 tool_name + arguments
    │  ├── MCP: 解析 JSON-RPC 2.0 → method + params
    │  └── HTTP: 解析 Request Body → tool + arguments
    │
    ▼
注册与路由层：
    │  ├── ToolRegistry.get(tool_name) → ToolDefinition
    │  ├── ParamValidator.validate(arguments, input_schema) → ValidatedArgs
    │  └── CallDispatcher.dispatch(tool_name, validated_args, ctx) → ToolCallResult
    │
    ▼
执行层：
    │  ├── 反序列化参数为强类型结构体
    │  ├── 调用核心 Service 方法
    │  └── 序列化结果为 serde_json::Value
    │
    ▼
协议层：包装为标准响应格式
    │  ├── MCP: CallToolResult { content, isError }
    │  └── HTTP: { tool, status, result/error }
    │
    ▼
传输层：序列化为 JSON bytes，发送响应
```

### 1.3 关键设计决策

| 决策 | 选型 | 理由 |
|---|---|---|
| 异步运行时 | Tokio | 与 Axum 天然集成，生态成熟 |
| HTTP 框架 | Axum | 类型安全路由，Tower 中间件生态 |
| JSON-RPC 实现 | 自行实现（轻量） | MCP 仅用到 JSON-RPC 的子集，无需引入完整 JSON-RPC 库 |
| Schema 校验 | `jsonschema` crate | 支持 JSON Schema Draft 7，可在运行时校验参数 |
| 工具注册 | 过程式宏 + 手动注册 | 初期手动注册，后期可引入过程式宏自动化 |
| 序列化 | serde + serde_json | Rust 标准选择，零成本抽象 |

---

## 2. 目录与文件组织

```
src/
├── api/                            # API 层（协议 + 传输）
│   ├── mod.rs                      # 模块导出 + 服务启动入口
│   ├── router.rs                   # Axum 路由定义 + 中间件链
│   ├── tool_protocol.rs            # MCP 协议实现（JSON-RPC 2.0 消息处理）
│   ├── mcp_stdio.rs                # MCP stdio 传输层实现
│   ├── mcp_sse.rs                  # MCP SSE 传输层实现（可选）
│   ├── middleware.rs                # 自定义中间件（日志、限流、CORS）
│   └── handlers/                   # HTTP 请求处理器
│       ├── mod.rs                  # Handler 模块导出
│       ├── tool_handler.rs         # /v1/tools 和 /v1/tools/call 处理器
│       ├── health_handler.rs       # /v1/health 处理器
│       └── search_handler.rs       # search_notes 工具实现
│       └── memory_handler.rs       # search_memory / add_memory 等工具实现
│       └── repo_handler.rs         # add_code_repo / list_code_repos 等工具实现
│       └── timeline_handler.rs     # get_timeline 工具实现
│       └── inspiration_handler.rs  # get_inspiration 工具实现
│       └── radar_handler.rs        # get_radar / add_to_vault 等工具实现
│       └── system_handler.rs       # get_stats 工具实现
│
├── tools/                          # 工具定义与注册
│   ├── mod.rs                      # 模块导出 + 工具集初始化
│   ├── registry.rs                 # ToolRegistry 结构 + 注册/查找/枚举
│   ├── definitions.rs              # 所有工具的 JSON Schema 定义
│   └── traits.rs                   # ToolHandler trait 定义
│
├── models/                         # 共享数据模型（被 handlers 使用）
│   ├── mod.rs
│   ├── note.rs
│   ├── memory.rs
│   ├── repo.rs
│   └── radar.rs
│
├── error.rs                        # 统一错误类型 BrainError
├── config.rs                       # 配置加载
└── main.rs                         # 入口
```

### 2.1 文件职责说明

| 文件 | 职责 | 行数预估 |
|---|---|---|
| `api/mod.rs` | 模块导出、`start_server()` 函数（根据配置启动 HTTP/MCP/Both） | ~80 |
| `api/router.rs` | Axum `Router` 构建、中间件挂载、路由表定义 | ~120 |
| `api/tool_protocol.rs` | MCP 协议核心逻辑：JSON-RPC 消息解析、方法分发、响应构建 | ~300 |
| `api/mcp_stdio.rs` | stdio 传输实现：从 stdin 读行、向 stdout 写行、消息分帧 | ~150 |
| `api/mcp_sse.rs` | SSE 传输实现：SSE 事件流 + POST 端点（可选实现） | ~200 |
| `api/middleware.rs` | 请求日志中间件、CORS 配置、简单限流 | ~100 |
| `api/handlers/*.rs` | 各工具的 HTTP handler + MCP handler 共享的业务逻辑 | 每个 ~80-150 |
| `tools/registry.rs` | ToolRegistry 数据结构、CRUD 操作、线程安全封装 | ~200 |
| `tools/definitions.rs` | 所有工具的 ToolDefinition 构建函数 | ~500 |
| `tools/traits.rs` | ToolHandler trait 和相关类型定义 | ~60 |

---

## 3. 各子模块详细设计

### 3.1 `tools/traits.rs` — ToolHandler Trait 定义

```rust
use async_trait::async_trait;
use serde_json::Value;
use crate::error::BrainError;

/// 应用上下文，包含所有 Service 的引用
pub struct AppContext {
    pub memory_service: Arc<MemoryService>,
    pub timeline_service: Arc<TimelineService>,
    pub code_repo_service: Arc<CodeRepoService>,
    pub inspiration_service: Arc<InspirationService>,
    pub radar_service: Arc<RadarService>,
    pub search_service: Arc<SearchService>,
    pub config: Arc<AppConfig>,
}

/// 工具调用请求（协议层标准化后的内部表示）
pub struct ToolCallRequest {
    pub request_id: Uuid,
    pub tool_name: String,
    pub arguments: Value,
}

/// 工具调用结果（协议层标准化的内部表示）
pub struct ToolCallResult {
    pub tool_name: String,
    pub status: CallStatus,
    pub result: Option<Value>,
    pub error: Option<ToolError>,
    pub duration_ms: u64,
}

pub enum CallStatus {
    Success,
    Error,
}

pub struct ToolError {
    pub code: String,
    pub message: String,
    pub suggestion: Option<String>,
}

/// 工具 Handler trait — 所有工具必须实现此 trait
#[async_trait]
pub trait ToolHandler: Send + Sync {
    /// 返回工具名称
    fn name(&self) -> &str;

    /// 返回工具定义（包含 Schema）
    fn definition(&self) -> ToolDefinition;

    /// 执行工具调用
    async fn handle(
        &self,
        args: Value,
        ctx: &AppContext,
    ) -> Result<Value, BrainError>;
}
```

### 3.2 `tools/registry.rs` — ToolRegistry 结构

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use serde_json::Value;

/// 工具定义，包含名称、描述、参数 Schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,          // JSON Schema object
    pub module: String,               // 所属模块标签
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,      // 工具版本号
}

/// 工具注册表 — 管理所有已注册工具的元信息和 Handler 引用
pub struct ToolRegistry {
    /// 工具名 → (ToolDefinition, Arc<dyn ToolHandler>)
    tools: RwLock<HashMap<String, RegisteredTool>>,
    /// 变更通知发送端（用于 MCP tools/list_changed 通知）
    change_tx: Option<tokio::sync::broadcast::Sender<()>>,
}

struct RegisteredTool {
    definition: ToolDefinition,
    handler: Arc<dyn ToolHandler>,
    registered_at: chrono::DateTime<chrono::Utc>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
            change_tx: None,
        }
    }

    /// 设置变更通知通道
    pub fn set_change_notifier(
        &mut self,
        tx: tokio::sync::broadcast::Sender<()>,
    ) {
        self.change_tx = Some(tx);
    }

    /// 注册一个工具（如已存在同名工具则覆盖）
    pub async fn register(&self, handler: Arc<dyn ToolHandler>) {
        let def = handler.definition();
        let name = def.name.clone();
        let mut tools = self.tools.write().await;

        if tools.contains_key(&name) {
            tracing::warn!("工具 '{}' 已存在，将被覆盖", name);
        }

        tools.insert(name.clone(), RegisteredTool {
            definition: def,
            handler,
            registered_at: chrono::Utc::now(),
        });

        tracing::info!("工具 '{}' 已注册", name);

        // 通知工具列表已变更
        if let Some(tx) = &self.change_tx {
            let _ = tx.send(());
        }
    }

    /// 注销一个工具
    pub async fn unregister(&self, name: &str) -> bool {
        let mut tools = self.tools.write().await;
        let removed = tools.remove(name).is_some();
        if removed {
            tracing::info!("工具 '{}' 已注销", name);
            if let Some(tx) = &self.change_tx {
                let _ = tx.send(());
            }
        }
        removed
    }

    /// 根据名称查找工具 Handler
    pub async fn get_handler(&self, name: &str) -> Option<Arc<dyn ToolHandler>> {
        let tools = self.tools.read().await;
        tools.get(name).map(|t| t.handler.clone())
    }

    /// 获取所有工具定义列表
    pub async fn list_definitions(&self) -> Vec<ToolDefinition> {
        let tools = self.tools.read().await;
        tools.values().map(|t| t.definition.clone()).collect()
    }

    /// 按模块标签过滤工具列表
    pub async fn list_by_module(&self, module: &str) -> Vec<ToolDefinition> {
        let tools = self.tools.read().await;
        tools.values()
            .filter(|t| t.definition.module == module)
            .map(|t| t.definition.clone())
            .collect()
    }

    /// 获取工具数量
    pub async fn count(&self) -> usize {
        let tools = self.tools.read().await;
        tools.len()
    }

    /// 检查工具是否存在
    pub async fn exists(&self, name: &str) -> bool {
        let tools = self.tools.read().await;
        tools.contains_key(name)
    }
}
```

### 3.3 `api/router.rs` — Axum 路由定义

```rust
use axum::{
    Router,
    routing::{get, post},
    middleware as axum_mw,
    extract::State,
};
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::api::handlers::{tool_handler, health_handler};
use crate::api::middleware::{request_id_middleware, tool_call_logging_middleware};
use crate::tools::registry::ToolRegistry;

/// 应用共享状态
pub struct AppState {
    pub registry: Arc<ToolRegistry>,
    pub context: Arc<AppContext>,
    pub start_time: chrono::DateTime<chrono::Utc>,
}

/// 构建 Axum Router
pub fn build_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any);

    Router::new()
        // ── 工具 API 路由 ──
        .route("/v1/tools", get(tool_handler::list_tools))
        .route("/v1/tools/call", post(tool_handler::call_tool))
        .route("/v1/tools/{tool_name}", get(tool_handler::get_tool))

        // ── 健康检查 ──
        .route("/v1/health", get(health_handler::health_check))

        // ── 中间件链 ──
        .layer(
            ServiceBuilder::new()
                .layer(axum_mw::from_fn(request_id_middleware))
                .layer(TraceLayer::new_for_http())
                .layer(axum_mw::from_fn(tool_call_logging_middleware))
                .layer(cors),
        )
        .with_state(state)
}
```

**中间件链执行顺序**（由外到内）：

```
请求进入
  │
  ├── 1. request_id_middleware：生成/提取 X-Request-ID，注入到 Extension
  ├── 2. TraceLayer：tower-http 提供的请求追踪（span 自动生成）
  ├── 3. tool_call_logging_middleware：记录请求路径、方法、耗时
  ├── 4. CorsLayer：处理跨域请求
  │
  ▼
Handler 执行
  │
  ▼
响应返回（中间件逆序执行）
```

### 3.4 `api/tool_protocol.rs` — MCP 协议实现

#### 3.4.1 JSON-RPC 2.0 消息类型

```rust
use serde::{Serialize, Deserialize};
use serde_json::Value;

/// JSON-RPC 2.0 请求
#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,           // 必须为 "2.0"
    pub id: Option<Value>,         // 请求 ID（notification 时为 None）
    pub method: String,            // 方法名
    #[serde(default)]
    pub params: Value,             // 参数（可为 object 或 array）
}

/// JSON-RPC 2.0 成功响应
#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,           // "2.0"
    pub id: Value,                 // 对应请求 ID
    pub result: Value,             // 结果
}

/// JSON-RPC 2.0 错误响应
#[derive(Debug, Serialize)]
pub struct JsonRpcErrorResponse {
    pub jsonrpc: String,           // "2.0"
    pub id: Value,                 // 对应请求 ID（可为 null）
    pub error: JsonRpcError,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,                 // JSON-RPC 标准错误码
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// JSON-RPC 标准错误码
pub mod error_codes {
    pub const PARSE_ERROR: i32      = -32700;
    pub const INVALID_REQUEST: i32  = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32   = -32602;
    pub const INTERNAL_ERROR: i32   = -32603;
}
```

#### 3.4.2 MCP 方法处理

```rust
use crate::tools::registry::ToolRegistry;
use std::sync::Arc;

/// MCP 协议处理器
pub struct McpProtocolHandler {
    registry: Arc<ToolRegistry>,
    context: Arc<AppContext>,
    initialized: AtomicBool,
}

impl McpProtocolHandler {
    pub fn new(registry: Arc<ToolRegistry>, context: Arc<AppContext>) -> Self {
        Self {
            registry,
            context,
            initialized: AtomicBool::new(false),
        }
    }

    /// 处理一条 JSON-RPC 消息，返回响应（notification 返回 None）
    pub async fn handle_message(
        &self,
        msg: JsonRpcRequest,
    ) -> Option<Result<JsonRpcResponse, JsonRpcErrorResponse>> {
        let id = msg.id.clone().unwrap_or(Value::Null);

        match msg.method.as_str() {
            "initialize"       => Some(self.handle_initialize(id, msg.params).await),
            "notifications/initialized" => {
                self.initialized.store(true, Ordering::SeqCst);
                None  // notification 无需响应
            }
            "tools/list"       => Some(self.handle_tools_list(id).await),
            "tools/call"       => Some(self.handle_tools_call(id, msg.params).await),
            "ping"             => Some(self.handle_ping(id).await),
            _ => {
                Some(Err(self.method_not_found(id, &msg.method)))
            }
        }
    }

    /// initialize — 握手与能力协商
    async fn handle_initialize(
        &self,
        id: Value,
        params: Value,
    ) -> Result<JsonRpcResponse, JsonRpcErrorResponse> {
        // 校验客户端 protocolVersion（当前支持 2024-11-05）
        let client_version = params
            .get("protocolVersion")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        tracing::info!("MCP initialize: client protocol version = {}", client_version);

        let result = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {
                    "listChanged": true
                }
            },
            "serverInfo": {
                "name": "obsidian-brain",
                "version": env!("CARGO_PKG_VERSION")
            }
        });

        Ok(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result,
        })
    }

    /// tools/list — 返回所有工具定义
    async fn handle_tools_list(
        &self,
        id: Value,
    ) -> Result<JsonRpcResponse, JsonRpcErrorResponse> {
        let definitions = self.registry.list_definitions().await;

        // MCP 格式：每个工具包含 name, description, inputSchema
        let tools: Vec<Value> = definitions.iter().map(|def| {
            serde_json::json!({
                "name": def.name,
                "description": def.description,
                "inputSchema": def.input_schema,
            })
        }).collect();

        Ok(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: serde_json::json!({ "tools": tools }),
        })
    }

    /// tools/call — 调用指定工具
    async fn handle_tools_call(
        &self,
        id: Value,
        params: Value,
    ) -> Result<JsonRpcResponse, JsonRpcErrorResponse> {
        let tool_name = match params.get("name").and_then(|v| v.as_str()) {
            Some(name) => name.to_string(),
            None => {
                return Err(self.invalid_params(id, "缺少 'name' 字段"));
            }
        };

        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::json!({}));

        // 调用统一的工具执行流程
        let result = self.execute_tool(&tool_name, arguments).await;

        match result {
            Ok(value) => {
                // MCP 成功响应格式
                Ok(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: serde_json::json!({
                        "content": [
                            {
                                "type": "text",
                                "text": serde_json::to_string(&value)
                                    .unwrap_or_else(|_| "{}".to_string())
                            }
                        ],
                        "isError": false
                    }),
                })
            }
            Err(err) => {
                // MCP 错误响应格式（isError: true）
                let error_json = serde_json::json!({
                    "error": {
                        "code": err.code,
                        "message": err.message,
                        "suggestion": err.suggestion,
                    }
                });

                Ok(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: serde_json::json!({
                        "content": [
                            {
                                "type": "text",
                                "text": serde_json::to_string(&error_json)
                                    .unwrap_or_else(|_| "{}".to_string())
                            }
                        ],
                        "isError": true
                    }),
                })
            }
        }
    }

    /// ping — 连接保活
    async fn handle_ping(&self, id: Value) -> Result<JsonRpcResponse, JsonRpcErrorResponse> {
        Ok(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: serde_json::json!({}),
        })
    }

    /// 统一的工具执行流程（MCP 和 HTTP 共享）
    async fn execute_tool(
        &self,
        tool_name: &str,
        arguments: Value,
    ) -> Result<Value, ToolError> {
        let start = std::time::Instant::now();
        let request_id = Uuid::new_v4();

        tracing::info!(
            request_id = %request_id,
            tool = tool_name,
            "工具调用开始"
        );

        // 1. 查找 Handler
        let handler = self.registry.get_handler(tool_name).await
            .ok_or_else(|| ToolError {
                code: "TOOL_NOT_FOUND".to_string(),
                message: format!("工具 '{}' 未找到", tool_name),
                suggestion: Some("请使用 tools/list 查看可用工具列表".to_string()),
            })?;

        // 2. 参数校验
        let def = handler.definition();
        validate_arguments(&arguments, &def.input_schema)?;

        // 3. 执行 Handler
        let result = handler.handle(arguments, &self.context).await
            .map_err(|e| brain_error_to_tool_error(e, tool_name));

        let duration = start.elapsed();

        // 4. 记录日志
        match &result {
            Ok(_) => {
                tracing::info!(
                    request_id = %request_id,
                    tool = tool_name,
                    duration_ms = duration.as_millis() as u64,
                    status = "success",
                    "工具调用完成"
                );
            }
            Err(err) => {
                tracing::warn!(
                    request_id = %request_id,
                    tool = tool_name,
                    duration_ms = duration.as_millis() as u64,
                    status = "error",
                    error_code = %err.code,
                    error_message = %err.message,
                    "工具调用失败"
                );
            }
        }

        result
    }
}
```

### 3.5 `api/handlers/tool_handler.rs` — HTTP 处理器

```rust
use axum::{
    extract::{State, Path, Query},
    Json,
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Serialize, Deserialize};
use serde_json::Value;
use std::sync::Arc;

use crate::api::router::AppState;

// ── 请求/响应类型 ──

#[derive(Debug, Deserialize)]
pub struct CallToolRequest {
    pub tool: String,
    #[serde(default)]
    pub arguments: Value,
}

#[derive(Debug, Serialize)]
pub struct ToolListResponse {
    pub tools: Vec<ToolListItem>,
}

#[derive(Debug, Serialize)]
pub struct ToolListItem {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
    pub module: String,
}

#[derive(Debug, Serialize)]
pub struct OpenAiToolListResponse {
    pub tools: Vec<OpenAiTool>,
}

#[derive(Debug, Serialize)]
pub struct OpenAiTool {
    #[serde(rename = "type")]
    pub tool_type: String,       // "function"
    pub function: OpenAiFunction,
}

#[derive(Debug, Serialize)]
pub struct OpenAiFunction {
    pub name: String,
    pub description: String,
    pub parameters: Value,       // inputSchema 直接作为 parameters
}

#[derive(Debug, Serialize)]
pub struct ToolCallResponse {
    pub tool: String,
    pub status: String,          // "success" | "error"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ToolListQuery {
    pub format: Option<String>,  // "openai" | None（默认原生格式）
    pub module: Option<String>,  // 按模块过滤
}

// ── Handler 函数 ──

/// GET /v1/tools — 获取工具列表
pub async fn list_tools(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ToolListQuery>,
) -> impl IntoResponse {
    let definitions = if let Some(module) = &query.module {
        state.registry.list_by_module(module).await
    } else {
        state.registry.list_definitions().await
    };

    match query.format.as_deref() {
        Some("openai") => {
            // OpenAI function calling 兼容格式
            let tools: Vec<OpenAiTool> = definitions.iter().map(|def| {
                OpenAiTool {
                    tool_type: "function".to_string(),
                    function: OpenAiFunction {
                        name: def.name.clone(),
                        description: def.description.clone(),
                        parameters: def.input_schema.clone(),
                    },
                }
            }).collect();

            Json(serde_json::to_value(OpenAiToolListResponse { tools }).unwrap())
        }
        _ => {
            // 原生格式
            let tools: Vec<ToolListItem> = definitions.iter().map(|def| {
                ToolListItem {
                    name: def.name.clone(),
                    description: def.description.clone(),
                    input_schema: def.input_schema.clone(),
                    module: def.module.clone(),
                }
            }).collect();

            Json(serde_json::to_value(ToolListResponse { tools }).unwrap())
        }
    }
}

/// POST /v1/tools/call — 调用工具
pub async fn call_tool(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CallToolRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let request_id = Uuid::new_v4();

    tracing::info!(
        request_id = %request_id,
        tool = %req.tool,
        "HTTP 工具调用开始"
    );

    // 查找 Handler
    let handler = match state.registry.get_handler(&req.tool).await {
        Some(h) => h,
        None => {
            let resp = ToolCallResponse {
                tool: req.tool.clone(),
                status: "error".to_string(),
                result: None,
                error: Some(ErrorBody {
                    code: "TOOL_NOT_FOUND".to_string(),
                    message: format!("工具 '{}' 未找到", req.tool),
                    suggestion: Some("使用 GET /v1/tools 查看可用工具列表".to_string()),
                }),
            };
            return Json(serde_json::to_value(resp).unwrap());
        }
    };

    // 参数校验
    let def = handler.definition();
    if let Err(e) = validate_arguments(&req.arguments, &def.input_schema) {
        let resp = ToolCallResponse {
            tool: req.tool.clone(),
            status: "error".to_string(),
            result: None,
            error: Some(ErrorBody {
                code: "INVALID_ARGUMENTS".to_string(),
                message: e.message,
                suggestion: e.suggestion,
            }),
        };
        return Json(serde_json::to_value(resp).unwrap());
    }

    // 执行 Handler
    let result = handler.handle(req.arguments.clone(), &state.context).await;

    let duration_ms = start.elapsed().as_millis() as u64;

    let resp = match result {
        Ok(value) => {
            tracing::info!(
                request_id = %request_id,
                tool = %req.tool,
                duration_ms,
                status = "success",
                "HTTP 工具调用完成"
            );
            ToolCallResponse {
                tool: req.tool,
                status: "success".to_string(),
                result: Some(value),
                error: None,
            }
        }
        Err(err) => {
            let tool_err = brain_error_to_tool_error(err, &req.tool);
            tracing::warn!(
                request_id = %request_id,
                tool = %req.tool,
                duration_ms,
                status = "error",
                error_code = %tool_err.code,
                "HTTP 工具调用失败"
            );
            ToolCallResponse {
                tool: req.tool,
                status: "error".to_string(),
                result: None,
                error: Some(ErrorBody {
                    code: tool_err.code,
                    message: tool_err.message,
                    suggestion: tool_err.suggestion,
                }),
            }
        }
    };

    Json(serde_json::to_value(resp).unwrap())
}

/// GET /v1/tools/:tool_name — 获取单个工具详情
pub async fn get_tool(
    State(state): State<Arc<AppState>>,
    Path(tool_name): Path<String>,
) -> impl IntoResponse {
    let definitions = state.registry.list_definitions().await;
    match definitions.iter().find(|d| d.name == tool_name) {
        Some(def) => (StatusCode::OK, Json(serde_json::to_value(def).unwrap())),
        None => {
            let err = serde_json::json!({
                "error": {
                    "code": "TOOL_NOT_FOUND",
                    "message": format!("工具 '{}' 未找到", tool_name),
                }
            });
            (StatusCode::NOT_FOUND, Json(err))
        }
    }
}
```

### 3.6 `api/handlers/health_handler.rs` — 健康检查

```rust
use axum::{
    extract::State,
    Json,
    response::IntoResponse,
};
use serde::Serialize;
use std::sync::Arc;

use crate::api::router::AppState;

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub components: ComponentHealth,
    pub tools_count: usize,
}

#[derive(Debug, Serialize)]
pub struct ComponentHealth {
    pub vault: String,      // "ok" | "error"
    pub qdrant: String,
    pub tantivy: String,
    pub sqlite: String,
}

/// GET /v1/health
pub async fn health_check(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let uptime = chrono::Utc::now()
        .signed_duration_since(state.start_time)
        .num_seconds() as u64;

    // 检查各组件连通性
    let qdrant_ok = state.context.search_service.check_qdrant().await;
    let tantivy_ok = state.context.search_service.check_tantivy().await;
    let sqlite_ok = state.context.memory_service.check_sqlite().await;
    let vault_ok = state.context.config.vault_path.exists();

    let all_ok = qdrant_ok && tantivy_ok && sqlite_ok && vault_ok;

    Json(HealthResponse {
        status: if all_ok { "healthy".to_string() } else { "degraded".to_string() },
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: uptime,
        components: ComponentHealth {
            vault: if vault_ok { "ok" } else { "error" }.to_string(),
            qdrant: if qdrant_ok { "ok" } else { "error" }.to_string(),
            tantivy: if tantivy_ok { "ok" } else { "error" }.to_string(),
            sqlite: if sqlite_ok { "ok" } else { "error" }.to_string(),
        },
        tools_count: state.registry.count().await,
    })
}
```

---

## 4. `tools/definitions.rs` — 工具 JSON Schema 定义

此文件定义所有工具的 `ToolDefinition`。以下展示 **5 个代表性工具** 的完整 JSON Schema。

### 4.1 工具定义构建函数

```rust
use crate::tools::registry::ToolDefinition;
use serde_json::json;

/// 构建所有内置工具的 ToolDefinition 列表
pub fn build_all_definitions() -> Vec<ToolDefinition> {
    vec![
        search_notes_def(),
        get_note_def(),
        list_recent_notes_def(),
        search_memory_def(),
        add_memory_def(),
        update_memory_def(),
        forget_memory_def(),
        get_memory_stats_def(),
        add_code_repo_def(),
        list_code_repos_def(),
        get_repo_detail_def(),
        link_note_to_repo_def(),
        generate_docs_def(),
        open_in_vscode_def(),
        get_timeline_def(),
        get_inspiration_def(),
        get_radar_def(),
        add_to_vault_def(),
        dismiss_radar_item_def(),
        get_stats_def(),
    ]
}
```

### 4.2 `search_notes` — 完整 JSON Schema

```rust
fn search_notes_def() -> ToolDefinition {
    ToolDefinition {
        name: "search_notes".to_string(),
        description: "在 Obsidian vault 中搜索笔记。支持全文搜索和语义搜索（混合检索），返回匹配的笔记片段及其 Obsidian URI 来源链接。适用于查找特定知识点、回顾过去的笔记、按标签过滤内容。".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "搜索查询词。支持关键词和自然语言查询。"
                },
                "top_k": {
                    "type": "integer",
                    "description": "返回结果数量，默认 5，最大 50",
                    "default": 5,
                    "minimum": 1,
                    "maximum": 50
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "按标签过滤结果，仅返回包含指定标签的笔记"
                }
            },
            "required": ["query"],
            "additionalProperties": false
        }),
        module: "search".to_string(),
        version: Some("1.0.0".to_string()),
    }
}
```

**MCP `tools/list` 输出片段**：

```json
{
  "name": "search_notes",
  "description": "在 Obsidian vault 中搜索笔记。支持全文搜索和语义搜索（混合检索），返回匹配的笔记片段及其 Obsidian URI 来源链接。适用于查找特定知识点、回顾过去的笔记、按标签过滤内容。",
  "inputSchema": {
    "type": "object",
    "properties": {
      "query": {
        "type": "string",
        "description": "搜索查询词。支持关键词和自然语言查询。"
      },
      "top_k": {
        "type": "integer",
        "description": "返回结果数量，默认 5，最大 50",
        "default": 5,
        "minimum": 1,
        "maximum": 50
      },
      "tags": {
        "type": "array",
        "items": { "type": "string" },
        "description": "按标签过滤结果，仅返回包含指定标签的笔记"
      }
    },
    "required": ["query"],
    "additionalProperties": false
  }
}
```

### 4.3 `add_memory` — 完整 JSON Schema

```rust
fn add_memory_def() -> ToolDefinition {
    ToolDefinition {
        name: "add_memory".to_string(),
        description: "手动添加一条记忆到知识库。记忆将写入指定笔记、建立全文索引、生成语义向量。适用于 LLM 认为某个重要信息需要被持久化记忆时使用。".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "note_path": {
                    "type": "string",
                    "description": "目标笔记的 vault 内相对路径（如 'notes/ideas.md'）。如果笔记不存在将自动创建。"
                },
                "content": {
                    "type": "string",
                    "description": "记忆文本内容。支持 Markdown 格式。",
                    "minLength": 1,
                    "maxLength": 10000
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "附加标签列表，用于分类和过滤"
                }
            },
            "required": ["note_path", "content"],
            "additionalProperties": false
        }),
        module: "memory".to_string(),
        version: Some("1.0.0".to_string()),
    }
}
```

### 4.4 `add_code_repo` — 完整 JSON Schema

```rust
fn add_code_repo_def() -> ToolDefinition {
    ToolDefinition {
        name: "add_code_repo".to_string(),
        description: "注册一个本地代码仓库到 ObsidianBrain。注册后将自动提取仓库元信息（分支、最近提交、语言统计），并建立与 Obsidian 笔记的关联能力。".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "代码仓库的本地绝对路径（如 '/Users/me/projects/my-app'）"
                },
                "name": {
                    "type": "string",
                    "description": "仓库显示名称（用于后续引用，如 'my-app'）",
                    "pattern": "^[a-zA-Z0-9_-]+$",
                    "minLength": 1,
                    "maxLength": 64
                }
            },
            "required": ["path", "name"],
            "additionalProperties": false
        }),
        module: "code_repo".to_string(),
        version: Some("1.0.0".to_string()),
    }
}
```

### 4.5 `get_inspiration` — 完整 JSON Schema

```rust
fn get_inspiration_def() -> ToolDefinition {
    ToolDefinition {
        name: "get_inspiration".to_string(),
        description: "触发灵感熔炉，基于用户知识库生成创意灵感。三种模式：concept_combo（随机概念组合，跨界碰撞）、reverse_question（反向提问，挖掘盲区）、counterpoint（对立观点，批判性思考）。当用户需要新想法、思维拓展或反思时使用。".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "type": {
                    "type": "string",
                    "enum": ["concept_combo", "reverse_question", "counterpoint"],
                    "description": "灵感模式。concept_combo: 从知识库中随机选取两个远距离概念进行跨界组合；reverse_question: 针对一篇笔记生成你可能没想过的问题；counterpoint: 对笔记观点生成反方论证和逻辑漏洞分析",
                    "default": "concept_combo"
                },
                "note_path": {
                    "type": "string",
                    "description": "指定笔记路径（用于 reverse_question 和 counterpoint 模式）。如果不指定则使用最近修改的笔记。"
                }
            },
            "required": [],
            "additionalProperties": false
        }),
        module: "inspiration".to_string(),
        version: Some("1.0.0".to_string()),
    }
}
```

### 4.6 `get_radar` — 完整 JSON Schema

```rust
fn get_radar_def() -> ToolDefinition {
    ToolDefinition {
        name: "get_radar".to_string(),
        description: "获取智识雷达推荐——基于你的知识图谱，从外部信息源（arXiv、Hacker News、RSS、Reddit）中筛选出与你当前兴趣最相关的文章和论文。结果按语义相关性排序，并附带与你笔记的关联。".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "description": "返回推荐条目数量，默认 10，最大 50",
                    "default": 10,
                    "minimum": 1,
                    "maximum": 50
                },
                "query": {
                    "type": "string",
                    "description": "可选的查询词，用于进一步过滤推荐结果。如不指定则返回按相关性排序的全部推荐。"
                }
            },
            "required": [],
            "additionalProperties": false
        }),
        module: "radar".to_string(),
        version: Some("1.0.0".to_string()),
    }
}
```

### 4.7 其余工具 Schema 概览

<details>
<summary>展开查看其余工具的 inputSchema 定义</summary>

**`get_note`**:
```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "笔记的 vault 内相对路径（如 'programming/rust-async.md'）"
    }
  },
  "required": ["path"],
  "additionalProperties": false
}
```

**`list_recent_notes`**:
```json
{
  "type": "object",
  "properties": {
    "days": {
      "type": "integer",
      "description": "查询最近几天的笔记，默认 7",
      "default": 7,
      "minimum": 1
    },
    "limit": {
      "type": "integer",
      "description": "返回数量上限，默认 20",
      "default": 20,
      "minimum": 1,
      "maximum": 100
    }
  },
  "required": [],
  "additionalProperties": false
}
```

**`search_memory`**:
```json
{
  "type": "object",
  "properties": {
    "query": { "type": "string", "description": "搜索查询词" },
    "top_k": { "type": "integer", "default": 5, "minimum": 1, "maximum": 50 },
    "tags": { "type": "array", "items": { "type": "string" } }
  },
  "required": ["query"],
  "additionalProperties": false
}
```

**`update_memory`**:
```json
{
  "type": "object",
  "properties": {
    "memory_id": { "type": "string", "format": "uuid" },
    "content": { "type": "string", "minLength": 1, "maxLength": 10000 }
  },
  "required": ["memory_id", "content"],
  "additionalProperties": false
}
```

**`forget_memory`**:
```json
{
  "type": "object",
  "properties": {
    "memory_id": { "type": "string", "format": "uuid" }
  },
  "required": ["memory_id"],
  "additionalProperties": false
}
```

**`get_memory_stats`**:
```json
{
  "type": "object",
  "properties": {},
  "additionalProperties": false
}
```

**`list_code_repos`**:
```json
{
  "type": "object",
  "properties": {},
  "additionalProperties": false
}
```

**`get_repo_detail`**:
```json
{
  "type": "object",
  "properties": {
    "name": { "type": "string", "description": "仓库名称" }
  },
  "required": ["name"],
  "additionalProperties": false
}
```

**`link_note_to_repo`**:
```json
{
  "type": "object",
  "properties": {
    "note_path": { "type": "string" },
    "repo_name": { "type": "string" }
  },
  "required": ["note_path", "repo_name"],
  "additionalProperties": false
}
```

**`generate_docs`**:
```json
{
  "type": "object",
  "properties": {
    "repo_name": { "type": "string" },
    "target_path": { "type": "string", "description": "输出目录的 vault 内相对路径，默认 'docs/'" }
  },
  "required": ["repo_name"],
  "additionalProperties": false
}
```

**`open_in_vscode`**:
```json
{
  "type": "object",
  "properties": {
    "repo_name": { "type": "string" }
  },
  "required": ["repo_name"],
  "additionalProperties": false
}
```

**`get_timeline`**:
```json
{
  "type": "object",
  "properties": {
    "start_date": { "type": "string", "format": "date", "description": "起始日期（YYYY-MM-DD）" },
    "end_date": { "type": "string", "format": "date", "description": "结束日期（YYYY-MM-DD）" }
  },
  "required": ["start_date", "end_date"],
  "additionalProperties": false
}
```

**`add_to_vault`**:
```json
{
  "type": "object",
  "properties": {
    "article_id": { "type": "string" },
    "target_dir": { "type": "string", "description": "vault 内目标目录，默认 'radar/'" }
  },
  "required": ["article_id"],
  "additionalProperties": false
}
```

**`dismiss_radar_item`**:
```json
{
  "type": "object",
  "properties": {
    "article_id": { "type": "string" }
  },
  "required": ["article_id"],
  "additionalProperties": false
}
```

**`get_stats`**:
```json
{
  "type": "object",
  "properties": {},
  "additionalProperties": false
}
```

</details>

---

## 5. MCP Server 实现细节

### 5.1 stdio 传输模式 (`api/mcp_stdio.rs`)

stdio 模式是 Claude Desktop 的标准接入方式。ObsidianBrain 作为子进程运行，通过 stdin/stdout 交换 JSON-RPC 消息。

```rust
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::sync::Arc;

/// MCP stdio 传输层
pub struct McpStdioTransport {
    protocol: Arc<McpProtocolHandler>,
}

impl McpStdioTransport {
    pub fn new(protocol: Arc<McpProtocolHandler>) -> Self {
        Self { protocol }
    }

    /// 启动 stdio 传输，阻塞运行直到 stdin 关闭
    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();

        tracing::info!("MCP stdio 传输已启动，等待请求...");

        while let Some(line) = lines.next_line().await? {
            let line = line.trim().to_string();
            if line.is_empty() {
                continue;
            }

            tracing::debug!(raw_input = %line, "收到 MCP 消息");

            // 解析 JSON-RPC 请求
            let request: JsonRpcRequest = match serde_json::from_str(&line) {
                Ok(req) => req,
                Err(e) => {
                    let error_resp = JsonRpcErrorResponse {
                        jsonrpc: "2.0".to_string(),
                        id: serde_json::Value::Null,
                        error: JsonRpcError {
                            code: error_codes::PARSE_ERROR,
                            message: format!("JSON 解析失败: {}", e),
                            data: None,
                        },
                    };
                    let resp_json = serde_json::to_string(&error_resp)?;
                    stdout.write_all(resp_json.as_bytes()).await?;
                    stdout.write_all(b"\n").await?;
                    stdout.flush().await?;
                    continue;
                }
            };

            // 处理消息
            if let Some(response) = self.protocol.handle_message(request).await {
                let resp_json = match response {
                    Ok(resp) => serde_json::to_string(&resp)?,
                    Err(err_resp) => serde_json::to_string(&err_resp)?,
                };

                tracing::debug!(raw_output = %resp_json, "发送 MCP 响应");

                stdout.write_all(resp_json.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
            }
            // notification（返回 None）不写 stdout
        }

        tracing::info!("MCP stdio 传输已停止（stdin 关闭）");
        Ok(())
    }
}
```

**消息分帧**：stdio 模式使用换行符 `\n` 作为消息分隔符（每行一条 JSON-RPC 消息），这是 MCP 标准规定的分帧方式。

**Claude Desktop 配置示例**：

```json
{
  "mcpServers": {
    "obsidian-brain": {
      "command": "/usr/local/bin/obsidianbrain",
      "args": ["serve", "--protocol", "mcp"],
      "env": {
        "RUST_LOG": "info"
      }
    }
  }
}
```

### 5.2 SSE 传输模式 (`api/mcp_sse.rs`)（可选）

SSE 模式通过 HTTP 提供 MCP 协议支持，适用于 Web 前端或自定义客户端。

```rust
use axum::{
    Router,
    routing::{get, post},
    extract::State,
    response::sse::{Sse, Event},
    Json,
};
use tokio_stream::wrappers::BroadcastStream;
use std::sync::Arc;

/// SSE 传输状态
pub struct McpSseState {
    protocol: Arc<McpProtocolHandler>,
    response_tx: tokio::sync::broadcast::Sender<String>,
}

/// 构建 SSE 路由（挂载到主 Axum 服务上）
pub fn build_sse_routes(state: Arc<McpSseState>) -> Router {
    Router::new()
        .route("/mcp/sse", get(sse_stream))
        .route("/mcp/message", post(post_message))
        .with_state(state)
}

/// GET /mcp/sse — 建立 SSE 连接
async fn sse_stream(
    State(state): State<Arc<McpSseState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = state.response_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|result| {
        match result {
            Ok(msg) => Some(Ok(Event::default().data(msg))),
            Err(_) => None,
        }
    });

    // 发送 endpoint 事件，告知客户端 POST 端点地址
    let init_event = Event::default()
        .event("endpoint")
        .data("/mcp/message");

    let stream = tokio_stream::once(Ok(init_event)).chain(stream);

    Sse::new(stream)
}

/// POST /mcp/message — 接收 MCP 消息
async fn post_message(
    State(state): State<Arc<McpSseState>>,
    Json(request): Json<JsonRpcRequest>,
) -> axum::http::StatusCode {
    if let Some(response) = state.protocol.handle_message(request).await {
        let resp_json = match response {
            Ok(resp) => serde_json::to_string(&resp).unwrap(),
            Err(err_resp) => serde_json::to_string(&err_resp).unwrap(),
        };
        let _ = state.response_tx.send(resp_json);
    }
    axum::http::StatusCode::ACCEPTED
}
```

### 5.3 工具列表序列化

MCP `tools/list` 响应中，工具列表需遵循 MCP 规范序列化。每个工具包含三个字段：

```json
{
  "name": "search_notes",
  "description": "在 Obsidian vault 中搜索笔记...",
  "inputSchema": { ... }
}
```

**注意**：MCP 规范中 `inputSchema` 字段使用 camelCase（不是 `input_schema`），序列化时需通过 `#[serde(rename = "inputSchema")]` 处理。

### 5.4 调用结果格式化

MCP `tools/call` 的响应格式为 `CallToolResult`：

```json
{
  "content": [
    {
      "type": "text",
      "text": "{ ... 工具结果的 JSON 字符串 ... }"
    }
  ],
  "isError": false
}
```

**设计决策**：我们将工具返回的 `serde_json::Value` 序列化为 JSON 字符串放入 `text` 字段。这样 LLM 可以直接解析结构化数据，比纯文本更易于程序化处理。

**错误情况**：

```json
{
  "content": [
    {
      "type": "text",
      "text": "{\"error\":{\"code\":\"NOTE_NOT_FOUND\",\"message\":\"...\",\"suggestion\":\"...\"}}"
    }
  ],
  "isError": true
}
```

注意：MCP 错误**不使用** JSON-RPC error response（那表示协议层错误），而是使用 `isError: true` + `content` 表示工具执行层面的错误。

---

## 6. HTTP API 实现细节

### 6.1 `GET /v1/tools` 响应格式

**默认格式**（原生）：

```json
{
  "tools": [
    {
      "name": "search_notes",
      "description": "在 Obsidian vault 中搜索笔记...",
      "inputSchema": {
        "type": "object",
        "properties": {
          "query": { "type": "string", "description": "搜索查询词" },
          "top_k": { "type": "integer", "default": 5 },
          "tags": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["query"]
      },
      "module": "search"
    },
    ...
  ]
}
```

**OpenAI 兼容格式**（`?format=openai`）：

```json
{
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "search_notes",
        "description": "在 Obsidian vault 中搜索笔记...",
        "parameters": {
          "type": "object",
          "properties": {
            "query": { "type": "string", "description": "搜索查询词" },
            "top_k": { "type": "integer", "default": 5 },
            "tags": { "type": "array", "items": { "type": "string" } }
          },
          "required": ["query"]
        }
      }
    },
    ...
  ]
}
```

### 6.2 `POST /v1/tools/call` 请求/响应格式

**请求**：

```json
{
  "tool": "search_notes",
  "arguments": {
    "query": "Rust 异步编程",
    "top_k": 5,
    "tags": ["rust", "async"]
  }
}
```

**成功响应** (HTTP 200)：

```json
{
  "tool": "search_notes",
  "status": "success",
  "result": {
    "notes": [
      {
        "path": "programming/rust-async.md",
        "title": "Rust 异步编程笔记",
        "snippet": "tokio::select! 宏允许同时等待多个 Future...",
        "score": 0.92,
        "tags": ["rust", "async", "tokio"],
        "obsidian_uri": "obsidian://open?vault=brain&file=programming/rust-async.md",
        "updated_at": "2026-05-28T14:30:00Z"
      }
    ],
    "total": 1
  }
}
```

**错误响应** (HTTP 200，业务错误)：

```json
{
  "tool": "search_notes",
  "status": "error",
  "error": {
    "code": "SEARCH_ERROR",
    "message": "全文索引查询失败：Tantivy 内部错误",
    "suggestion": "请稍后重试，或尝试简化搜索关键词"
  }
}
```

**协议错误** (HTTP 400)：

```json
{
  "error": {
    "code": "BAD_REQUEST",
    "message": "请求体 JSON 解析失败: expected `:` at line 3 column 5"
  }
}
```

### 6.3 错误响应格式汇总

| 场景 | HTTP 状态码 | 响应体格式 |
|---|---|---|
| 请求体 JSON 格式错误 | 400 | `{ "error": { "code": "BAD_REQUEST", "message": "..." } }` |
| 工具不存在 | 200 | `{ "tool": "...", "status": "error", "error": { "code": "TOOL_NOT_FOUND", ... } }` |
| 参数校验失败 | 200 | `{ "tool": "...", "status": "error", "error": { "code": "INVALID_ARGUMENTS", ... } }` |
| 工具执行失败 | 200 | `{ "tool": "...", "status": "error", "error": { "code": "...", ... } }` |
| 服务未就绪 | 503 | `{ "error": { "code": "SERVICE_UNAVAILABLE", "message": "..." } }` |
| 内部未预期错误 | 200 | `{ "tool": "...", "status": "error", "error": { "code": "INTERNAL_ERROR", ... } }` |

---

## 7. 工具注册流程

### 7.1 启动时自动扫描 + 注册

```rust
/// 初始化所有内置工具并注册到 Registry
pub async fn initialize_tools(registry: &ToolRegistry, ctx: Arc<AppContext>) {
    tracing::info!("开始注册内置工具...");

    let handlers: Vec<Arc<dyn ToolHandler>> = vec![
        // 笔记检索模块
        Arc::new(SearchNotesHandler { ctx: ctx.clone() }),
        Arc::new(GetNoteHandler { ctx: ctx.clone() }),
        Arc::new(ListRecentNotesHandler { ctx: ctx.clone() }),

        // 记忆管理模块
        Arc::new(SearchMemoryHandler { ctx: ctx.clone() }),
        Arc::new(AddMemoryHandler { ctx: ctx.clone() }),
        Arc::new(UpdateMemoryHandler { ctx: ctx.clone() }),
        Arc::new(ForgetMemoryHandler { ctx: ctx.clone() }),
        Arc::new(GetMemoryStatsHandler { ctx: ctx.clone() }),

        // 代码仓管理模块
        Arc::new(AddCodeRepoHandler { ctx: ctx.clone() }),
        Arc::new(ListCodeReposHandler { ctx: ctx.clone() }),
        Arc::new(GetRepoDetailHandler { ctx: ctx.clone() }),
        Arc::new(LinkNoteToRepoHandler { ctx: ctx.clone() }),
        Arc::new(GenerateDocsHandler { ctx: ctx.clone() }),
        Arc::new(OpenInVscodeHandler { ctx: ctx.clone() }),

        // 时间线模块
        Arc::new(GetTimelineHandler { ctx: ctx.clone() }),

        // 灵感熔炉模块
        Arc::new(GetInspirationHandler { ctx: ctx.clone() }),

        // 智识雷达模块
        Arc::new(GetRadarHandler { ctx: ctx.clone() }),
        Arc::new(AddToVaultHandler { ctx: ctx.clone() }),
        Arc::new(DismissRadarItemHandler { ctx: ctx.clone() }),

        // 系统模块
        Arc::new(GetStatsHandler { ctx: ctx.clone() }),
    ];

    for handler in handlers {
        registry.register(handler).await;
    }

    let count = registry.count().await;
    tracing::info!("内置工具注册完成，共 {} 个工具", count);
}
```

### 7.2 Handler 实现示例（SearchNotesHandler）

```rust
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

pub struct SearchNotesHandler {
    pub ctx: Arc<AppContext>,
}

#[derive(Debug, Deserialize)]
struct SearchNotesArgs {
    query: String,
    top_k: Option<usize>,
    tags: Option<Vec<String>>,
}

#[async_trait]
impl ToolHandler for SearchNotesHandler {
    fn name(&self) -> &str {
        "search_notes"
    }

    fn definition(&self) -> ToolDefinition {
        definitions::search_notes_def()
    }

    async fn handle(
        &self,
        args: Value,
        ctx: &AppContext,
    ) -> Result<Value, BrainError> {
        let params: SearchNotesArgs = serde_json::from_value(args)
            .map_err(|e| BrainError::Internal(
                format!("参数反序列化失败: {}", e)
            ))?;

        let top_k = params.top_k.unwrap_or(5);

        // 调用 SearchService 执行混合搜索
        let results = ctx.search_service
            .hybrid_search(&params.query, top_k, params.tags.as_deref())
            .await?;

        // 组装返回结果
        let notes: Vec<Value> = results.iter().map(|r| {
            json!({
                "path": r.path,
                "title": r.title,
                "snippet": r.snippet,
                "score": r.score,
                "tags": r.tags,
                "obsidian_uri": r.obsidian_uri,
                "updated_at": r.updated_at,
            })
        }).collect();

        Ok(json!({
            "notes": notes,
            "total": notes.len(),
        }))
    }
}
```

---

## 8. 请求处理 Pipeline

### 8.1 完整 Pipeline 流程

```
                    ┌─────────────────┐
                    │   原始请求到达    │
                    │ (HTTP/MCP bytes) │
                    └────────┬────────┘
                             │
                    ┌────────▼────────┐
              ①    │   消息解析        │  将原始字节解析为结构化请求
                    │  (Parse)         │  HTTP: Axum JSON 反序列化
                    │                  │  MCP: 行分割 + JSON-RPC 解析
                    └────────┬────────┘
                             │
                    ┌────────▼────────┐
              ②    │  Request ID 注入  │  生成 UUID，注入 tracing span
                    │  (Trace)         │
                    └────────┬────────┘
                             │
                    ┌────────▼────────┐
              ③    │   工具查找        │  ToolRegistry.get(name)
                    │  (Lookup)        │  未找到 → TOOL_NOT_FOUND
                    └────────┬────────┘
                             │
                    ┌────────▼────────┐
              ④    │   参数校验        │  jsonschema::validate(args, input_schema)
                    │  (Validate)      │  失败 → INVALID_ARGUMENTS
                    └────────┬────────┘
                             │
                    ┌────────▼────────┐
              ⑤    │   超时控制        │  tokio::time::timeout(30s, ...)
                    │  (Timeout)       │  超时 → TOOL_TIMEOUT
                    └────────┬────────┘
                             │
                    ┌────────▼────────┐
              ⑥    │   Handler 执行    │  handler.handle(args, ctx).await
                    │  (Execute)       │  调用核心 Service 层
                    └────────┬────────┘
                             │
                    ┌────────▼────────┐
              ⑦    │   结果格式化      │  包装为标准响应信封
                    │  (Format)        │  HTTP: { tool, status, result/error }
                    │                  │  MCP: { content, isError }
                    └────────┬────────┘
                             │
                    ┌────────▼────────┐
              ⑧    │   日志记录        │  tracing::info/warn!
                    │  (Log)           │  request_id, tool, duration, status
                    └────────┬────────┘
                             │
                    ┌────────▼────────┐
                    │   发送响应        │
                    │  (Respond)       │
                    └─────────────────┘
```

### 8.2 参数校验实现

```rust
use jsonschema::{JSONSchema, Draft};

/// 校验参数是否符合 JSON Schema
pub fn validate_arguments(
    args: &Value,
    schema: &Value,
) -> Result<(), ToolError> {
    let compiled = JSONSchema::options()
        .with_draft(Draft::Draft7)
        .compile(schema)
        .map_err(|e| ToolError {
            code: "INTERNAL_ERROR".to_string(),
            message: format!("Schema 编译失败: {}", e),
            suggestion: None,
        })?;

    let result = compiled.validate(args);
    if let Err(errors) = result {
        let error_messages: Vec<String> = errors
            .take(5)  // 最多展示 5 个校验错误
            .map(|e| format!("{}: {}", e.instance_path, e))
            .collect();

        return Err(ToolError {
            code: "INVALID_ARGUMENTS".to_string(),
            message: format!("参数校验失败:\n{}", error_messages.join("\n")),
            suggestion: Some("请检查参数格式是否符合工具的 inputSchema 定义".to_string()),
        });
    }

    Ok(())
}
```

### 8.3 错误转换

```rust
/// 将 BrainError 映射为标准 ToolError
pub fn brain_error_to_tool_error(err: BrainError, tool_name: &str) -> ToolError {
    match err {
        BrainError::NoteNotFound(path) => ToolError {
            code: "NOTE_NOT_FOUND".to_string(),
            message: format!("笔记 '{}' 未找到", path.display()),
            suggestion: Some("请使用 search_notes 搜索笔记，或使用 list_recent_notes 查看最近笔记".to_string()),
        },
        BrainError::RepoNotFound(path) => ToolError {
            code: "REPO_NOT_FOUND".to_string(),
            message: format!("代码仓库 '{}' 未找到", path.display()),
            suggestion: Some("请先使用 add_code_repo 注册仓库，或使用 list_code_repos 查看已注册仓库".to_string()),
        },
        BrainError::GitError { path, detail } => ToolError {
            code: "GIT_ERROR".to_string(),
            message: format!("Git 操作失败 ({}): {}", path.display(), detail),
            suggestion: Some("请检查仓库状态是否正常".to_string()),
        },
        BrainError::SearchError(detail) => ToolError {
            code: "SEARCH_ERROR".to_string(),
            message: format!("搜索执行失败: {}", detail),
            suggestion: Some("请稍后重试，或尝试简化搜索关键词".to_string()),
        },
        BrainError::EmbeddingError(detail) => ToolError {
            code: "EMBEDDING_ERROR".to_string(),
            message: format!("Embedding 生成失败: {}", detail),
            suggestion: Some("语义搜索暂不可用，全文搜索仍然可用".to_string()),
        },
        BrainError::QdrantError(detail) => ToolError {
            code: "QDRANT_UNAVAILABLE".to_string(),
            message: format!("Qdrant 向量库不可用: {}", detail),
            suggestion: Some("已自动降级为全文搜索模式".to_string()),
        },
        BrainError::LlmApiError { provider, detail } => ToolError {
            code: "LLM_API_ERROR".to_string(),
            message: format!("LLM API 调用失败 ({}): {}", provider, detail),
            suggestion: Some("请稍后重试，或检查 LLM 配置".to_string()),
        },
        BrainError::IoError(e) => ToolError {
            code: "FILE_WRITE_ERROR".to_string(),
            message: format!("I/O 错误: {}", e),
            suggestion: Some("请检查 vault 路径权限和磁盘空间".to_string()),
        },
        _ => ToolError {
            code: "INTERNAL_ERROR".to_string(),
            message: format!("内部错误: {}", err),
            suggestion: Some("请查看服务端日志获取详细信息".to_string()),
        },
    }
}
```

---

## 9. 日志与监控

### 9.1 结构化日志设计

使用 `tracing` crate 的结构化日志能力，每次工具调用创建独立的 span：

```rust
use tracing::{info_span, Instrument};

/// 在工具执行时创建 tracing span
async fn execute_with_span(
    handler: &dyn ToolHandler,
    args: Value,
    ctx: &AppContext,
    request_id: Uuid,
    tool_name: &str,
) -> Result<Value, BrainError> {
    let span = info_span!(
        "tool_call",
        request_id = %request_id,
        tool = tool_name,
    );

    async {
        let start = std::time::Instant::now();

        tracing::debug!(arguments = %args, "工具调用参数");

        let result = handler.handle(args, ctx).await;
        let duration_ms = start.elapsed().as_millis() as u64;

        match &result {
            Ok(value) => {
                tracing::info!(
                    duration_ms,
                    status = "success",
                    result_size = serde_json::to_string(value)
                        .map(|s| s.len())
                        .unwrap_or(0),
                    "工具调用成功"
                );
                tracing::debug!(result = %value, "工具调用结果");
            }
            Err(err) => {
                tracing::warn!(
                    duration_ms,
                    status = "error",
                    error = %err,
                    "工具调用失败"
                );
            }
        }

        result
    }
    .instrument(span)
    .await
}
```

### 9.2 日志级别说明

| 级别 | 记录内容 | 使用场景 |
|---|---|---|
| `error` | panic 恢复、不可恢复错误 | 生产环境排障 |
| `warn` | 工具执行失败、降级事件、参数校验失败 | 日常监控 |
| `info` | 工具调用概要（工具名、耗时、状态）、注册事件 | 正常运行 |
| `debug` | 完整请求/响应 JSON、中间状态 | 开发调试 |
| `trace` | 传输层原始消息、JSON-RPC 消息细节 | 深度排障 |

### 9.3 日志输出示例

```
2026-05-29T10:23:45.123Z  INFO tool_call{request_id=550e8400 tool=search_notes}: obsidian_brain::api: 工具调用开始
2026-05-29T10:23:45.245Z  INFO tool_call{request_id=550e8400 tool=search_notes}: obsidian_brain::api: 工具调用成功 duration_ms=122 status=success result_size=2340
2026-05-29T10:23:46.100Z  WARN tool_call{request_id=6ba7b810 tool=get_note}: obsidian_brain::api: 工具调用失败 duration_ms=5 status=error error="笔记 'nonexistent.md' 未找到"
```

---

## 10. 服务启动流程

### 10.1 `api/mod.rs` — 服务启动入口

```rust
use std::sync::Arc;
use std::net::SocketAddr;

/// 根据配置启动对应协议的服务
pub async fn start_server(
    config: &AppConfig,
    context: Arc<AppContext>,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. 创建工具注册表
    let registry = Arc::new(ToolRegistry::new());

    // 2. 注册所有内置工具
    initialize_tools(&registry, context.clone()).await;

    // 3. 创建应用状态
    let state = Arc::new(AppState {
        registry: registry.clone(),
        context: context.clone(),
        start_time: chrono::Utc::now(),
    });

    // 4. 根据配置启动对应协议
    match config.server.protocol.as_str() {
        "mcp" => {
            start_mcp_stdio(registry, context).await?;
        }
        "http" => {
            start_http(state, config).await?;
        }
        "both" => {
            // HTTP 在后台运行
            let state_clone = state.clone();
            let config_clone = config.clone();
            let http_handle = tokio::spawn(async move {
                start_http(state_clone, &config_clone).await
            });

            // MCP stdio 在前台运行（阻塞 stdin/stdout）
            let protocol = Arc::new(McpProtocolHandler::new(
                registry.clone(),
                context.clone(),
            ));
            let transport = McpStdioTransport::new(protocol);

            // 注意：both 模式下 HTTP 服务独立运行，MCP 走 stdio
            tokio::select! {
                result = transport.run() => {
                    result?;
                }
                result = http_handle => {
                    result??;
                }
            }
        }
        _ => {
            return Err(format!("未知协议模式: {}", config.server.protocol).into());
        }
    }

    Ok(())
}

/// 启动 HTTP 服务
async fn start_http(
    state: Arc<AppState>,
    config: &AppConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let router = build_router(state);

    let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port)
        .parse()?;

    tracing::info!("HTTP 服务启动: http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// 启动 MCP stdio 服务
async fn start_mcp_stdio(
    registry: Arc<ToolRegistry>,
    context: Arc<AppContext>,
) -> Result<(), Box<dyn std::error::Error>> {
    let protocol = Arc::new(McpProtocolHandler::new(registry, context));
    let transport = McpStdioTransport::new(protocol);

    tracing::info!("MCP stdio 服务启动");
    transport.run().await?;

    Ok(())
}

/// 优雅关闭信号
async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("无法注册 Ctrl+C 信号");
    tracing::info!("收到关闭信号，正在优雅关闭...");
}
```

---

## 11. 测试策略

### 11.1 单元测试

| 测试模块 | 测试内容 | 测试方式 |
|---|---|---|
| `tools/registry.rs` | 注册/注销/查找/列表 | Mock Handler，直接调用 Registry API |
| `tools/definitions.rs` | Schema 格式正确性 | 验证每个 Schema 可被 `jsonschema` 编译 |
| `api/tool_protocol.rs` | JSON-RPC 消息解析与响应 | 构造 `JsonRpcRequest`，验证响应结构 |
| `api/handlers/` | 参数校验、错误映射 | Mock AppContext，测试各种参数场景 |

### 11.2 协议一致性测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mcp_initialize() {
        let (registry, ctx) = setup_test_env();
        let handler = McpProtocolHandler::new(registry, ctx);

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "initialize".to_string(),
            params: json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "test", "version": "1.0" }
            }),
        };

        let resp = handler.handle_message(req).await.unwrap().unwrap();
        assert_eq!(resp.result["protocolVersion"], "2024-11-05");
        assert_eq!(resp.result["capabilities"]["tools"]["listChanged"], true);
    }

    #[tokio::test]
    async fn test_mcp_tools_list_format() {
        let (registry, ctx) = setup_test_env();
        register_test_tools(&registry).await;
        let handler = McpProtocolHandler::new(registry, ctx);

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(2)),
            method: "tools/list".to_string(),
            params: json!({}),
        };

        let resp = handler.handle_message(req).await.unwrap().unwrap();
        let tools = resp.result["tools"].as_array().unwrap();
        assert!(tools.len() > 0);

        // 验证每个工具都有 name, description, inputSchema
        for tool in tools {
            assert!(tool.get("name").is_some());
            assert!(tool.get("description").is_some());
            assert!(tool.get("inputSchema").is_some());
        }
    }

    #[tokio::test]
    async fn test_mcp_tools_call_success() {
        let (registry, ctx) = setup_test_env();
        register_test_tools(&registry).await;
        let handler = McpProtocolHandler::new(registry, ctx);

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(3)),
            method: "tools/call".to_string(),
            params: json!({
                "name": "get_memory_stats",
                "arguments": {}
            }),
        };

        let resp = handler.handle_message(req).await.unwrap().unwrap();
        assert_eq!(resp.result["isError"], false);
        assert!(resp.result["content"].as_array().unwrap().len() > 0);
    }

    #[tokio::test]
    async fn test_mcp_tools_call_not_found() {
        let (registry, ctx) = setup_test_env();
        let handler = McpProtocolHandler::new(registry, ctx);

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(4)),
            method: "tools/call".to_string(),
            params: json!({
                "name": "nonexistent_tool",
                "arguments": {}
            }),
        };

        let resp = handler.handle_message(req).await.unwrap().unwrap();
        assert_eq!(resp.result["isError"], true);

        let text = resp.result["content"][0]["text"].as_str().unwrap();
        let error_obj: Value = serde_json::from_str(text).unwrap();
        assert_eq!(error_obj["error"]["code"], "TOOL_NOT_FOUND");
    }

    #[tokio::test]
    async fn test_mcp_invalid_json_rpc() {
        let (registry, ctx) = setup_test_env();
        let handler = McpProtocolHandler::new(registry, ctx);

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(5)),
            method: "unknown/method".to_string(),
            params: json!({}),
        };

        let resp = handler.handle_message(req).await.unwrap();
        assert!(resp.is_err());
        let err = resp.unwrap_err();
        assert_eq!(err.error.code, error_codes::METHOD_NOT_FOUND);
    }
}
```

### 11.3 HTTP 端到端测试

```rust
#[cfg(test)]
mod http_tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_get_tools() {
        let app = setup_test_router();

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/v1/tools")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert!(json["tools"].as_array().unwrap().len() > 0);
    }

    #[tokio::test]
    async fn test_get_tools_openai_format() {
        let app = setup_test_router();

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/v1/tools?format=openai")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        // 验证 OpenAI 格式
        let tools = json["tools"].as_array().unwrap();
        for tool in tools {
            assert_eq!(tool["type"], "function");
            assert!(tool["function"]["name"].is_string());
            assert!(tool["function"]["parameters"].is_object());
        }
    }

    #[tokio::test]
    async fn test_call_tool_success() {
        let app = setup_test_router();

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/tools/call")
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_string(&json!({
                        "tool": "get_memory_stats",
                        "arguments": {}
                    })).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["tool"], "get_memory_stats");
        assert_eq!(json["status"], "success");
    }

    #[tokio::test]
    async fn test_call_tool_not_found() {
        let app = setup_test_router();

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/tools/call")
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_string(&json!({
                        "tool": "nonexistent",
                        "arguments": {}
                    })).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "error");
        assert_eq!(json["error"]["code"], "TOOL_NOT_FOUND");
    }

    #[tokio::test]
    async fn test_call_tool_invalid_arguments() {
        let app = setup_test_router();

        // search_notes 需要 query 参数，这里故意不传
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/tools/call")
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_string(&json!({
                        "tool": "search_notes",
                        "arguments": {}
                    })).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "error");
        assert_eq!(json["error"]["code"], "INVALID_ARGUMENTS");
    }

    #[tokio::test]
    async fn test_health_check() {
        let app = setup_test_router();

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/v1/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert!(json["status"].is_string());
        assert!(json["tools_count"].is_number());
    }
}
```

### 11.4 工具调用端到端测试

```rust
/// 端到端测试：完整的 MCP 工具调用流程
#[tokio::test]
async fn test_e2e_mcp_tool_call_flow() {
    // 1. 设置测试环境（含真实的 SQLite、内存 Tantivy 索引）
    let (registry, ctx) = setup_test_env_with_real_data();
    let handler = McpProtocolHandler::new(registry, ctx);

    // 2. 初始化握手
    let init_resp = handler.handle_message(JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "initialize".to_string(),
        params: json!({ "protocolVersion": "2024-11-05" }),
    }).await.unwrap().unwrap();
    assert_eq!(init_resp.result["protocolVersion"], "2024-11-05");

    // 3. 获取工具列表
    let list_resp = handler.handle_message(JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(2)),
        method: "tools/list".to_string(),
        params: json!({}),
    }).await.unwrap().unwrap();
    let tools = list_resp.result["tools"].as_array().unwrap();
    assert!(tools.len() >= 20);  // 至少 20 个内置工具

    // 4. 调用搜索工具
    let call_resp = handler.handle_message(JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(3)),
        method: "tools/call".to_string(),
        params: json!({
            "name": "search_notes",
            "arguments": { "query": "test", "top_k": 3 }
        }),
    }).await.unwrap().unwrap();
    assert_eq!(call_resp.result["isError"], false);

    // 5. 验证返回格式
    let content = &call_resp.result["content"][0];
    assert_eq!(content["type"], "text");
    let result: Value = serde_json::from_str(content["text"].as_str().unwrap()).unwrap();
    assert!(result.get("notes").is_some());
}
```

---

## 12. 依赖清单

### 12.1 直接依赖

| Crate | 版本 | 用途 | 所属层 |
|---|---|---|---|
| `axum` | `0.8` | HTTP 框架、路由、中间件 | api |
| `tokio` | `1` (features: full) | 异步运行时、I/O、信号处理 | 全局 |
| `serde` | `1` (features: derive) | 序列化/反序列化 | 全局 |
| `serde_json` | `1` | JSON 处理 | 全局 |
| `tower` | `0.5` | 中间件抽象 | api |
| `tower-http` | `0.6` (features: cors, trace) | HTTP 中间件（CORS、日志追踪） | api |
| `tracing` | `0.1` | 结构化日志 | 全局 |
| `tracing-subscriber` | `0.3` | 日志输出配置 | main |
| `uuid` | `1` (features: v4) | 请求 ID 生成 | api, tools |
| `chrono` | `0.4` (features: serde) | 时间戳处理 | 全局 |
| `jsonschema` | `0.18` | JSON Schema 运行时校验 | tools |
| `async-trait` | `0.1` | 异步 trait 支持 | tools |
| `tokio-stream` | `0.1` | Stream 工具（SSE 用） | api |

### 12.2 间接依赖（通过其他模块引入）

| Crate | 用途 | 被谁使用 |
|---|---|---|
| `reqwest` | HTTP 客户端 | infra/llm_client, infra/embedding |
| `rusqlite` | SQLite 操作 | infra/sqlite_store |
| `tantivy` | 全文索引 | infra/tantivy_index |
| `qdrant-client` | Qdrant 向量库客户端 | infra/qdrant_client |
| `git2` | Git 操作 | core/code_repo |
| `pulldown-cmark` | Markdown 解析 | core/memory |
| `gray_matter` | YAML frontmatter 解析 | core/memory |
| `notify` | 文件系统监控 | infra/file_watcher |

### 12.3 `Cargo.toml` 依赖片段

```toml
[dependencies]
# Web 框架
axum = { version = "0.8", features = ["macros"] }
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "trace"] }
tokio = { version = "1", features = ["full"] }
tokio-stream = "0.1"

# 序列化
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# 日志
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

# 工具
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
jsonschema = "0.18"
async-trait = "0.1"

[dev-dependencies]
tower = { version = "0.5", features = ["util"] }
http-body-util = "0.1"
```

---

## 13. 实施计划

| 阶段 | 任务 | 预估工时 | 依赖 |
|---|---|---|---|
| **T1** | `tools/traits.rs` + `tools/registry.rs` 实现 | 1 天 | 无 |
| **T2** | `tools/definitions.rs` 全部工具 Schema 定义 | 1 天 | T1 |
| **T3** | `api/router.rs` + `api/middleware.rs` Axum 路由和中间件 | 0.5 天 | T1 |
| **T4** | `api/handlers/tool_handler.rs` + `health_handler.rs` HTTP 处理器 | 1 天 | T3 |
| **T5** | `api/tool_protocol.rs` MCP 协议核心实现 | 1.5 天 | T1 |
| **T6** | `api/mcp_stdio.rs` stdio 传输实现 | 0.5 天 | T5 |
| **T7** | `api/mod.rs` 服务启动流程 + 工具自动注册 | 0.5 天 | T1-T6 |
| **T8** | 各工具 Handler 实现（与核心服务层联调） | 3-5 天 | T4, 核心服务层 |
| **T9** | 单元测试 + 协议一致性测试 | 1 天 | T5, T6 |
| **T10** | HTTP 端到端测试 + Claude Desktop 集成测试 | 1 天 | T7, T8 |
| **T11** | `api/mcp_sse.rs` SSE 传输（可选） | 1 天 | T5 |

**总预估**: 12-15 天（不含 SSE 可选模块）

---

> **上游**: [需求设计文档](../requirement/02-tool-protocol.md) 定义了本层需要满足的功能需求和验收标准。  
> **下游**: 核心服务层（`src/core/`）的各个 Service 是工具 Handler 的实际执行者，Handler 通过 `AppContext` 调用它们。
