import api from '@/api'

export interface TopicReviewLocation {
  page_id: string
  title: string
  heading: string
  line: number
  quote: string
}

export interface TopicReviewIssue {
  kind: 'number' | 'quotation' | 'unsupported' | 'missing_condition' | 'contradiction'
  location: TopicReviewLocation
  related: TopicReviewLocation | null
  detail: string
}

export interface TopicReviewReport {
  run_id: string
  reviewed_at: string
  reviewed_pages: number
  published_pages: number
  reviewed_passages: number
  eligible_passages: number
  issues: TopicReviewIssue[]
}

interface TopicReviewStatus {
  suspected: TopicReviewIssue[]
  latest: TopicReviewReport | null
}

const reviewUrl = (baseId: string) => `/knowledge/bases/${encodeURIComponent(baseId)}/topics/semantic-review`

export const getTopicReview = (baseId: string) => api.get<unknown, TopicReviewStatus>(reviewUrl(baseId))
export const runTopicReview = (baseId: string) => api.post<unknown, TopicReviewReport>(reviewUrl(baseId))
