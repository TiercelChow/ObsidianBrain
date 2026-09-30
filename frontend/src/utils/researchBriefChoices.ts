import type { KnowledgeTask, ResearchPreflight } from '../api/knowledge'

export type ResearchBriefDecisionField = ResearchPreflight['focus_decisions'][number]

const commonFields: readonly ResearchBriefDecisionField[] = ['audience', 'purpose', 'tone', 'depth']
const presentationFields: readonly ResearchBriefDecisionField[] = ['presentation_theme', 'presentation_format']

/** Keep model-suggested defaults editable without burying the decisions it flagged. */
export function visibleResearchBriefFields(
  preflight: { focus_decisions: readonly ResearchBriefDecisionField[] } | null,
  deliverable: KnowledgeTask['deliverable_type'],
  showAll: boolean,
): ResearchBriefDecisionField[] {
  const fields = deliverable === 'presentation'
    ? [...commonFields, ...presentationFields]
    : [...commonFields]
  if (showAll || !preflight?.focus_decisions.length) return fields
  const focused = fields.filter(field => preflight.focus_decisions.includes(field))
  return focused.length ? focused : fields
}
