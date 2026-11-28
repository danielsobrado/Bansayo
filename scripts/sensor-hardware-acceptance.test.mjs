import {test} from 'node:test';
import assert from 'node:assert/strict';
import {evaluateHardware} from './sensor-hardware-acceptance.mjs';
const options={rate:200,magRate:10,minRateRatio:.95,maxGapRatio:.01,maxFpsRegression:.1,minSeconds:15};
const epoch='2026-01-01T00:00:00Z';
const benchmark={session_epoch_utc:epoch,devices:[{id:'A',origin:'live',kind:'imu_tracker'}],baseline:{received_fps:120,diagnostics_windows:[{}]},recording:{received_fps:119,diagnostics_windows:[{}],devices:[{id:'A',time_sync:{settled:true}}]},recording_status:{observations:9300,lost:0,window_start_us:100,window_end_us:15_000_100}};
const journal={complete:true,header:{session_epoch_utc:epoch},footer:{reason:'Stopped by operator'},observations:9300,journal_lost:0,window:{start_us:100,end_us:15_000_100,duration_s:15,channels:['orientation','gyro','accel','mag0','mag1'].map((channel,i)=>({device:'A',channel,count:i<3?3000:150,window_rate_hz:i<3?200:10,sequence_gaps:0,nonpositive_intervals:0}))}};
test('explicit engineering thresholds pass valid evidence, not simulation or missing data',()=>{
  assert.equal(evaluateHardware(benchmark,journal,options).passed,true);
  assert.equal(evaluateHardware({...benchmark,devices:[{...benchmark.devices[0],origin:'simulated'}]},journal,options).passed,false);
  assert.equal(evaluateHardware(benchmark,{...journal,journal_lost:1},options).passed,false);
  assert.equal(evaluateHardware(benchmark,{...journal,window:{...journal.window,channels:journal.window.channels.slice(0,3)}},options).passed,false);
});
