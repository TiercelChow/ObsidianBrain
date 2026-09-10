import {
  renderMarkdownDocument,
  type MarkdownResourceContext,
} from '@/markdown/renderMarkdown'

interface MarkdownWorkerRequest {
  id: number
  source: string
  documentKey?: string
  resourceContext?: MarkdownResourceContext
}

interface MarkdownWorkerScope {
  onmessage: ((event: MessageEvent<MarkdownWorkerRequest>) => void) | null
  postMessage: (message: unknown) => void
}

const workerScope = self as unknown as MarkdownWorkerScope

workerScope.onmessage = (event) => {
  const { id, source, documentKey, resourceContext } = event.data
  const result = renderMarkdownDocument(source, { documentKey, resourceContext })
  workerScope.postMessage({ id, result })
}
