/** Assemble SSE data blocks across arbitrary network chunk boundaries. */
export function createSseDataParser(onData: (data: string) => void) {
  let buffer = ''

  function consume(block: string) {
    const data = block
      .split('\n')
      .filter(line => line.startsWith('data:'))
      .map(line => line.slice(5).replace(/^ /, ''))
      .join('\n')
    if (data) onData(data)
  }

  return {
    push(chunk: string) {
      // Join before normalizing so a CRLF split across reads remains one newline.
      buffer += chunk
      buffer = buffer.replace(/\r\n/g, '\n')
      let boundary = buffer.indexOf('\n\n')
      while (boundary >= 0) {
        consume(buffer.slice(0, boundary))
        buffer = buffer.slice(boundary + 2)
        boundary = buffer.indexOf('\n\n')
      }
    },
    finish() {
      if (buffer.trim()) consume(buffer)
      buffer = ''
    },
  }
}
