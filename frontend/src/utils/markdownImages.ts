/**
 * Local-image plumbing for the markdown pipeline (Reader).
 *
 * Notes authored in Obsidian embed images with wiki syntax `![[path]]` that
 * plain markdown does not understand, and standard `![alt](path)` srcs resolve
 * against the app origin instead of the filesystem. These helpers convert
 * embeds and resolve hrefs so both render through the backend's
 * `/v1/reader/raw` file endpoint.
 */

/** Extensions Obsidian treats as media embeds (matched case-insensitively). */
const IMAGE_EXT = /\.(png|jpe?g|gif|webp|svg|bmp|avif|ico)$/i

/** Whether an image href points at the local filesystem (not web/data/blob). */
export function isLocalHref(href: string): boolean {
  return !/^(https?:|data:|blob:|mailto:|ftp:|tel:|\/\/)/i.test(href.trim())
}

/** decodeURIComponent that passes malformed sequences through instead of
 *  throwing — a stray `%` in a filename must not break the whole render. */
export function safeDecodeHref(s: string): string {
  try {
    return decodeURIComponent(s)
  } catch {
    return s
  }
}

/**
 * Join a relative path onto a base directory (macOS-style `/` separators).
 * `.` segments are skipped, `..` pops one segment; mirrors the Reader's link
 * resolution so images and links land on the same file. Paths keep their raw
 * characters (spaces, CJK) — callers URL-encode when building request URLs.
 */
export function resolveRelativePath(baseDir: string, rel: string): string {
  const parts = baseDir ? baseDir.split('/') : []
  for (const seg of rel.replace(/^\.\//, '').split('/')) {
    if (seg === '' || seg === '.') continue
    if (seg === '..') parts.pop()
    else parts.push(seg)
  }
  return parts.join('/')
}

/**
 * Convert Obsidian wiki image embeds (`![[a.jpg]]`, `![[a.jpg|300]]`) to
 * standard markdown images whose URL comes from `resolve(target)`. Non-image
 * embeds (notes, anchors) and a `null` from `resolve` pass through untouched;
 * the Obsidian size suffix is accepted but dropped — markdown has no width
 * syntax and `.markdown-body img` already caps width at 100%.
 */
export function convertObsidianImageEmbeds(
  md: string,
  resolve: (target: string) => string | null,
): string {
  return md.replace(/!\[\[([^\]]+)\]\]/g, (whole, inner: string) => {
    const [rawPath] = inner.split('|')
    const target = rawPath.trim()
    if (!IMAGE_EXT.test(target)) return whole
    const url = resolve(target)
    if (!url) return whole
    const alt = target.split('/').pop() || target
    return `![${alt}](${url})`
  })
}
