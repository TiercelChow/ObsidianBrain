import { parse } from 'vue/compiler-sfc'

const symbols = /^(?:[+＋−\-×✕✖‹›←→↑↓↶↷↻↺⤢⋯…]|\.\.\.)$/u
const iconTags = new Set(['el-icon', 'svg'])
const classes = node => node.props.find(prop => prop.type === 6 && prop.name === 'class')?.value?.content || ''
const meaningful = children => children.filter(node => node.type !== 3 && !(node.type === 2 && !node.content.trim()))

// Conservative, template-only audit. Text/interpolations never count as icons.
// Status/selection widgets retain their control shape rather than becoming actions.
export function iconActions(source) {
  const template = parse(source).descriptor.template
  const actions = []
  if (!template?.ast) return actions
  function inspect(node) {
    if (node.type === 1 && ['button', 'el-button', 'a'].includes(node.tag) && !classes(node).split(/\s+/).includes('status-orb')) {
      let icons = node.props.some(prop => (prop.type === 6 && prop.name === 'icon') || (prop.type === 7 && prop.name === 'bind' && prop.arg?.content === 'icon')) ? 1 : 0
      const textGlyphs = []
      function onlyIcons(child) {
        if (child.type === 3) return true
        if (child.type === 2) {
          if (!child.content.trim()) return true
          if (!symbols.test(child.content.trim())) return false
          icons += 1
          textGlyphs.push(child.content.trim())
          return true
        }
        if (child.type !== 1) return false
        if (iconTags.has(child.tag) || classes(child).split(/\s+/).includes('stop-square')) {
          icons += 1
          return true
        }
        if (!['span', 'template'].includes(child.tag)) return false
        return meaningful(child.children).every(onlyIcons)
      }
      if (meaningful(node.children).every(onlyIcons) && icons > 0) {
        actions.push({
          tag: node.tag, classes: classes(node), line: node.loc.start.line,
          offset: node.loc.start.offset, // SFC template AST offsets already refer to the whole source.
          textGlyphs,
          marked: node.props.some(prop => prop.type === 6 && prop.name === 'data-icon-only'),
          named: node.props.some(prop => ['aria-label', 'title'].includes(prop.name) || (prop.type === 7 && prop.name === 'bind' && ['aria-label', 'title'].includes(prop.arg?.content))),
        })
      }
    }
    for (const child of node.children || []) inspect(child)
  }
  inspect(template.ast)
  return actions
}
