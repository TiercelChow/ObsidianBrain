export interface DateParts { year: number; month: number; day: number }
export type PickerDate = Date | string | null | undefined
export type DatePickerModel = PickerDate | [Date, Date] | [string, string]
export type DateDraft = [DateParts, DateParts]

export function clampDateParts(parts: DateParts): DateParts {
  const year = Math.max(1, Math.min(9999, Math.trunc(parts.year)))
  const month = Math.max(1, Math.min(12, Math.trunc(parts.month)))
  const leap = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0)
  const days = [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][month - 1]
  return { year, month, day: Math.max(1, Math.min(days, Math.trunc(parts.day))) }
}

export function dateKey(parts: DateParts): string {
  return `${String(parts.year).padStart(4, '0')}-${String(parts.month).padStart(2, '0')}-${String(parts.day).padStart(2, '0')}`
}

export function dateParts(value: PickerDate, fallback = new Date()): DateParts {
  if (value instanceof Date && Number.isFinite(value.getTime())) {
    return { year: value.getFullYear(), month: value.getMonth() + 1, day: value.getDate() }
  }
  if (typeof value === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(value)) {
    const [year, month, day] = value.split('-').map(Number)
    const clamped = clampDateParts({ year, month, day })
    if (dateKey(clamped) === value) return clamped
  }
  return { year: fallback.getFullYear(), month: fallback.getMonth() + 1, day: fallback.getDate() }
}

export function createDateDraft(value: DatePickerModel, range: boolean, now = new Date()): DateDraft {
  const start = dateParts(Array.isArray(value) ? value[0] : value, now)
  const end = range && Array.isArray(value) ? dateParts(value[1], now) : { ...start }
  return dateKey(start) <= dateKey(end) ? [start, end] : [end, start]
}

export function replaceDraftDate(draft: DateDraft, endpoint: 0 | 1, value: DateParts, range: boolean): DateDraft {
  const next: DateDraft = [{ ...draft[0] }, { ...draft[1] }]
  next[endpoint] = clampDateParts(value)
  if (range && dateKey(next[0]) > dateKey(next[1])) next[endpoint === 0 ? 1 : 0] = { ...next[endpoint] }
  return next
}

export function datePickerValue(draft: DateDraft, range: boolean, asString: boolean): DatePickerModel {
  const localDate = (parts: DateParts) => {
    // setFullYear also preserves years 1–99, unlike new Date(year, month, day).
    const date = new Date(0)
    date.setFullYear(parts.year, parts.month - 1, parts.day)
    date.setHours(0, 0, 0, 0)
    return date
  }
  if (asString) return range ? [dateKey(draft[0]), dateKey(draft[1])] : dateKey(draft[0])
  return range ? [localDate(draft[0]), localDate(draft[1])] : localDate(draft[0])
}

export function wheelIndex(scrollTop: number, count: number): number {
  return Math.max(0, Math.min(Math.max(0, count - 1), Math.round((Number.isFinite(scrollTop) ? scrollTop : 0) / 44)))
}
