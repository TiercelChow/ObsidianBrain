# Book Wiki 开源 Skill 来源审查（2026-09-16）

## 目的

本审查用于建立 Skill 质量基线，不代表把第三方 Skill 直接启用到生产运行。任何候选都必须先经过许可证确认、内容审查、当前工具边界适配和固定评测，最后以带来源信息的新版本进入 SQLite Skill Registry。

## 筛选规则

1. 优先官方或维护活跃的上游、明确许可证、可查看完整 `SKILL.md` 与资源。
2. Skills CLI 安装量只作为流行度信号，不等同于质量或安全背书。
3. 当前 Runtime 只接受 UTF-8 指令与参考文本；脚本、Shell、任意文件和未授权网络步骤必须移除或改写。
4. 第三方指令不能覆盖服务端引用校验、工具白名单、变更集审核和数据库事务边界。
5. 来源、上游路径、许可证、审查日期和适配差异必须随 Skill 版本保存。

## 候选结论

| 场景 | 候选 | 观察信号 | 许可证 | 结论 |
|---|---|---:|---|---|
| LLM Wiki | [NousResearch/hermes-agent · llm-wiki](https://github.com/NousResearch/hermes-agent/blob/main/website/docs/user-guide/skills/bundled/research/research-llm-wiki.md) | 完整工作流；仓库维护活跃 | MIT | 进入适配候选；重点吸收会话定向、分流、交叉引用、冲突保留和 lint 思路，不复制文件存储假设 |
| LLM Wiki | [Astro-Han/karpathy-llm-wiki](https://github.com/Astro-Han/karpathy-llm-wiki) | Skills CLI 约 7.3K 安装 | 未确认 | 仅作评测对照；许可证明确前不得复制正文或资源 |
| LLM Wiki | [nashsu/llm_wiki_skill](https://github.com/nashsu/llm_wiki_skill) | Skills CLI 约 1.6K 安装；141 GitHub stars | 仓库未明确 | 仅作产品/API 对照；许可证明确前不得导入 |
| 深度研究 | [Weizhena/deep-research-skills](https://github.com/Weizhena/deep-research-skills) | Skills CLI 约 804 安装；约 2.1K GitHub stars | MIT | 进入适配候选；保留计划—执行—综合与人工确认，移除外部脚本/子 Agent 强依赖 |
| Skill 设计与评测 | [anthropics/skills · skill-creator](https://github.com/anthropics/skills/blob/main/skills/skill-creator/SKILL.md) | 官方示例仓库；约 176K GitHub stars | 仓库内按目录区分 | 作为结构和评测方法参考；逐文件确认许可证，不整体复制 |
| 演示文稿 | [anthropics/skills · pptx](https://github.com/anthropics/skills/tree/main/skills/pptx) | 生产级示例 | source-available，非开源 | 只研究质量检查思路，不复制内容；继续使用本项目 Rust PPTX 生成器 |

安装量是 2026-09-16 通过 `npx skills find` 得到的快照，会随时间变化。

## 首批适配目标

### `book-ingest` v2

- 增加 New / Update / Disputed / No material 四类分流。
- 先检查既有实体与别名，再决定新增或更新。
- 数字、日期、长引用和关键结论必须定位到来源片段。
- 对冲突论断分别保存证据、适用条件和置信度，不自动消解。
- 没有实质新增时允许返回空候选，避免制造低价值实体。

### `book-research` v2

- 先生成有界研究计划，再逐步检索书内证据。
- 每一节结论都关联来源，明确区分书内证据与授权外部资料。
- 在综合前列出证据缺口、冲突和不可回答项。
- 不直接写正式知识；有保存价值的结论仍进入审核变更集。

### `book-presentation` v2

- 只负责报告到演示文稿的信息架构、叙事和密度约束。
- 二进制 PPTX 继续由受控 Rust 生成器输出并校验。
- 不引入第三方脚本、外部渲染服务或未授权素材抓取。

## 当前已实现的发布门槛

- 首版使用确定性的离线契约评测，分别检查摄入分流与引用、问答溯源与不确定性、研究计划与来源边界、演示文稿叙事与证据约束。
- 评测结果保存候选分数、当前版本基线分数和逐用例缺失项；固定用途的内置 Skill 不能借用其他评测集通过门槛。
- 候选版本只能进入 `candidate`；离线评测通过后仍需用户点击发布，发布后保留回滚到任一历史 `published` revision 的能力。
- 此评测只证明 Skill 文本包含必要结构与安全规则，不等价于真实模型回答质量，也不会把候选自动设为默认版本。

## 后续真实模型基准扩展

- 固定不少于 12 个摄入样例、8 个问答样例和 6 个研究样例。
- 至少比较当前版本与候选版本的引用正确率、重复实体率、冲突保留率、无实质变化误写率和 Token/耗时。
- 真实模型基准用于扩大质量置信度，不取代人工发布，也不得依赖线上供应商才能完成数据库迁移或普通单元测试。
- 任何许可证不清、要求执行脚本或扩大工具权限的候选保持 `hold`。
