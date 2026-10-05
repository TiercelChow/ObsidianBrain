import test from 'node:test'
import assert from 'node:assert/strict'
import { memoImageUrl, memoImageSource, memoLocalDate, memoToolResult } from '../src/utils/timelineMemo.ts'

test('local image URLs encode Unicode and reserved characters, preserving path separators', () => {
  assert.equal(memoImageUrl('Timeline/images/中文 #问号?.png'),'/v1/timeline/images/Timeline/images/%E4%B8%AD%E6%96%87%20%23%E9%97%AE%E5%8F%B7%3F.png')
  assert.equal(memoImageUrl('a%b.png',true),'/v1/timeline/thumbnails/a%25b.png')
})
test('memo grouping uses persisted local date, not UTC date', () => {
  assert.equal(memoLocalDate({date:'2026-10-04',timestamp:'2026-10-03T18:00:00Z'}),'2026-10-04')
})
test('embedded local pictures use managed storage, remote pictures stay HTTPS and executable URLs are rejected', () => {
  assert.equal(memoImageSource('Timeline/images/a&amp;b.png'), memoImageUrl('Timeline/images/a&b.png'))
  assert.equal(memoImageSource('https://example.com/a.png'), 'https://example.com/a.png')
  assert.equal(memoImageSource('/v1/vault/images/Timeline/images/a.png'), '/v1/vault/images/Timeline/images/a.png')
  assert.equal(memoImageSource('javascript:alert(1)'), '')
})
test('failed tool envelope never looks like a successful edit or deletion', () => {
  assert.throws(()=>memoToolResult({status:'error',error:{message:'小记版本冲突'}}),/版本冲突/)
  assert.throws(()=>memoToolResult({status:'success',result:null}),/返回/)
  assert.deepEqual(memoToolResult({status:'success',result:{id:'one'}}),{id:'one'})
})
