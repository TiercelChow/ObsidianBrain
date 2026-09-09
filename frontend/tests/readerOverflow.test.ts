import assert from 'node:assert/strict'
import test from 'node:test'

import { getHorizontalOverflowPosition } from '../src/utils/readerOverflow.ts'

test('horizontal overflow cue distinguishes both scroll edges', () => {
  assert.equal(getHorizontalOverflowPosition(0, 320, 320), 'none')
  assert.equal(getHorizontalOverflowPosition(0, 320, 640), 'start')
  assert.equal(getHorizontalOverflowPosition(120, 320, 640), 'middle')
  assert.equal(getHorizontalOverflowPosition(320, 320, 640), 'end')
})

test('horizontal overflow cue tolerates subpixel measurements', () => {
  assert.equal(getHorizontalOverflowPosition(0.4, 320, 640), 'start')
  assert.equal(getHorizontalOverflowPosition(319.4, 320, 640), 'end')
})
