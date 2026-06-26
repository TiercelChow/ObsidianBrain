import { callTool } from '@/api'

export function fetchExternal(limit = 10) {
  return callTool('get_radar', { limit })
}

export function prescreenArticle(articleId: string) {
  return callTool('prescreen_article', { article_id: articleId })
}

export function batchIngest(articleIds: string[]) {
  return callTool('batch_ingest', { article_ids: articleIds })
}
