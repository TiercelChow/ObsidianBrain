// Isolated responsive preview. Reading patches update synthetic memory only;
// no user database, source file, model or production access.
// From frontend: node tests/fixtures/mobileNavigation.mjs
import { fileURLToPath } from 'node:url'
import { createServer } from 'vite'

const timestamp = '2026-10-04T08:00:00Z'
const storage = { data_directory: '/mock', originals_directory: '/mock/timeline/images', cache_directory: '/mock/timeline/cache', originals_count: 3, originals_bytes: 3145728, cache_bytes: 0, cache_limit_bytes: 268435456, pending_cleanup: 0, missing_images: [], last_import: null }
const cards = Array.from({ length: 8 }, (_, index) => ({
  book: { id: `book-${index}`, name: ['阅读与思考', '设计的细节', '知识的连接', '持续成长'][index % 4], path: `/mock/book-${index}`, kind: 'folder', addedAt: 1 },
  knowledge_base: { id: `base-${index}`, book_id: `book-${index}`, book_name: ['阅读与思考', '设计的细节', '知识的连接', '持续成长'][index % 4], book_path: `/mock/book-${index}`, book_kind: 'folder', lifecycle: 'active', source_available: true, entry_count: 12, source_count: 8, claim_count: 32, task_count: 0, sync_state: 'clean', compile_mode: 'smart', compile_state: 'ready', compile_phase: 'completed', health_state: 'healthy' },
}))
const day = new Date().toLocaleDateString('en-CA')
const personalTasks = ['整理阅读笔记与研究提纲', '更新项目计划', '完成本周知识回顾'].map((title,index)=>({
  id:`task-${index}`,root_id:`task-${index}`,parent_id:null,kind:index===1?'short':'long',role:'root',title,description:'隔离验证记录，不是真实数据',status:index===1?'open':'in_progress',importance:index===0?'high':'normal',start_date:day,end_date:day,position:0,created_at:timestamp,updated_at:timestamp,revision:1,archived_at:null,progress_percent:[45,0,20][index],completed_leaf_count:1,effective_leaf_count:3,storage_path:`mock-${index}`,document_version:{revision:1,content_hash:'mock'},derived:{overdue:false,active_today:true,due_today:true},
}))
const child = {...personalTasks[0],id:'child-0',role:'subtask',root_id:'task-0',parent_id:'task-0',title:'核对资料中的关键数据',status:'blocked'}
const grandchild = {...child,id:'child-1',parent_id:'child-0',title:'对照来源与引用',status:'open'}
const previewEntries = ['构建知识之间的连接','从阅读到实践'].map((title,index)=>({id:`entry-${index}`,knowledge_base_id:'base-0',entry_type:'concept',slug:`preview-${index}`,title,summary:'以问题为起点，连接阅读、记录与行动。',status:'verified',source_path:'/mock/chapter.md',updated_at:timestamp}))
const previewConversation = {id:'conversation-0',knowledge_base_id:'base-0',title:'如何让阅读转化为行动',message_count:2,preview:'通过问题与实践连接知识',created_at:timestamp,updated_at:timestamp}
const previewProvider = {provider_id:'isolated-preview',display_name:'隔离预览供应商',api_protocol:'openai-completions',base_url:'https://example.invalid/v1',model:'preview-model',context_window:1000000,max_output_tokens:32000,reasoning_policy:'auto',credential_source:'environment',api_key_env:'PREVIEW_ONLY',api_key_configured:false,enabled:false,revision:1,updated_at:timestamp}
const researchTask={id:'research-0',knowledge_base_id:'base-0',book_name:'阅读与思考',title:'如何建立可持续的个人知识体系',description:'只读隔离预览',task_type:'research',status:'running',deliverable_type:'report',artifact_state:'not_requested',knowledge_change_state:'none',result_summary:'',created_at:timestamp,updated_at:timestamp}
const section=data=>({data,error:null})
const homeOverview={today:day,week_start:day,generated_at:new Date().toISOString(),system:{version:'隔离预览',uptime_seconds:9600,components:{server:'ok',sqlite:'ok'}},
  tasks:section({active_count:7,today_count:3,overdue_count:1,items:personalTasks.map((task,index)=>({...task,child_risk_id:index===0?'child-0':null,child_risk_title:index===0?child.title:null}))}),
  reading:section(cards.slice(0,3).map((card,index)=>({id:card.book.id,name:card.book.name,kind:'folder',last_file:'/mock/chapter.md',position:.7,page_count:null,read_at:Date.now()-index*86400000}))),
  memos:section({total_count:12,week_count:5,items:[{id:'memo-0',date:day,timestamp,excerpt:'把阅读中的问题记录下来，再带着问题回到原文。知识的连接，比收藏本身更重要。',tags:['阅读','思考'],thumbnail:null},{id:'memo-1',date:day,timestamp,excerpt:'今天推进了项目计划，留一点时间整理这一周的收获。',tags:['日常'],thumbnail:null}]}),
  wiki:section({running_count:2,review_count:1,failed_count:0,items:[{id:'base-0',base_id:'base-0',book_name:'阅读与思考',kind:'compile',title:'知识编译',status:'compiling',detail:'正在归并语义主题 · 第 2 / 4 批',artifact_state:null,updated_at:timestamp},{id:'research-0',base_id:'base-0',book_name:'阅读与思考',kind:'research',title:researchTask.title,status:'running',detail:'',artifact_state:null,updated_at:timestamp}]}),
  storage:section({originals_bytes:184*1024*1024,cache_bytes:24*1024*1024,cache_limit_bytes:268435456,pending_cleanup:0}),
}
cards[0].knowledge_base.compile_state='compiling';cards[0].knowledge_base.compile_phase='thinking';cards[0].knowledge_base.compile_message='正在归并语义主题';cards[0].knowledge_base.pending_review_count=1
cards.forEach(card=>{card.book.progress={lastFile:'/mock/chapter.md',position:.7,updatedAt:Date.now()}})
const resultFor = {
  get_timeline_storage: storage,
  get_memo_stats: { total_memos: 12 },
  list_code_repos: { repos: [{ name: '桌面仓库', path: '/mock/repo', current_branch: 'main', is_dirty: false, languages: { Rust: 1 } }] },
  get_reader_books: { books: cards.map(card=>card.book) },
  get_reader_history: { history: [] },
  list_tasks: { tasks: Array.from({length:18}, (_, index) => ({...personalTasks[index % 3], id:`task-${index}`, title:index < 3 ? personalTasks[index].title : `规划与复盘 ${index + 1}`})), next_cursor: null },
  get_task: { root:personalTasks[0],tasks:[personalTasks[0],child,grandchild],progress:[],audit:[],storage_path:'mock',document_version:{revision:1,content_hash:'mock'},progress_percent:45,completed_leaf_count:1,effective_leaf_count:3 },
  list_local_dir:{root:'/mock',total_files:1,entries:[{name:'chapter.md',path:'/mock/chapter.md',is_dir:false,size:40}]},
  read_local_file:{path:'/mock/chapter.md',name:'chapter.md',content:'# 阅读与思考\n\n这是隔离的阅读预览。',size:40},
  get_knowledge_compile_report:{report:null},
  get_knowledge_research_workspace:{task_id:'research-0',knowledge_base_id:'base-0',original_request:{},plan:null,stages:[],baselines:[],created_at:timestamp,updated_at:timestamp},
  get_task_calendar: [...personalTasks, child],
  browse_timeline: { memos: Array.from({ length: 12 }, (_, index) => ({ id: `memo-${index}`, content: `## 日常记录 ${index + 1}\n\n阅读、记录、执行，再把知识连接成自己的思考。`, timestamp:new Date(Date.now() - index * 86400000).toISOString(), tags: ['隔离预览'], images: index === 0 ? ['portrait.svg', 'landscape.svg', 'broken.svg'] : [], revision: 1 })), total: 12, has_more: false },
  search_memos: { memos: [], total: 0, has_more: false },
  list_book_knowledge_bases: { items: cards },
  list_knowledge_entries: { entries: previewEntries, total: 2, has_more: false },
  get_knowledge_entry: {...previewEntries[0],content_md:'# 构建知识之间的连接\n\n把问题、阅读与行动连接起来。\n\n- 提出问题\n- 对照来源\n- 记录实践结果',aliases:[],edit_policy:'review',revision:1,citations:[],claims:[],relations:[],versions:[]},
  list_knowledge_change_sets: { change_sets: [{id:'review-0',knowledge_base_id:'base-0',title:'整理知识脉络',reason:'隔离预览 · 审核候选',risk_level:'low',status:'proposed',created_at:timestamp,classification_summary:{new:1,update:0,disputed:0,no_material:0},citation_audit:{passed:true,entry_citations:1,claim_citations:0,issues:[]},impact_summary:{entries:1,claims:0,relations:0,citations:1},changes:[]}] },
  list_knowledge_tasks: { tasks: [researchTask] },
  list_knowledge_conversations: { conversations: [previewConversation] },
  get_knowledge_conversation: {...previewConversation,messages:[{id:'question-0',role:'user',content:'如何让阅读转化为行动？',run_id:null,evidence:[],created_at:timestamp},{id:'answer-0',role:'assistant',content:'## 从一个问题开始\n\n将阅读中的观点变成可执行的行动，再回到来源核对结果。\n\n1. 提出具体问题。\n2. 将观点与实践连接。\n3. 保留证据与反思。',run_id:'run-preview',evidence:previewEntries,created_at:timestamp}]},
  list_unfinished_qa_runs: { runs: [] },
  list_wiki_skills: { skills: [{id:'preview-skill',slug:'knowledge-query',name:'知识问答',description:'根据本书的知识与来源回答问题。',source_type:'custom',status:'ready',permissions:[],requirements:[],revision:1,instructions:'Preview only',enabled:false,usage_scope:'both',updated_at:timestamp}] },
  list_knowledge_backups: { backups: [], retention: 7 },
  get_book_wiki_settings: { runtime_profiles: [], model_providers: [previewProvider], documents: [{id:'doc-0',name:'问答规则',scope:'book',revision:1,content_md:'# 问答规则\n根据来源提供回答。'},{id:'doc-1',name:'研究规则',scope:'book',revision:1,content_md:'# 研究规则\n规划、取证、检查与交付。'}] },
  get_knowledge_task_result: {task:{...researchTask,result_summary:'# 研究报告\n\n隔离预览：通过记录与实践构建持续更新的知识体系。'},evidence:previewEntries,artifacts:[]},
  get_agent_usage_stats: { usage_source: 'unavailable', totals: { runs: 0, unreported_runs: 0, total_tokens: 0, input_tokens: 0, output_tokens: 0 }, daily: [], by_caller: [] },
  get_book_knowledge_base: cards[0].knowledge_base,
}
const readOnlyPreview = {
  name: 'isolated-mobile-navigation',
  configureServer(server) {
    server.middlewares.use(async (request, response, next) => {
      if (!request.url?.startsWith('/v1')) return next()
      if (/^\/v1\/timeline\/(?:images|thumbnails)\//.test(request.url)) {
        const portrait=request.url.endsWith('portrait.svg')
        if (request.url.endsWith('broken.svg')) {response.statusCode=404;response.end();return}
        if(request.url.includes('/images/')) await new Promise(resolve=>setTimeout(resolve, 450))
        response.setHeader('Content-Type','image/svg+xml')
        const width=portrait?600:1200, height=portrait?1200:600
        response.end(`<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}"><defs><linearGradient id="sky" x2="1" y2="1"><stop stop-color="#163b42"/><stop offset="1" stop-color="#7cad99"/></linearGradient></defs><rect width="100%" height="100%" fill="url(#sky)"/><circle cx="${width*.7}" cy="${height*.28}" r="${width*.15}" fill="#dcebd8" opacity=".7"/><path d="M0 ${height*.75} Q${width*.4} ${height*.35} ${width} ${height*.8} V${height} H0Z" fill="#153a32"/><text x="${width*.08}" y="${height*.15}" font-family="sans-serif" font-size="32" fill="white">${portrait?'Portrait':'Landscape'} · Preview</text></svg>`);return
      }
      response.setHeader('Content-Type', 'application/json')
      if (request.url.startsWith('/v1/home/overview')) {
        const mode=new URL(request.headers.referer || 'http://localhost').searchParams.get('preview')
        const data=structuredClone(homeOverview)
        if(mode==='reading' || cards.some(card=>card.book.progress?.lastReadAt)) {
          data.reading=section(cards.filter(card=>card.book.progress?.lastReadAt).map(card=>({id:card.book.id,name:card.book.name,kind:card.book.kind,last_file:card.book.progress.lastFile,position:card.book.progress.position,page_count:card.book.progress.pageCount||null,read_at:card.book.progress.lastReadAt})).sort((a,b)=>b.read_at-a.read_at).slice(0,4))
        }
        if(mode==='empty') {data.tasks=section({active_count:0,today_count:0,overdue_count:0,items:[]});data.reading=section([]);data.memos=section({total_count:0,week_count:0,items:[]});data.wiki=section({running_count:0,review_count:0,failed_count:0,items:[]})}
        if(mode==='partial') data.memos={data:null,error:'小记暂时无法读取，请重试'}
        if(mode==='right-tall') {
          data.tasks.data.items=data.tasks.data.items.slice(0,1);data.reading.data=data.reading.data.slice(0,1)
          data.wiki.data.items=Array.from({length:5},(_,index)=>({...homeOverview.wiki.data.items[index%2],id:`preview-wiki-${index}`,title:'整理阅读资料并检查知识之间的关联与引用',detail:'正在整理跨章节的语义主题，核对来源与引用，并生成等待确认的知识候选'}))
          data.memos.data.items=Array.from({length:3},(_,index)=>({...homeOverview.memos.data.items[0],id:`preview-memo-${index}`}))
        }
        if(mode==='left-tall') {
          data.tasks.data.items=Array.from({length:4},(_,index)=>({...personalTasks[index%3],id:`preview-task-${index}`}));data.reading.data=Array.from({length:4},(_,index)=>({...homeOverview.reading.data[index%3],id:`preview-book-${index}`}))
          data.wiki.data.items=data.wiki.data.items.slice(0,1);data.memos.data.items=data.memos.data.items.slice(0,1)
        }
        response.end(JSON.stringify(data));return
      }
      if(request.url.startsWith('/v1/timeline/memos/')) {
        const id=decodeURIComponent(request.url.split('/').pop());response.end(JSON.stringify({...resultFor.browse_timeline.memos[0],id,date:day}));return
      }
      if (request.url === '/v1/health') {
        response.end(JSON.stringify({ status: 'healthy', version: 'preview', components: { timeline: 'ok', wiki: 'ok' }, uptime_seconds: 3600, tools_count: 48, storage: { path: '/mock', exists: true } }))
        return
      }
      try {
        let body = ''
        for await (const part of request) body += part
        const call = JSON.parse(body)
        if(request.url==='/v1/tools/call' && call.tool==='save_reader_progress') {
          const {book_id,state}=call.arguments
          const card=cards.find(card=>card.book.id===book_id)
          if(!card) throw new Error('Synthetic book missing')
          const old=card.book.progress
          const byFile={...old.byFile,...state.byFile}
          const file=byFile[state.lastFile]
          card.book.progress={lastFile:state.lastFile,lastReadAt:state.lastReadAt,updatedAt:state.lastReadAt,position:file.position,pageCount:file.pageCount,byFile}
          response.end(JSON.stringify({tool:call.tool,status:'success',result:{state:{lastFile:state.lastFile,lastReadAt:state.lastReadAt,byFile}}}));return
        }
        if (request.url !== '/v1/tools/call' || !Object.hasOwn(resultFor, call.tool)) throw new Error('Read-only preview')
        console.log(`Preview read: ${call.tool}`)
        let result=resultFor[call.tool]
        if(call.tool==='list_tasks') {
          const filters=call.arguments
          result={tasks:result.tasks.filter(task=>(!filters.query || task.title.includes(filters.query)) && (!filters.kinds || filters.kinds.includes(task.kind)) && (!filters.statuses || filters.statuses.includes(task.status))),next_cursor:null}
        }
        if(call.tool==='list_local_dir') {
          const root=call.arguments.path
          result={root,total:2,entries:[{name:'chapter.md',path:`${root}/chapter.md`,is_dir:false,is_markdown:true,is_pdf:false},{name:'next.md',path:`${root}/next.md`,is_dir:false,is_markdown:true,is_pdf:false}]}
        }
        if(call.tool==='read_local_file') {
          const path=call.arguments.path
          result={path,name:path.split('/').pop(),content:`# ${path.endsWith('next.md')?'下一章':'阅读与思考'}\n\n`+Array.from({length:70},(_,index)=>`## ${index+1} · 阅读片段\n\n这是只在内存中保存的隔离测试记录，用于验证打开、滚动、章节切换和首页最近阅读。`).join('\n\n'),size:5000}
        }
        response.end(JSON.stringify({ tool: call.tool, status: 'success', result }))
      } catch {
        response.statusCode = 400
        response.end(JSON.stringify({ status: 'error', error: { message: '此预览只支持导航验收所需的只读请求' } }))
      }
    })
  },
}
const vite = await createServer({
  root: fileURLToPath(new URL('../../', import.meta.url)),
  configFile: fileURLToPath(new URL('../../vite.config.ts', import.meta.url)),
  plugins: [readOnlyPreview],
  server: { host: '127.0.0.1', port: 5220, strictPort: true },
})
await vite.listen()
console.log('Isolated mobile navigation preview: http://127.0.0.1:5220/')
let stopping = false
async function stop() {
  if (stopping) return
  stopping = true
  await vite.close()
  process.exit(0)
}
process.on('SIGINT', stop)
process.on('SIGTERM', stop)
