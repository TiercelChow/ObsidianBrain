import type { MermaidConfig } from 'mermaid'

export interface DiagramTokens {
  background: string
  foreground: string
  muted: string
  accent: string
  fontFamily: string
}

function mix(background: string, foreground: string, amount: number): string {
  const rgb = (color: string) => color.match(/[\da-f]{2}/gi)?.map(value => parseInt(value, 16)) ?? [0, 0, 0]
  const base = rgb(background), tint = rgb(foreground)
  return `#${base.map((value, index) => Math.round(value + (tint[index] - value) * amount).toString(16).padStart(2, '0')).join('')}`
}

export function getMermaidPalette(tokens: DiagramTokens) {
  return {
    background: tokens.background,
    text: tokens.foreground,
    accent: tokens.accent,
    node: mix(tokens.background, tokens.accent, 0.09),
    secondary: mix(tokens.background, tokens.accent, 0.15),
    group: mix(tokens.background, tokens.foreground, 0.025),
    border: mix(tokens.background, tokens.foreground, 0.28),
    line: mix(tokens.background, tokens.muted, 0.85),
    groupBorder: mix(tokens.background, tokens.foreground, 0.15),
  }
}

/** Concrete colors are required: Mermaid's theme color calculations cannot use CSS var(). */
export function buildMermaidConfig(tokens: DiagramTokens, diagramType = 'flowchart-v2'): MermaidConfig {
  const p = getMermaidPalette(tokens)
  return {
    startOnLoad: false,
    securityLevel: 'strict',
    secure: ['secure', 'securityLevel', 'startOnLoad', 'maxTextSize', 'suppressErrorRendering'],
    suppressErrorRendering: true,
    forceLegacyMathML: true,
    theme: 'base',
    look: 'classic',
    layout: /^(flowchart|graph)/.test(diagramType) ? 'elk' : 'dagre',
    elk: { nodePlacementStrategy: 'NETWORK_SIMPLEX' },
    fontFamily: tokens.fontFamily,
    fontSize: 16,
    htmlLabels: true,
    markdownAutoWrap: true,
    // These rules participate in text measurement AND the final SVG. Never
    // resize labels after layout or let surrounding Markdown typography leak in.
    themeCSS: `
      .nodeLabel, .edgeLabel, .label, .cluster-label { line-height: 1.5; }
      .nodeLabel p, .edgeLabel p, .cluster-label p { margin: 0; padding: 0; }
      .nodeLabel, .edgeLabel, .cluster-label { overflow-wrap: normal; word-break: normal; }
      .node rect, .cluster rect { rx: 9px; ry: 9px; }
      .edgeLabel rect { rx: 5px; ry: 5px; }
      .flowchart-link { stroke-width: 1.5px; stroke-linejoin: round; stroke-linecap: round; }
      .katex { display: inline; min-width: 0; max-width: none; overflow: visible; vertical-align: baseline; }
      .katex-display { max-width: none; overflow: visible; padding: 0; }
      foreignObject img { max-width: none; margin: 0; border-radius: 0; }
      foreignObject svg { max-width: none; }
    `,
    themeVariables: {
      darkMode: parseInt(tokens.background.slice(1, 3), 16) < 100,
      fontFamily: tokens.fontFamily,
      fontSize: '16px',
      background: p.background,
      primaryColor: p.node,
      primaryTextColor: p.text,
      primaryBorderColor: p.border,
      secondaryColor: p.secondary,
      secondaryTextColor: p.text,
      secondaryBorderColor: p.border,
      tertiaryColor: p.group,
      tertiaryTextColor: p.text,
      tertiaryBorderColor: p.groupBorder,
      mainBkg: p.node,
      nodeBorder: p.border,
      nodeTextColor: p.text,
      lineColor: p.line,
      textColor: p.text,
      clusterBkg: p.group,
      clusterBorder: p.groupBorder,
      edgeLabelBackground: p.background,
      titleColor: p.text,
      actorBkg: p.node,
      actorBorder: p.border,
      actorTextColor: p.text,
      actorLineColor: p.line,
      signalColor: p.line,
      signalTextColor: p.text,
      labelBoxBkgColor: p.node,
      labelBoxBorderColor: p.border,
      labelTextColor: p.text,
      loopTextColor: p.text,
      noteBkgColor: p.secondary,
      noteBorderColor: p.border,
      noteTextColor: p.text,
      activationBkgColor: p.secondary,
      activationBorderColor: p.border,
    },
    flowchart: { useMaxWidth: false, padding: 16, nodeSpacing: 36, rankSpacing: 52, wrappingWidth: 240, curve: 'linear' },
    sequence: { useMaxWidth: false, wrap: true, wrapPadding: 16, actorMargin: 56, messageMargin: 40, diagramMarginX: 20, diagramMarginY: 20 },
    gantt: { useMaxWidth: false },
    journey: { useMaxWidth: false },
  }
}
