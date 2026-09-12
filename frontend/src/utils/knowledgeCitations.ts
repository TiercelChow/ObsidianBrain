export interface KnowledgeCitationSegment {
  text: string
  sourceIndex?: number
}

function appendPlain(segments: KnowledgeCitationSegment[], text: string) {
  if (!text) return
  const previous = segments[segments.length - 1]
  if (previous && previous.sourceIndex === undefined) previous.text += text
  else segments.push({ text })
}

export function parseKnowledgeCitations(
  answer: string,
  sourceCount: number,
): KnowledgeCitationSegment[] {
  const segments: KnowledgeCitationSegment[] = []
  const marker = /\[S(\d+)\]/g
  let cursor = 0

  for (const match of answer.matchAll(marker)) {
    const offset = match.index ?? cursor
    appendPlain(segments, answer.slice(cursor, offset))
    const sourceNumber = Number(match[1])
    if (sourceNumber >= 1 && sourceNumber <= sourceCount) {
      segments.push({ text: match[0], sourceIndex: sourceNumber - 1 })
    } else {
      appendPlain(segments, match[0])
    }
    cursor = offset + match[0].length
  }

  appendPlain(segments, answer.slice(cursor))
  return segments
}
