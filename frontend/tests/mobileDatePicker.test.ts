import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'
import { clampDateParts, dateParts, dateKey, createDateDraft, replaceDraftDate, datePickerValue, wheelIndex } from '../src/utils/dateWheel.ts'

test('month changes clamp days using Gregorian leap rules, not date overflow', () => {
  assert.deepEqual(clampDateParts({ year: 2024, month: 2, day: 31 }), { year: 2024, month: 2, day: 29 })
  assert.deepEqual(clampDateParts({ year: 2100, month: 2, day: 29 }), { year: 2100, month: 2, day: 28 })
  assert.deepEqual(clampDateParts({ year: 2000, month: 2, day: 29 }), { year: 2000, month: 2, day: 29 })
  assert.deepEqual(clampDateParts({ year: 2026, month: 4, day: 31 }), { year: 2026, month: 4, day: 30 })
})

test('date drafts preserve local dates, treat invalid values as unselected and do not mutate inputs', () => {
  const fallback = new Date(2026, 9, 18, 12)
  assert.equal(dateKey(dateParts(new Date(2026, 9, 18, 0, 1), fallback)), '2026-10-18')
  assert.equal(dateKey(dateParts('2024-02-29', fallback)), '2024-02-29')
  for (const value of [null, undefined, '', '2026-02-31', '2026-10-18junk', new Date(NaN)]) {
    assert.equal(dateKey(dateParts(value, fallback)), '2026-10-18')
  }
  const input: [string, string] = ['2026-10-20', '2026-10-10']
  const draft = createDateDraft(input, true, fallback)
  assert.deepEqual(draft.map(dateKey), ['2026-10-10', '2026-10-20'])
  assert.deepEqual(input, ['2026-10-20', '2026-10-10'])
})

test('range edits keep endpoints ordered while single dates remain independent', () => {
  const original = createDateDraft(['2026-10-01', '2026-10-10'], true)
  const forward = replaceDraftDate(original, 0, { year: 2026, month: 11, day: 2 }, true)
  assert.deepEqual(forward.map(dateKey), ['2026-11-02', '2026-11-02'])
  const backward = replaceDraftDate(original, 1, { year: 2026, month: 9, day: 30 }, true)
  assert.deepEqual(backward.map(dateKey), ['2026-09-30', '2026-09-30'])
  assert.deepEqual(original.map(dateKey), ['2026-10-01', '2026-10-10'])
})

test('commits preserve the existing Date and string contracts without UTC conversion', () => {
  const draft = createDateDraft(['2024-02-29', '2026-10-18'], true)
  assert.equal(datePickerValue(draft, false, true), '2024-02-29')
  assert.deepEqual(datePickerValue(draft, true, true), ['2024-02-29', '2026-10-18'])
  const dates = datePickerValue(draft, true, false) as [Date, Date]
  assert.deepEqual(dates.map(date => [date.getFullYear(), date.getMonth() + 1, date.getDate(), date.getHours()]), [[2024, 2, 29, 0], [2026, 10, 18, 0]])
  assert.equal(dateKey(dateParts(datePickerValue(draft, false, false) as Date)), '2024-02-29')
})

test('wheel selection rounds at row centers and clamps overscroll and invalid geometry', () => {
  assert.equal(wheelIndex(65, 12), 1)
  assert.equal(wheelIndex(66, 12), 2)
  assert.equal(wheelIndex(-88, 12), 0)
  assert.equal(wheelIndex(9999, 12), 11)
  assert.equal(wheelIndex(NaN, 12), 0)
  assert.equal(wheelIndex(44, 0), 0)
})

test('all product date inputs use the shared responsive picker rather than independent mobile calendars', async () => {
  for (const page of ['Tasks.vue', 'Timeline.vue', 'knowledge/WikiSettings.vue']) {
    const source = await readFile(new URL(`../src/views/${page}`, import.meta.url), 'utf8')
    assert.match(source, /import AppDatePicker from '@\/components\/controls\/AppDatePicker.vue'/)
    assert.doesNotMatch(source, /<el-date-picker\b/)
    assert.match(source, /<AppDatePicker\b/)
  }
  const picker = await readFile(new URL('../src/components/controls/AppDatePicker.vue', import.meta.url), 'utf8')
  assert.match(picker, /<el-date-picker\s+v-if="!isMobile"/)
  assert.match(picker, /<MotionModal/)
  assert.match(picker, /\.settle\(\)/)
  const wheel = await readFile(new URL('../src/components/controls/DateWheelColumn.vue', import.meta.url), 'utf8')
  assert.match(wheel, /createDateWheelMotion/)
  assert.match(wheel, /touch-action:\s*none/)
  assert.match(wheel, /role="listbox"/)
  assert.match(wheel, /role="option"/)
  assert.match(wheel, /overscroll-behavior:\s*contain/)
})
