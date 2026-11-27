import {test} from 'node:test';
import assert from 'node:assert/strict';
import {validHeader,validateBatch} from './diagnostic-journal-validation.mjs';
const header={type:'header',format:'bansayo-diagnostic-journal-v1',started_utc:'2026-01-01T00:00:00Z',devices:['A']};
const entry=cursor=>({cursor,channel:'gyro',point:{t_us:100,seq:cursor,device_time_us:null,values:[0,1,2]}});
const batch=(next,lost,entries)=>({type:'batch',device:'A',batch:{epoch:1,next,lost,reset:false,entries}});
test('journal header and batch integrity',()=>{
  assert.equal(validHeader(header),true);
  assert.equal(validHeader({...header,devices:['A','A']}),false);
  assert.equal(validateBatch(batch(5,0,[entry(5)]),header.devices),null);
  assert.equal(validateBatch(batch(8,2,[entry(8)]),header.devices,{epoch:1,next:5}),null);
  assert.ok(validateBatch(batch(8,0,[entry(8)]),header.devices,{epoch:1,next:5}));
  assert.ok(validateBatch(batch(8,0,[entry(6),entry(8)]),header.devices));
  assert.ok(validateBatch(batch(8,0,[entry(7)]),header.devices));
  assert.ok(validateBatch(batch(8,0,[{...entry(8),channel:'unknown'}]),header.devices));
  assert.ok(validateBatch(null,header.devices));
});
