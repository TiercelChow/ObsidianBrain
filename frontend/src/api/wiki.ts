export function ingestSource(sourcePath: string, sourceType = 'article', sourceUrl?: string) {
  return callTool('ingest_source', {
    source_path: sourcePath,
    source_type: sourceType,
    ...(sourceUrl ? { source_url: sourceUrl } : {}),
  })
}

export function queryWiki(question: string, saveAnswer = false) {
  return callTool('query_wiki', { question, save_answer: saveAnswer })
}

export function lintWiki(autoFix = false) {
  return callTool('lint_wiki', { auto_fix: autoFix })
}

export function getWikiStatus() {
  return callTool('get_wiki_status')
}