export const mermaidSamples = [
  { id: 'overview', title: '知识工作流', source: `flowchart LR
    A[阅读书籍] --> B[整理知识]
    B --> C{下一步}
    C -->|深入理解| D[知识问答]
    C -->|形成成果| E[研究报告]
    D --> B
    E --> F[演示文稿]` },
  { id: 'chinese', title: '中文长标签', source: `flowchart TB
    A["这是一段较长的中文节点说明，需要完整显示而不是被节点右侧的边界截断"] --> B["输入文档包含中文、English identifiers 以及多种标点（示例）"]
    B -->|"关系说明也需要保持完整可读"| C["完成知识整理与结果校验"]` },
  { id: 'markdown', title: '自动换行与强调', source: 'flowchart LR\n A["`**知识编译**\n将阅读内容整理为可以检索与引用的知识条目，保留完整的来源。`"] --> B["`**证据问答**\n根据原始资料与编译结果回答问题。`"]' },
  { id: 'multiline', title: '多行与长英文', source: `flowchart TB
    A["第一行：中文说明<br/>Second line: configuration<br/>第三行：完成"] --> B["very_long_configuration_identifier_without_spaces_0123456789"]` },
  { id: 'subgraphs', title: '嵌套中文子图', source: `flowchart LR
    subgraph 知识管理
      subgraph 来源管理
        A[本地书籍] --> B[来源片段]
      end
      subgraph 编译结果
        C[概念条目] --> D[关系与引用]
      end
      B --> C
    end
    D --> E[知识问答]` },
  { id: 'dense', title: '复杂连线', source: `flowchart LR
    A[用户问题] --> B[理解意图]
    B --> C[候选知识]
    B --> D[上下文记忆]
    C --> E[阅读条目]
    C --> F[比较论断]
    E --> G{证据是否充分}
    F --> G
    D --> G
    G -->|否| H[读取原文]
    H --> E
    G -->|是| I[组织答案]
    I --> J[引用与结论]
    E --> J` },
  { id: 'math', title: '节点和边中的公式', source: String.raw`flowchart LR
    X["输入 $$x_t$$"] -->|"$$W x_t + b$$"| H["隐藏状态 $$h_t$$"]
    H --> Y["输出 $$\hat{y}_t$$"]` },
  { id: 'matrix', title: '分数和矩阵', source: String.raw`flowchart TB
    A["归一化 $$\frac{x_i}{\sqrt{\sum_j x_j^2}}$$"] --> B["矩阵 $$\begin{pmatrix}a & b \\\\ c & d\end{pmatrix}$$"]` },
  { id: 'sequence', title: '时序图', source: `sequenceDiagram
    participant U as 用户
    participant R as 阅境轩
    participant K as 书籍知识库
    U->>R: 打开最近阅读的书籍
    R->>K: 获取已经整理的知识条目
    K-->>R: 返回条目和原文引用
    Note over R,K: 资料保存在本地，阅读进度自动记录
    R-->>U: 展示完整内容` },
  { id: 'sequence-math', title: '时序图中的公式', source: String.raw`sequenceDiagram
    participant A as 输入 $$x_t$$
    participant B as 状态 $$h_t$$
    A->>B: 更新 $$h_t = \sigma(Wx_t)$$
    Note over A,B: 约束 $$\lVert h_t\rVert_2 \leq 1$$` },
  { id: 'state', title: '状态图', source: `stateDiagram-v2
    [*] --> 等待开始
    等待开始 --> 正在处理: 开始
    正在处理 --> 等待审核: 提交结果
    等待审核 --> 正在处理: 补充证据
    等待审核 --> 已完成: 通过
    已完成 --> [*]` },
  { id: 'class', title: '类图', source: `classDiagram
    class Book {
      +String title
      +read()
    }
    class KnowledgeBase {
      +compile()
      +query()
    }
    Book "1" --> "1" KnowledgeBase : 关联` },
  { id: 'er', title: '实体关系图', source: `erDiagram
    BOOK ||--o{ CHAPTER : contains
    CHAPTER ||--o{ CITATION : references
    BOOK { string title }
    CHAPTER { string content }
    CITATION { int line }` },
  { id: 'gantt', title: '甘特图', source: `gantt
    title 阅读与研究计划
    dateFormat YYYY-MM-DD
    section 阅读
    阅读资料 :a, 2026-10-01, 5d
    整理知识 :b, after a, 3d
    section 研究
    形成报告 :c, after b, 4d` },
  { id: 'pie', title: '饼图', source: `pie title 知识类型
    "概念" : 45
    "方法" : 35
    "案例" : 20` },
  { id: 'mindmap', title: '思维导图', source: `mindmap
    root((知识工作台))
      阅读
        书架
        阅读进度
      整理
        概念
        来源
      应用
        问答
        研究` },
  { id: 'timeline', title: '时间线', source: `timeline
    title 知识积累
    第一周 : 阅读原始资料
    第二周 : 整理知识库 : 校验证据
    第三周 : 完成研究报告` },
  { id: 'styles', title: '保留作者样式', source: `flowchart LR
    A[需要关注]:::important --> B[正常流程]
    classDef important fill:#fff0d6,stroke:#aa6515,color:#533408,stroke-width:2px
    linkStyle 0 stroke:#aa6515,stroke-width:2px` },
  { id: 'dagre', title: '作者指定布局', source: `---
config:
  layout: dagre
---
flowchart LR
    A[保留作者的布局选择] --> B[不改写文档源码]` },
  { id: 'invalid', title: '错误恢复', invalid: true, source: 'flowchart LR\n A[未闭合的节点 --> B[结果]' },
]
