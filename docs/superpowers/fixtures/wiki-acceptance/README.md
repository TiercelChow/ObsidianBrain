# Wiki 隔离验收材料

这里的“北辰 Cache”是虚构系统，版本、基准和参数均为专门构造的测试事实，不是实际产品建议。所有问题只根据本目录的原文判断，不查询互联网。

`sources/` 中八篇 Markdown 构成一本测试书；`cases.json` 在模型调用前冻结期望来源、事实和禁止推断。不要把这些文件添加到正式书架。

## 使用

在仓库根目录用现有构建启动验收实例。脚本使用固定的本机端口 `19988`，需要先确认它空闲。

```bash
WIKI_ACCEPTANCE_DIR=$(mktemp -d "${TMPDIR:-/tmp}/obsidianbrain-wiki-acceptance.XXXXXX")
echo "$WIKI_ACCEPTANCE_DIR"
OBRAIN_DATA_DIR="$WIKI_ACCEPTANCE_DIR" backend/target/debug/obsidian-brain start --foreground --host 127.0.0.1 --port 19988
```

在另一个终端把下文的 `$WIKI_ACCEPTANCE_DIR` 设为打印出的路径。`prepare` 核对服务数据目录和书架，复制八份材料并同步原文。配置供应商后，使用同一个实例先测只有原文的问答，再编译并发布，测带主题页的问答。两组使用相同的问答入口、模型和题目。脚本每条 `qa` 命令运行一个案例并新建会话，Q3 在该会话中连续三轮。

```bash
node backend/scripts/wiki-acceptance.mjs prepare --workspace "$WIKI_ACCEPTANCE_DIR"
node backend/scripts/wiki-acceptance.mjs status --workspace "$WIKI_ACCEPTANCE_DIR"
node backend/scripts/wiki-acceptance.mjs qa --workspace "$WIKI_ACCEPTANCE_DIR" --variant raw --case Q1
node backend/scripts/wiki-acceptance.mjs compile --workspace "$WIKI_ACCEPTANCE_DIR"
```

`compile` 保存编译进度、首份草稿观察时间、主题草稿和逐份原文覆盖。审核主题后在 Wiki 工作台发布，再运行同一道题的主题组。

```bash
node backend/scripts/wiki-acceptance.mjs qa --workspace "$WIKI_ACCEPTANCE_DIR" --variant topics --case Q1
```

Q2–Q4 同样执行。保留两组原始答案、流事件时间和实际引用，再按 `cases.json` 评估答全率、适用条件和引用支持。研究与 PPT 按同一文件的简报在隔离界面执行并人工审阅。

已有研究任务可用 `watch-task --workspace <隔离目录> --task-id <明确选定的任务 ID>` 观察并保存阶段、正文及成果元数据。它先验证任务属于本次知识库，只执行读取；到达完成、失败或取消状态即停止，不创建任务、不触发执行、不自动恢复，也不把结构校验标记当作语义/视觉通过。PPT 文件还需要从成果下载接口获取并逐页渲染检查。

结果位于临时目录的 `results/`，不进入正式数据目录。不得把含密钥或个人内容的记录提交。

## 可选 Harness 参数回归（不调用模型）

后端的 `test_harness_wire_disables_thinking_and_preserves_qa_auto` 在普通回归中忽略。若本机已安装 Harness，可将 `HARNESS_PI_AI_MODULE` 指向其 `@deepseek-ai/dsh-llm-pi-ai/lib/index.js`，在 `backend/` 运行 `cargo test test_harness_wire -- --ignored`。

测试把生产代码生成的供应商 Patch 交给已安装 Harness 的公开适配器，只把端点替换为临时本机 HTTP 服务并提供虚拟凭据。校验编译/显式 off 实际发送关闭思考、系统指令仍为 `system`、输出上限正确，以及问答 auto 保留供应商默认。它不读取真实 Key、不访问供应商、不写入用户配置；版本升级后可复用以发现适配器序列化行为变化。
