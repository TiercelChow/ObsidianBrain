// Synthetic, in-memory PDF for viewer tests. No personal books or filesystem writes.
export function createPdfReadingFixture() {
  const objects = []
  const add = body => { objects.push(body); return objects.length }
  add('<< /Type /Catalog /Pages 2 0 R >>')
  add('')
  const font = add('<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>')
  const pages = []
  for (let i = 0; i < 14; i++) {
    const width = i === 3 ? 840 : 600
    const height = i === 3 ? 600 : 850
    const lines = [
      `BT /F1 22 Tf 54 ${height - 70} Td (PDF reading quality - page ${i + 1}) Tj ET`,
      `BT /F1 12 Tf 54 ${height - 104} Td (Pinch, zoom and pan: fine text must stay sharp.) Tj ET`,
      ...Array.from({ length: 24 }, (_, n) => `BT /F1 12 Tf 54 ${height - 130 - n * 22} Td (Line ${String(n + 1).padStart(2, '0')}: Evidence, formulas, citations, references and all details remain readable.) Tj ET`),
      '0.2 0.4 0.6 RG .4 w 54 76 492 35 re S',
      'BT /F1 10 Tf 62 88 Td (Fine strokes and small footer text - horizontal margins remain reachable.) Tj ET',
    ]
    const stream = lines.join('\n')
    const content = add(`<< /Length ${stream.length} >>\nstream\n${stream}\nendstream`)
    pages.push(add(`<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ${width} ${height}] /Resources << /Font << /F1 ${font} 0 R >> >> /Contents ${content} 0 R >>`))
  }
  objects[1] = `<< /Type /Pages /Count ${pages.length} /Kids [${pages.map(id => `${id} 0 R`).join(' ')}] >>`
  let pdf = '%PDF-1.4\n'
  const offsets = [0]
  objects.forEach((body, i) => { offsets.push(pdf.length); pdf += `${i + 1} 0 obj\n${body}\nendobj\n` })
  const xref = pdf.length
  pdf += `xref\n0 ${offsets.length}\n0000000000 65535 f \n`
  for (const offset of offsets.slice(1)) pdf += `${String(offset).padStart(10, '0')} 00000 n \n`
  pdf += `trailer\n<< /Size ${offsets.length} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`
  return new TextEncoder().encode(pdf)
}
