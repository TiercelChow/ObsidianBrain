// Fully isolated Reader/Worker/PDF verification; never forwards an API request.
import assert from 'node:assert/strict'
import { mkdtemp } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createServer } from 'vite'
const {chromium}=await import(process.env.PLAYWRIGHT_MODULE || 'playwright')
const directory=await mkdtemp(join(tmpdir(),'reader-bookmarks-'))
const books=['a','b'].map(id=>({id,path:`/mock/${id}`,kind:'folder',name:`书签验证 ${id}`,description:'',category:'',addedAt:1}))
const chapter=name=>`# ${name}\n\n[第二章](./two.md) · [PDF](./paper.pdf)\n\n`+Array.from({length:34},(_,i)=>`## ${name} 第 ${i+1} 节\n\n${name}段落 ${i+1}：这是用于验证精确定位的独立内容，换屏、刷新和跨章节跳转后仍应返回这里。`.repeat(2)).join('\n\n')
const note='## 我的笔记\n\n**关键结论** 与 $x_i^2$。\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n\n```typescript\nconst count = 128;\n```'
function pdfFixture(){
  const objects=['<< /Type /Catalog /Pages 2 0 R >>','<< /Type /Pages /Kids [3 0 R 5 0 R 7 0 R] /Count 3 >>']
  for(let page=1;page<=3;page++){
    const content=`BT /F1 24 Tf 60 820 Td (Bookmark page ${page}) Tj 0 -200 Td (Top section) Tj 0 -250 Td (Middle section) Tj 0 -250 Td (Bottom section) Tj ET`
    objects.push(`<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 900] /Resources << /Font << /F1 9 0 R >> >> /Contents ${page*2+2} 0 R >>`,`<< /Length ${Buffer.byteLength(content)} >>\nstream\n${content}\nendstream`)
  }
  objects.push('<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>')
  let file='%PDF-1.4\n';const offsets=[0]
  objects.forEach((object,index)=>{offsets.push(Buffer.byteLength(file));file+=`${index+1} 0 obj\n${object}\nendobj\n`})
  const xref=Buffer.byteLength(file)
  file+=`xref\n0 ${objects.length+1}\n0000000000 65535 f \n`+offsets.slice(1).map(offset=>`${String(offset).padStart(10,'0')} 00000 n \n`).join('')+`trailer\n<< /Size ${objects.length+1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF`
  return Buffer.from(file)
}
const pdf=pdfFixture()
const settlePaint=page=>page.evaluate(async()=>{await Promise.all(document.getAnimations().filter(animation=>animation.effect?.getTiming().iterations!==Infinity).map(animation=>animation.finished.catch(()=>{})))})

async function assertEdgeRibbon(page, marker) {
  await marker.locator('.reader-bookmark-ribbon').waitFor({timeout:2500})
  await page.waitForFunction(()=>{
    const pane=document.querySelector('.pane-center')
    const ribbon=pane?.querySelector('.reader-bookmark-ribbon')
    return ribbon && Math.abs(ribbon.getBoundingClientRect().right-(pane.getBoundingClientRect().left+pane.clientLeft+pane.clientWidth))<1.5
  },null,{timeout:2500})
  const appearance=await marker.evaluate(button=>{
    const ribbon=button.querySelector('.reader-bookmark-ribbon')
    const tail=ribbon.querySelector('.reader-bookmark-tail')
    const root=getComputedStyle(document.documentElement)
    const ink=getComputedStyle(tail,'::after').backgroundColor.match(/[\d.]+/g).map(Number)
    const base=root.getPropertyValue('--bg-base').trim().slice(1).match(/../g).map(n=>parseInt(n,16))
    const luma=rgb=>rgb.slice(0,3).reduce((sum,value,i)=>{
      const channel=value/255
      return sum+[.2126,.7152,.0722][i]*(channel<=.04045?channel/12.92:((channel+.055)/1.055)**2.4)
    },0)
    const luminances=[luma(ink),luma(base)].sort((a,b)=>b-a)
    return { height:button.offsetHeight, width:button.offsetWidth, tint:getComputedStyle(ribbon).backgroundImage, shape:getComputedStyle(tail).clipPath, contrast:(luminances[0]+.05)/(luminances[1]+.05) }
  })
  assert.ok(appearance.height>=44 && appearance.width>=44,'Ribbon has a full touch target')
  assert.match(appearance.tint,/linear-gradient/)
  assert.match(appearance.shape,/polygon/)
  assert.ok(appearance.contrast>=4.5,JSON.stringify(appearance))
}
const vite=await createServer({root:fileURLToPath(new URL('../',import.meta.url)),configFile:fileURLToPath(new URL('../vite.config.ts',import.meta.url)),server:{host:'127.0.0.1',port:5232,strictPort:true,open:false},plugins:[{name:'deny-user-data',configureServer(server){server.middlewares.use('/v1',(_req,res)=>{res.statusCode=403;res.end('Isolated bookmark verification')})}}]})
let browser
const results=[],errors=[],unexpected=[]
try{
  await vite.listen();const origin='http://127.0.0.1:5232'
  browser=await chromium.launch({headless:true,channel:'chromium'})
  for(const width of [390,1440]){
    let bookmarks=[],sequence=0,failNextSave=false,missingFile=false,badPdf=false
    const context=await browser.newContext({viewport:{width,height:950},isMobile:width<768,hasTouch:width<768})
    await context.route('**/*',async route=>{
      const url=new URL(route.request().url())
      if(url.origin!==origin)return route.abort()
      if(!url.pathname.startsWith('/v1'))return route.continue()
      if(url.pathname==='/v1/reader/raw')return route.fulfill({contentType:'application/pdf',body:badPdf?Buffer.from('invalid PDF fixture'):pdf})
      if(url.pathname==='/v1/health')return route.fulfill({json:{status:'healthy',version:'bookmark-preview',components:{}}})
      const call=route.request().postDataJSON(),args=call?.arguments || {}
      let result
      if(call.tool==='get_reader_books')result={books}
      if(call.tool==='get_reader_history')result={history:[]}
      if(call.tool==='save_reader_history')result={ok:true,count:1}
      if(call.tool==='save_reader_progress')result={state:args.state}
      if(call.tool==='list_local_dir')result={root:args.path,total:3,entries:['one.md','two.md','paper.pdf'].map(name=>({name,path:`${args.path}/${name}`,is_dir:false,is_markdown:name.endsWith('.md'),is_pdf:name.endsWith('.pdf')}))}
      if(call.tool==='read_local_file'){
        if(missingFile && args.path.endsWith('two.md'))return route.fulfill({json:{status:'error',error:{message:'文件已移走，书签笔记仍保留'}}})
        result={path:args.path,name:args.path.split('/').pop(),content:chapter(args.path.endsWith('two.md')?'第二章':'第一章'),size:4000}
      }
      if(call.tool==='list_reader_bookmarks')result={bookmarks:bookmarks.filter(item=>item.book_id===args.book_id)}
      if(call.tool==='save_reader_bookmark'){
        if(failNextSave){failNextSave=false;return route.fulfill({json:{status:'error',error:{message:'书签版本冲突，当前草稿未覆盖'}}})}
        const old=bookmarks.find(item=>item.id===args.id)
        if(args.id && (!old || old.revision!==args.revision))throw new Error('Unexpected mock version')
        const bookmark={...args,id:args.id || `bookmark-${++sequence}`,revision:(old?.revision||0)+1,created_at:old?.created_at||new Date().toISOString(),updated_at:new Date().toISOString()}
        bookmarks=[...bookmarks.filter(item=>item.id!==bookmark.id),bookmark];result={bookmark}
      }
      if(call.tool==='delete_reader_bookmark'){bookmarks=bookmarks.filter(item=>item.id!==args.id);result={deleted:true}}
      if(!result){unexpected.push(call.tool);return route.fulfill({status:400,json:{error:'Unexpected mock request'}})}
      return route.fulfill({json:{tool:call.tool,status:'success',result}})
    })
    const page=await context.newPage();page.on('pageerror',error=>errors.push(error.message))
    await page.goto(`${origin}/reader?book=a&view=read`)
    const article=page.locator('.pane-center>article.markdown-body').last()
    await article.getByRole('heading',{name:'第一章',exact:true}).waitFor()
    const originalPadding=await article.evaluate(el=>getComputedStyle(el).paddingInlineStart)
    const open=async()=>{await page.getByRole('button',{name:width<768?/，本书书签$/:/^本书书签$/}).first().click();await page.getByRole('dialog',{name:'本书书签',exact:true}).waitFor()}
    const create=async(title,body)=>{
      await page.getByRole('button',{name:'添加当前位置书签'}).click()
      assert.equal(await page.getByRole('textbox',{name:'笔记',exact:true}).inputValue(),'','New notes remain empty, even when source text is selected')
      await page.getByLabel('标题',{exact:true}).fill(title)
      await page.getByRole('textbox',{name:'笔记',exact:true}).fill(body)
      await page.getByRole('button',{name:'保存',exact:true}).click()
      await page.locator('.bookmark-detail-title h3').filter({hasText:title}).waitFor()
    }
    const paragraph=article.locator('p').filter({hasText:'第一章段落 13'}).first()
    await page.locator('.pane-center').dispatchEvent('wheel',{deltaY:120})
    await paragraph.scrollIntoViewIfNeeded();await paragraph.click({position:{x:60,y:12}})
    await paragraph.evaluate(element=>{
      const selection=window.getSelection(),range=document.createRange()
      range.selectNodeContents(element);selection.removeAllRanges();selection.addRange(range)
    })
    await paragraph.dispatchEvent('mouseup')
    await page.getByRole('toolbar',{name:'选中文本操作'}).getByRole('button',{name:'书签',exact:true}).click()
    await create('第一条书签',note)
    assert.ok(bookmarks[0].location.text.includes('第一章段落 13'),JSON.stringify(bookmarks[0].location))
    await page.locator('.bookmark-markdown strong').filter({hasText:'关键结论'}).waitFor()
    assert.ok(await page.locator('.bookmark-markdown .katex').count())
    await page.locator('.bookmark-markdown .hljs-keyword').first().waitFor()
    for(const theme of ['light','dark','eye-care']){
      await page.evaluate(async theme=>(await import('/src/stores/app.ts')).useAppStore().setTheme(theme),theme)
      await settlePaint(page)
      assert.equal(await page.locator('html').evaluate(el=>el.scrollWidth>innerWidth),false)
      await page.screenshot({path:join(directory,`${width}-${theme}.png`)})
    }
    await page.getByRole('button',{name:'转到原文'}).click()
    await page.locator('.bookmarks-panel').waitFor({state:'hidden'})
    const marker=page.getByRole('button',{name:'查看书签：第一条书签',exact:true})
    await marker.waitFor()
    assert.equal(await article.evaluate(el=>getComputedStyle(el).paddingInlineStart),originalPadding,'A bookmark must not shift the article')
    for(const theme of ['light','dark','eye-care']){
      await page.evaluate(async theme=>(await import('/src/stores/app.ts')).useAppStore().setTheme(theme),theme)
      await assertEdgeRibbon(page,marker)
      await settlePaint(page)
      await page.screenshot({path:join(directory,`${width}-${theme}-ribbon.png`)})
    }
    if(width>768){
      await page.locator('.reader-fullscreen-btn').click()
      await page.waitForFunction(()=>!!document.fullscreenElement)
      await page.locator('.fs-ui').getByRole('button',{name:'本书书签',exact:true}).click()
      await page.getByRole('dialog',{name:'本书书签',exact:true}).waitFor()
      assert.equal(await page.evaluate(()=>document.fullscreenElement.contains(document.querySelector('.bookmarks-panel'))),true,'Bookmark panel remains inside the fullscreen top layer')
      await page.getByRole('button',{name:'关闭书签'}).click()
      await page.evaluate(()=>document.exitFullscreen())
      await page.waitForFunction(()=>!document.fullscreenElement)
    }
    await marker.click()
    await page.getByRole('button',{name:'编辑书签',exact:true}).click()
    await page.getByLabel('标题',{exact:true}).fill('修改后的书签')
    await page.getByRole('button',{name:'保存',exact:true}).click()
    assert.equal(bookmarks.find(item=>item.title==='修改后的书签')?.note_md,note,'Editing a title preserves existing note content')
    await page.getByRole('button',{name:'关闭书签'}).click()
    await page.reload();await article.getByRole('heading',{name:'第一章',exact:true}).waitFor()
    await open();await page.getByRole('button',{name:'查看笔记：修改后的书签',exact:true}).click()
    await page.locator('.bookmark-markdown strong').filter({hasText:'关键结论'}).waitFor()
    await page.getByRole('button',{name:'转到原文'}).click()
    await page.waitForFunction(()=>document.querySelector('.pane-center').scrollTop>1000)
    const scrollAfter=await page.locator('.pane-center').evaluate(el=>el.scrollTop)
    assert.ok(scrollAfter>1000,JSON.stringify({message:'Reloaded bookmark jumps below the first screen',bookmarks,width,scrollAfter}))
    await article.getByRole('link',{name:'第二章',exact:true}).click()
    await article.getByRole('heading',{name:'第二章',exact:true}).waitFor()
    await open();await create('第二章书签','第二章的笔记')
    await page.getByRole('button',{name:'返回书签列表'}).click()
    assert.equal(await page.locator('.bookmark-row').count(),2)
    await page.getByRole('button',{name:'跳转书签：修改后的书签',exact:true}).click()
    await article.getByRole('heading',{name:'第一章',exact:true}).waitFor()
    await article.getByRole('link',{name:'PDF',exact:true}).click()
    await page.locator('.pdf-page-wrap[data-page-num="3"]').waitFor()
    await article.waitFor({state:'detached'})
    await page.locator('.pane-center').dispatchEvent('wheel',{deltaY:120})
    await page.locator('.pane-center').evaluate(root=>{
      const page=root.querySelector('.pdf-page-wrap[data-page-num="2"]'),rect=page.getBoundingClientRect()
      root.scrollTop+=rect.top-root.getBoundingClientRect().top+rect.height*.6-200
    })
    const pageTwo=page.locator('.pdf-page-wrap[data-page-num="2"]')
    await pageTwo.locator('.pdf-canvas.is-rendered').waitFor()
    const rect=await pageTwo.boundingBox()
    await page.mouse.click(rect.x+60,rect.y+rect.height*.6)
    await open();await create('PDF 书签','PDF **页内**笔记')
    const pdfBookmark=bookmarks.find(item=>item.title==='PDF 书签')
    assert.equal(pdfBookmark.location.page,2)
    assert.ok(Math.abs(pdfBookmark.location.offset-.6)<.03,JSON.stringify({expected:.6,location:pdfBookmark.location,width}))
    await page.getByRole('button',{name:'返回书签列表'}).click()
    await page.getByRole('button',{name:'查看笔记：修改后的书签',exact:true}).click();await page.getByRole('button',{name:'转到原文'}).click()
    await article.getByRole('heading',{name:'第一章',exact:true}).waitFor()
    await open();await page.getByRole('button',{name:'查看笔记：PDF 书签',exact:true}).click();await page.getByRole('button',{name:'转到原文'}).click()
    await pageTwo.waitFor()
    await page.waitForFunction(()=>{
      const root=document.querySelector('.pane-center'),page=root.querySelector('.pdf-page-wrap[data-page-num="2"]')
      return page && Math.abs(page.getBoundingClientRect().top+page.clientHeight*.6-root.getBoundingClientRect().top-64)<8
    })
    const pdfMarker=page.getByRole('button',{name:'查看书签：PDF 书签',exact:true})
    await assertEdgeRibbon(page,pdfMarker)
    if(width<768)await page.locator('.pane-center').dispatchEvent('touchmove')
    await page.getByRole('button',{name:'放大 PDF',exact:true}).click()
    await page.waitForFunction(()=>{const pane=document.querySelector('.pane-center');return pane.scrollWidth-pane.clientWidth>15})
    await page.locator('.pane-center').evaluate(el=>{el.scrollLeft=el.scrollWidth-el.clientWidth})
    await page.waitForFunction(()=>document.querySelector('.pane-center').scrollLeft>15)
    await assertEdgeRibbon(page,pdfMarker)
    await pdfMarker.click();await page.getByRole('button',{name:'编辑书签',exact:true}).click()
    await page.getByRole('textbox',{name:'笔记',exact:true}).fill('冲突时保留的草稿')
    failNextSave=true
    await page.getByRole('button',{name:'保存',exact:true}).click()
    await page.getByRole('alert').filter({hasText:'书签版本冲突'}).waitFor()
    assert.equal(await page.getByRole('textbox',{name:'笔记',exact:true}).inputValue(),'冲突时保留的草稿')
    await page.getByRole('button',{name:'返回书签列表'}).click();await page.getByRole('button',{name:'放弃',exact:true}).click()
    await page.getByRole('button',{name:'查看笔记：第二章书签',exact:true}).click()
    missingFile=true
    await page.getByRole('button',{name:'转到原文'}).click()
    await page.locator('.bookmarks-panel').getByRole('alert').filter({hasText:'文件已移走'}).waitFor()
    assert.equal(bookmarks.length,3,'Missing sources never delete notes')
    await page.getByRole('button',{name:'删除书签',exact:true}).click();await page.getByRole('button',{name:'删除',exact:true}).click()
    await page.waitForFunction(()=>document.querySelectorAll('.bookmark-row').length===2)
    assert.equal(await page.locator('.bookmark-row').count(),2)
    await page.getByRole('button',{name:'跳转书签：修改后的书签',exact:true}).click()
    await article.getByRole('heading',{name:'第一章',exact:true}).waitFor()
    badPdf=true
    await open();await page.getByRole('button',{name:'跳转书签：PDF 书签',exact:true}).click()
    await page.locator('.bookmarks-panel').getByRole('alert').filter({hasText:'PDF 无法打开'}).waitFor()
    assert.equal(bookmarks.length,2,'Failed PDF parsing retains notes')
    await page.getByRole('button',{name:'关闭书签'}).click()
    await page.goto(`${origin}/reader?book=b&view=read`);await article.getByRole('heading',{name:'第一章',exact:true}).waitFor()
    await open();assert.equal(await page.locator('.bookmark-row').count(),0)
    results.push({width,markdown:true,pdf:true,themes:3,edit:true,delete:true,reload:true,crossFile:true,conflictDraft:true,missingSource:true,invalidPdf:true,isolation:true,fullscreen:width>768})
    await context.close()
  }
  assert.deepEqual(errors,[]);assert.deepEqual(unexpected,[])
  console.log(JSON.stringify({results,directory,errors,unexpected},null,2))
}catch(error){
  const pages=browser?.contexts().flatMap(context=>context.pages()) || []
  if(pages.length)await pages[pages.length-1].screenshot({path:join(directory,'failure.png')})
  console.log('Screenshots:',directory)
  throw error
}finally{await browser?.close();await vite.close()}
