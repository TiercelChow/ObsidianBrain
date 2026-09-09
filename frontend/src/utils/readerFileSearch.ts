import type { DirEntry } from '@/api/reader'

export interface ReaderFileSearchResult {
  name: string
  path: string
  relativePath: string
  kind: 'markdown' | 'pdf'
}

function relativeToRoot(path: string, root: string) {
  const normalizedPath = path.replace(/\\/g, '/')
  const normalizedRoot = root.replace(/\\/g, '/').replace(/\/+$/, '')
  const prefix = normalizedRoot ? `${normalizedRoot}/` : ''
  return prefix && normalizedPath.startsWith(prefix)
    ? normalizedPath.slice(prefix.length)
    : normalizedPath
}

function collectReadableFiles(entries: DirEntry[], root: string, output: ReaderFileSearchResult[]) {
  for (const entry of entries) {
    if (entry.is_dir) {
      if (entry.children?.length) collectReadableFiles(entry.children, root, output)
      continue
    }
    if (!entry.is_markdown && !entry.is_pdf) continue
    output.push({
      name: entry.name,
      path: entry.path,
      relativePath: relativeToRoot(entry.path, root),
      kind: entry.is_pdf ? 'pdf' : 'markdown',
    })
  }
}

export function searchReaderFiles(
  entries: DirEntry[],
  root: string,
  query: string,
  limit = 60,
): ReaderFileSearchResult[] {
  const terms = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean)
  if (!terms.length || limit <= 0) return []

  const files: ReaderFileSearchResult[] = []
  collectReadableFiles(entries, root, files)
  return files
    .filter((file) => {
      const searchable = `${file.name}\n${file.relativePath}`.toLocaleLowerCase()
      return terms.every((term) => searchable.includes(term))
    })
    .sort((a, b) => a.relativePath.localeCompare(b.relativePath, 'zh-CN', { numeric: true }))
    .slice(0, limit)
}
