/** Compare observation recording off/on against a running local engine. Node 22+. */
import assert from 'node:assert/strict';
import { writeFile } from 'node:fs/promises';
import { setTimeout as delay } from 'node:timers/promises';
import { performance } from 'node:perf_hooks';
import os from 'node:os';

const args=process.argv.slice(2);
const option=(name,fallback)=>{const i=args.indexOf(name);return i<0?fallback:args[i+1];};
const base=option('--url','http://127.0.0.1:7700/api/v1').replace(/\/$/,'');
const seconds=Number(option('--seconds','30'));
const ids=option('--devices','').split(',').filter(Boolean);
const label=option('--label','unspecified');
const output=option('--out','sensor-studio-benchmark.json');
assert.ok(Number.isInteger(seconds)&&seconds>=15&&seconds<=300,'--seconds must be 15–300');
assert.ok(ids.length>=1&&ids.length<=4&&new Set(ids).size===ids.length,'Provide 1–4 distinct --devices');
const endpoint=new URL(base);
assert.ok(['127.0.0.1','localhost','[::1]'].includes(endpoint.hostname),'Local engine only');
async function api(path,method='GET',body){
  const res=await fetch(base+path,{method,headers:body?{'Content-Type':'application/json'}:{},body:body?JSON.stringify(body):undefined,signal:AbortSignal.timeout(5000)});
  const envelope=await res.json();
  if(!res.ok||!envelope.ok)throw Error(path+': '+JSON.stringify(envelope));
  return envelope.data;
}
const percentile=(values,p)=>{const sorted=[...values].sort((a,b)=>a-b);return sorted.length?sorted[Math.round((sorted.length-1)*p)]:null;};
const initial=await api('/state');
for(const id of ids)assert.ok(initial.devices.some(d=>d.descriptor.id===id&&d.descriptor.kind!=='camera'),'Unknown or unsupported device '+id);
assert.equal((await api('/inspection/recording')).running,false,'Stop existing recording first');
const descriptor=initial.devices.filter(d=>ids.includes(d.descriptor.id)).map(d=>d.descriptor);
const ws=new WebSocket(base.replace(/^http/,'ws')+'/stream?topics=frame,state,diagnostics');
let active=null,hello=null,closed=false;
ws.addEventListener('close',()=>closed=true);
ws.addEventListener('message',event=>{
  const m=JSON.parse(event.data);
  if(m.type==='hello')hello=m;
  if(!active)return;
  if(m.type==='frame'){
    const now=performance.now();
    if(active.lastArrival!==null)active.intervals.push(now-active.lastArrival);
    if(active.lastSeq!==null)active.sequenceGaps+=Math.max(0,m.seq-active.lastSeq-1);
    active.lastArrival=now;active.lastSeq=m.seq;active.frames++;
  }
  if(m.type==='diagnostics')active.diagnostics.push(m);
});
let owns=false;
async function phase(name){
  const before=await api('/state');
  active={frames:0,intervals:[],lastArrival:null,lastSeq:null,sequenceGaps:0,diagnostics:[]};
  const started=performance.now();
  await delay(seconds*1000);
  const elapsed=(performance.now()-started)/1000;
  const result=active;active=null;
  assert.ok(!closed&&result.frames>0,'Frame stream unavailable');
  const after=await api('/state');
  const devices=after.devices.filter(d=>ids.includes(d.descriptor.id)).map(d=>{
    const b=before.devices.find(x=>x.descriptor.id===d.descriptor.id);
    return {id:d.descriptor.id,rate_hz:d.stream.measured_rate_hz,received_delta:d.stream.received-(b?.stream.received??0),dropped_delta:d.stream.dropped-(b?.stream.dropped??0),time_sync:d.time_sync};
  });
  return {name,elapsed_s:elapsed,frames:result.frames,received_fps:result.frames/elapsed,client_frame_sequence_gaps:result.sequenceGaps,arrival_interval_ms:{p50:percentile(result.intervals,.5),p95:percentile(result.intervals,.95),p99:percentile(result.intervals,.99)},devices,diagnostics_windows:result.diagnostics};
}
try{
  for(let i=0;i<100&&!hello&&!closed;i++)await delay(50);
  assert.ok(hello,'WebSocket handshake failed');
  console.log('Warm-up: 12 seconds; do not change the rig during measurement');
  await delay(12000);
  const baseline=await phase('baseline');
  await api('/inspection/recording','POST',{device_ids:ids,duration_s:seconds+20});owns=true;
  let status;
  for(let i=0;i<50;i++){status=await api('/inspection/recording');if(status.file||!status.running)break;await delay(100);}
  assert.ok(status?.running&&status.file,'Recording failed to start');
  const recording=await phase('recording');
  await api('/inspection/recording/stop','POST');
  for(let i=0;i<100;i++){status=await api('/inspection/recording');if(!status.running)break;await delay(100);}
  assert.ok(status&&!status.running&&status.file?.endsWith('.jsonl'),'Recording did not finalize');
  owns=false;
  const report={format:'bansayo-studio-benchmark-v1',label,created_utc:new Date().toISOString(),engine_version:hello.engine_version,session_epoch_utc:hello.session_epoch_utc,host:{platform:os.platform(),release:os.release(),cpu:os.cpus()[0]?.model,logical_cpus:os.cpus().length,memory_bytes:os.totalmem()},devices:descriptor,baseline,recording,recording_status:status,limitations:['Client arrival intervals are not motion-to-photon latency.','Stage timings require diagnostics.timing: true.','CPU/GPU process utilization requires external sampling.','No automatic pass threshold without hardware baseline.']};
  await writeFile(output,JSON.stringify(report,null,2)+'\n',{flag:'wx'});
  console.log(JSON.stringify({baseline_fps:baseline.received_fps,recording_fps:recording.received_fps,observations:status.observations,lost:status.lost,output},null,2));
}finally{
  if(owns)await api('/inspection/recording/stop','POST').catch(e=>console.error(e.message));
  ws.close();
}
