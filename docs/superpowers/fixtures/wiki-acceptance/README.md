# Wiki 隔离验收材料

这里的“北辰 Cache”是虚构系统，版本、基准和参数均为专门构造的测试事实，不是实际产品建议。所有问题只根据本目录的原文判断，不查询互联网。

`sources/` 中八篇 Markdown 构成一本测试书；`cases.json` 在模型调用前冻结期望来源、事实和禁止推断。不要把这些文件添加到正式书架。

## 使用

先按 [实施指导](../../../development/2026-10-01-wiki-real-model-acceptance-guide.md) 启动隔离实例。然后运行：

```bash
node backend/scripts/wiki-acceptance.mjs prepare --workspace /tmp/obsidianbrain-wiki-acceptance.XXXXXX
node backend/scripts/wiki-acceptance.mjs status --workspace /tmp/obsidianbrain-wiki-acceptance.XXXXXX
```

替换为实际临时目录。脚本只接受本机 `19988` 的实例，核对其 Vault 路径、禁用的 Obsidian 连接和受控书架后才写入。复制材料后同步来源，不发起编译、不读取密钥、不调用模型、不自动批准知识。

在该隔离实例的配置页配置供应商，执行智能编译；检查 W1 核对表和变更集后手动批准。之后可运行单个真实问答：

```bash
node backend/scripts/wiki-acceptance.mjs qa --workspace /tmp/obsidianbrain-wiki-acceptance.XXXXXX --case Q1
node backend/scripts/wiki-acceptance.mjs qa --workspace /tmp/obsidianbrain-wiki-acceptance.XXXXXX --case Q3
```

也可用 `compile --workspace <隔离目录>` 启动一次编译并记录各阶段、最终状态和候选。进入 `waiting_review` 后脚本停止观察，不继续轮询、不自动批准。编译失败也保存证据并返回失败状态，不自动再次触发。

每次仅执行一个案例，无自动重跑。Q3 是同一会话的三轮，其余是单轮。脚本保存结果和流事件的接收时间，不根据关键词自动宣布语义验收通过。研究与 PPT 通过隔离界面按 `cases.json` 执行并人工审阅。

已有研究任务可用 `watch-task --workspace <隔离目录> --task-id <明确选定的任务 ID>` 观察并保存阶段、正文及成果元数据。它先验证任务属于本次知识库，只执行读取；到达完成、失败或取消状态即停止，不创建任务、不触发执行、不自动恢复，也不把结构校验标记当作语义/视觉通过。PPT 文件还需要从成果下载接口获取并逐页渲染检查。

结果位于临时目录的 `results/`，不进入正式数据目录。不得把含密钥或个人内容的记录提交。

## 可选 Harness 参数回归（不调用模型）

后端的 `test_bailian_harness_wire_disables_thinking_and_preserves_qa_auto` 在普通回归中忽略。若本机已安装 Harness，可将 `HARNESS_PI_AI_MODULE` 指向其 `@deepseek-ai/dsh-llm-pi-ai/lib/index.js`，在 `backend/` 运行 `cargo test test_bailian_harness_wire -- --ignored`。

测试把生产代码生成的供应商 Patch 交给已安装 Harness 的公开适配器，只把端点替换为临时本机 HTTP 服务并提供虚拟凭据。校验编译/显式 off 实际发送关闭思考、系统指令仍为 `system`、输出上限正确，以及问答 auto 保留供应商默认。它不读取真实 Key、不访问供应商、不写入用户配置；版本升级后可复用以发现适配器序列化行为变化。
