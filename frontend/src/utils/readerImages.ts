/**
 * Wiring between the markdown pipeline and the Reader's raw-file endpoint:
 * build the per-note image resolvers consumed by useMarkdownRender.
 *
 * Obsidian wiki embeds (`![[Timeline/images/a.jpg]]`) carry opened-root-relative
 * paths (vault semantics — the vault evidence is all path-qualified), while
 * standard markdown hrefs are relative to the note's own directory.
 */

import { localFileUrl } from '@/api/reader'
import type { MarkdownImageResolvers } from '@/composables/useMarkdownRender'
import { resolveRelativePath, safeDecodeHref } from './markdownImages'

export function makeReaderImageResolvers(notePath: string, root: string): MarkdownImageResolvers {
  const noteDir = notePath.substring(0, notePath.lastIndexOf('/'))
  const rootDir = root.replace(/\/$/, '') || noteDir
  return {
    resolveEmbed: (target) =>
      localFileUrl(resolveRelativePath(rootDir, target.replace(/^\//, ''))),
    resolveImage: (href) => {
      // Idempotence: embed-converted images already carry a /v1/reader/raw URL
      // (they flow through this renderer too) — re-wrapping would nest the query.
      if (href.startsWith('/v1/reader/raw')) return href
      return localFileUrl(resolveRelativePath(noteDir, safeDecodeHref(href.split('#')[0])))
    },
  }
}
