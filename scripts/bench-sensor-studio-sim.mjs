/** Isolated simulator smoke benchmark. Results are explicitly not hardware acceptance. */
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdir, readFile, copyFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { root,scratch,command,start,stop,cleanup } from './engine-test-harness.mjs';

const out=resolve(process.argv[2]??'tmp/studio-sim-benchmark');
await mkdir(out,{recursive:true});
async function node(args){
  const child=spawn(process.execPath,args,{cwd:root,stdio:'inherit'});
  assert.equal((await once(child,'exit'))[0],0,args[0]+' failed');
}
try{
  await command(['build','-p','bansayo-engine']);
  const name='sim-six-trackers',dataName='sensor-studio-bench';
  const rig=await start(name,dataName,yaml=>yaml.replace('schema_version: 1','schema_version: 2')+'\ndiagnostics:\n  timing: true\n');
  const benchmark=join(out,'benchmark.json');
  await node(['scripts/bench-sensor-studio.mjs','--url',rig.base,'--devices','SIM-T01,SIM-T02,SIM-T03,SIM-T04','--seconds','15','--label','simulated-debug-ci','--out',benchmark]);
  const report=JSON.parse(await readFile(benchmark,'utf8'));
  assert.ok(report.recording_status.observations>0,'No accepted observations recorded');
  const source=join(scratch,dataName,report.recording_status.file);
  const capture=join(out,'capture.jsonl');
  await copyFile(source,capture);
  await node(['scripts/analyze-diagnostic-recording.mjs',capture,join(out,'capture-report.json')]);
  await stop(rig.child);
}finally{await cleanup();}
