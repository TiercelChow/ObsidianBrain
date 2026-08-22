export interface LocalDateParts {
  year: number
  month: number
  day: number
}

export interface CalendarDay {
  date: string
  day: number
  inCurrentMonth: boolean
  isToday: boolean
}

const lunarFormatter = new Intl.DateTimeFormat('zh-CN-u-ca-chinese', {
  month: 'short',
  day: 'numeric',
})

const chineseDigits = ['零', '一', '二', '三', '四', '五', '六', '七', '八', '九', '十']

export function parseLocalDate(value: string): LocalDateParts {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value)
  if (!match) throw new Error(`无效日期: ${value}`)
  const parts = { year: Number(match[1]), month: Number(match[2]), day: Number(match[3]) }
  const date = new Date(parts.year, parts.month - 1, parts.day, 12)
  if (
    date.getFullYear() !== parts.year
    || date.getMonth() !== parts.month - 1
    || date.getDate() !== parts.day
  ) throw new Error(`无效日期: ${value}`)
  return parts
}

export function formatLocalDate(parts: LocalDateParts): string {
  return `${parts.year.toString().padStart(4, '0')}-${parts.month.toString().padStart(2, '0')}-${parts.day.toString().padStart(2, '0')}`
}

/** Compact lunar label used in calendar cells: month on the first, day otherwise. */
export function formatLunarDate(value: string): string {
  const parts = parseLocalDate(value)
  const date = new Date(parts.year, parts.month - 1, parts.day, 12)
  const lunarParts = lunarFormatter.formatToParts(date)
  const month = lunarParts.find(part => part.type === 'month')?.value || ''
  const lunarDay = Number(lunarParts.find(part => part.type === 'day')?.value)

  if (!Number.isFinite(lunarDay)) return month
  if (lunarDay === 1) return month
  if (lunarDay <= 10) return `初${chineseDigits[lunarDay]}`
  if (lunarDay < 20) return `十${chineseDigits[lunarDay - 10]}`
  if (lunarDay === 20) return '二十'
  if (lunarDay < 30) return `廿${chineseDigits[lunarDay - 20]}`
  return '三十'
}

export function todayLocal(): string {
  const now = new Date()
  return formatLocalDate({ year: now.getFullYear(), month: now.getMonth() + 1, day: now.getDate() })
}

export function addLocalDays(value: string, amount: number): string {
  const parts = parseLocalDate(value)
  const date = new Date(parts.year, parts.month - 1, parts.day + amount, 12)
  return formatLocalDate({ year: date.getFullYear(), month: date.getMonth() + 1, day: date.getDate() })
}

export function shiftMonth(value: string, amount: number): string {
  const parts = parseLocalDate(value)
  const date = new Date(parts.year, parts.month - 1 + amount, 1, 12)
  return formatLocalDate({ year: date.getFullYear(), month: date.getMonth() + 1, day: 1 })
}

export function buildMonthGrid(anchor: string, today = todayLocal()): CalendarDay[] {
  const parts = parseLocalDate(anchor)
  const first = new Date(parts.year, parts.month - 1, 1, 12)
  const mondayOffset = (first.getDay() + 6) % 7
  const gridStart = addLocalDays(formatLocalDate({ year: parts.year, month: parts.month, day: 1 }), -mondayOffset)
  return Array.from({ length: 42 }, (_, index) => {
    const date = addLocalDays(gridStart, index)
    const current = parseLocalDate(date)
    return {
      date,
      day: current.day,
      inCurrentMonth: current.year === parts.year && current.month === parts.month,
      isToday: date === today,
    }
  })
}

export function rangesOverlap(
  startA: string,
  endA: string,
  startB: string,
  endB: string,
): boolean {
  return startA <= endB && endA >= startB
}

export function dateInRange(date: string, start: string, end: string): boolean {
  return start <= date && date <= end
}

export function formatTaskDateRange(start: string, end: string): string {
  if (start === end) return formatShortDate(start)
  return `${formatShortDate(start)} – ${formatShortDate(end)}`
}

export function formatShortDate(value: string): string {
  const parts = parseLocalDate(value)
  return `${parts.month}月${parts.day}日`
}

export function monthTitle(value: string): string {
  const parts = parseLocalDate(value)
  return `${parts.year}年 ${parts.month}月`
}

/** Format an ISO timestamp as "M/D HH:mm" for activity feeds; unparseable values pass through. */
export function formatTimestamp(value: string): string {
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString('zh-CN', { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit' })
}
