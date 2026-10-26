import assert from 'node:assert/strict'

/** Read-only geometry audit of the currently visible real controls. */
export async function verifyIconGeometry(page, label = '') {
  const result = await page.locator('[data-icon-only], .ui-action, .el-dialog__headerbtn, .el-drawer__close-btn, .el-message-box__headerbtn').evaluateAll(elements => {
    const visible = element => {
      const rect = element.getBoundingClientRect(), style = getComputedStyle(element)
      return rect.width > 0 && rect.height > 0 && style.visibility === 'visible' && style.display !== 'none'
    }
    const checks = []
    for (const element of elements) {
      if (!visible(element) || element.closest('[inert]')) continue
      const rect = element.getBoundingClientRect()
      if (rect.right < 0 || rect.left > innerWidth || rect.bottom < 0 || rect.top > innerHeight) continue
      const glyphs = [...element.querySelectorAll('svg, .stop-square')].filter(visible)
      const glyph = glyphs[0]?.getBoundingClientRect()
      checks.push({ name: element.getAttribute('aria-label') || element.getAttribute('title') || element.className, count: glyphs.length,
        dx: glyph ? Math.abs(glyph.x + glyph.width / 2 - rect.x - rect.width / 2) : null,
        dy: glyph ? Math.abs(glyph.y + glyph.height / 2 - rect.y - rect.height / 2) : null,
        width: rect.width, height: rect.height })
    }
    return { checked: checks.length, violations: checks.filter(check => check.count !== 1 || check.dx > .75 || check.dy > .75) }
  })
  assert.deepEqual(result.violations, [], `icon centers in ${label || page.url()}`)
  return result.checked
}
