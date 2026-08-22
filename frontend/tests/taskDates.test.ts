import assert from 'node:assert/strict'
import test from 'node:test'

import {
  addLocalDays,
  buildMonthGrid,
  dateInRange,
  formatLunarDate,
  formatLocalDate,
  formatTimestamp,
  parseLocalDate,
  rangesOverlap,
} from '../src/utils/taskDates.ts'

test('local date parsing does not depend on UTC string parsing', () => {
  assert.deepEqual(parseLocalDate('2026-08-17'), { year: 2026, month: 8, day: 17 })
  assert.equal(formatLocalDate({ year: 2026, month: 8, day: 7 }), '2026-08-07')
})

test('month grid always contains six complete Monday-first weeks', () => {
  const grid = buildMonthGrid('2026-08-15', '2026-08-17')
  assert.equal(grid.length, 42)
  assert.equal(grid[0]?.date, '2026-07-27')
  assert.equal(grid[41]?.date, '2026-09-06')
  assert.equal(grid.find(day => day.isToday)?.date, '2026-08-17')
})

test('date math crosses leap day and range overlap is inclusive', () => {
  assert.equal(addLocalDays('2028-02-28', 1), '2028-02-29')
  assert.equal(rangesOverlap('2026-08-10', '2026-08-17', '2026-08-17', '2026-08-20'), true)
  assert.equal(dateInRange('2026-08-17', '2026-08-17', '2026-08-17'), true)
})

test('lunar date uses compact Chinese calendar labels', () => {
  assert.equal(formatLunarDate('2026-02-17'), '正月')
  assert.equal(formatLunarDate('2026-08-19'), '初七')
  assert.equal(formatLunarDate('2026-09-11'), '八月')
})

test('formatTimestamp keeps unparseable values verbatim', () => {
  assert.equal(formatTimestamp('not-a-date'), 'not-a-date')
})

test('formatTimestamp renders month/day and hh:mm', () => {
  assert.match(formatTimestamp('2026-08-21T09:52:16Z'), /^\d{1,2}\/\d{1,2},? \d{2}:\d{2}$/)
})
