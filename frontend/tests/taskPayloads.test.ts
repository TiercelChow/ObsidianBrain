import assert from 'node:assert/strict'
import test from 'node:test'

import { taskFieldsPayload } from '../src/utils/taskPayloads.ts'

test('task fields payload strips form-only kind before subtask requests', () => {
  const payload = taskFieldsPayload({
    kind: 'long',
    title: '梳理月度目标',
    description: '拆解为可执行步骤',
    start_date: '2026-08-19',
    end_date: '2026-08-25',
    importance: 'high',
  })

  assert.deepEqual(payload, {
    title: '梳理月度目标',
    description: '拆解为可执行步骤',
    start_date: '2026-08-19',
    end_date: '2026-08-25',
    importance: 'high',
  })
  assert.equal('kind' in payload, false)
})
