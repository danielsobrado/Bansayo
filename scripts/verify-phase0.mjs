/** Phase 0 additions on the existing API; gate1 and gate2 remain unchanged. */
import assert from 'node:assert/strict';
import { command, start, stream, stop, cleanup } from './engine-test-harness.mjs';
try {
  await command(['build','-p','bansayo-engine']);
  for (const [name,profile] of [['sim-full-body','hybrid'],['sim-six-trackers','inertial'],['sim-camera-only','basic'],['empty','none']]) {
    const engine=await start(name);
    const client=await stream(engine.base,'topics=frame,state&max_rate_hz=10');
    if (name==='sim-full-body'||name==='sim-six-trackers') {
      assert.equal((await engine.api('/simulation','PUT',{motion:'reference_pose'})).status,200);
      await client.until(m=>m.some(m=>m.type==='state'&&m.state.simulation?.motion==='reference_pose'));
      // Allow one device period for control adoption before the existing capture API.
      await new Promise(resolve=>setTimeout(resolve,100));
      assert.equal((await engine.api('/calibration/reference-pose','POST',{})).status,200);
    }
    const state=await client.until(m=>m.findLast(m=>m.type==='state'&&m.state.capability_report?.profile===profile));
    assert.match(state.state.capability_report.tracking_config_id,/^[a-f0-9]{16}$/);
    assert.match(state.state.capability_report.calibration_set_id,/^[a-f0-9]{16}$/);
    client.ws.close();await stop(engine.child);
    console.log(`PASS ${name}: SystemState capability profile ${profile}`);
  }
} finally { await cleanup(); }
