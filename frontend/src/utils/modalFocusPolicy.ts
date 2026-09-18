/** A background tab must never reclaim focus from another app. */
export function canFocusDocument(doc: Pick<Document, 'visibilityState' | 'hasFocus'>): boolean {
  return doc.visibilityState === 'visible' && doc.hasFocus()
}
