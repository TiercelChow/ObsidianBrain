import assert from 'node:assert/strict'
import test from 'node:test'
import { compileReportStatus, compileFragmentStatus, compileTopicStatus } from '../src/utils/knowledgeCompileReport.ts'

test('analysis and review labels never imply complete knowledge or proven facts', () => {
  assert.equal(compileReportStatus('waiting_review'), '候选待审核')
  assert.equal(compileReportStatus('applied'), '当次审核已应用')
  assert.equal(compileReportStatus('no_material'), '分析完成 · 无实质新增')
  assert.equal(compileFragmentStatus('analyzed'), '已分析')
  assert.equal(compileFragmentStatus('no_material'), '已分析 · 无实质结果')
  assert.equal(compileFragmentStatus('unprocessed'), '尚未分析')
  assert.equal(compileTopicStatus('excluded_by_archive'), '保留归档 · 未恢复')
  assert.equal(compileTopicStatus('reconciled_topic'), '已归并 · 尚需审核')
  assert.equal(compileTopicStatus('reconciled_topic', 'applied'), '当次审核已应用')
  assert.equal(compileTopicStatus('reconciled_topic', 'failed'), '主题结果已记录 · 本次未完成')
  assert.equal(compileReportStatus('unknown'), '未记录状态')
})
