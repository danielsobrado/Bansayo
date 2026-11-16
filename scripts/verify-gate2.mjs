/** Real-process assignments, faults, MCAP, playback and shutdown checks. */
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { command, scratch, start, stop, stream, cleanup } from './engine-test-harness.mjs';

const results = [];
const passed = message => { results.push(message); console.log(`PASS ${message}`); };
const ok = response => { assert.equal(response.status, 200, JSON.stringify(response.body)); return response.body.data; };
const device = (state, id) => state.devices.find(d => d.descriptor.id === id);

try {
  await command(['build', '-p', 'kinexa-engine']);
  let engine = await start('sim-full-body');
  let live = await stream(engine.base, 'topics=frame,state,landmarks_2d&max_rate_hz=60');
  const skeleton = live.messages[0].skeleton;
  const shinIndex = skeleton.segments.findIndex(s => s.id === 'left_shin');
  const kneeIndex = skeleton.joints.findIndex(j => j.id === 'left_knee_flexion');
  ok(await engine.api('/simulation', 'PUT', { motion: 'reference_pose' }));
  ok(await engine.api('/calibration/reference-pose', 'POST', { device_ids: null }));

  const assigned = ok(await engine.api('/devices/SIM-T04/assignment', 'PUT', { segment: 'head' }));
  assert.equal(assigned.assignment, 'head');
  let state = ok(await engine.api('/state'));
  assert(state.calibration.some(p => p.device_id === 'SIM-T04' && p.segment_id === 'left_shin' && p.state === 'stale'));
  live.ws.close();
  await stop(engine.child);
  engine = await start('sim-full-body');
  state = ok(await engine.api('/state'));
  assert.equal(device(state, 'SIM-T04').assignment, 'head');
  assert(state.calibration.some(p => p.device_id === 'SIM-T04' && p.state === 'stale'));
  const back = ok(await engine.api('/devices/SIM-T04/assignment', 'PUT', { segment: 'left_shin' }));
  assert(back.calibration.some(p => p.kind === 'mounting' && p.state === 'stale'));
  live = await stream(engine.base, 'topics=frame,state,landmarks_2d&max_rate_hz=60');
  await live.until(list => list.find(m => m.type === 'frame' && m.pose?.segments[shinIndex].provenance === 'inferred'));
  ok(await engine.api('/simulation', 'PUT', { motion: 'reference_pose' }));
  ok(await engine.api('/calibration/reference-pose', 'POST', { device_ids: ['SIM-T04'] }));
  passed('assignments and stale calibration persist; assigning back requires recapture');

  let cutoff = live.messages.filter(m => m.type === 'frame').at(-1)?.seq ?? -1;
  const disconnected = ok(await engine.api('/simulation/devices/SIM-T04/fault', 'POST', { action: 'disconnect' }));
  assert.equal(disconnected.state, 'disconnected');
  const gap = await live.until(list => list.find(m => m.type === 'frame' && m.seq > cutoff && m.pose?.segments[shinIndex].provenance === 'inferred'));
  assert.equal(gap.joints[kneeIndex].value, null);
  assert(gap.joints[kneeIndex].reasons.length > 0);
  cutoff = gap.seq;
  ok(await engine.api('/simulation/devices/SIM-T04/fault', 'POST', { action: 'reconnect' }));
  await live.until(list => list.find(m => m.type === 'frame' && m.seq > cutoff && m.pose?.segments[shinIndex].provenance === 'derived'));
  const degraded = ok(await engine.api('/simulation/devices/SIM-T03/fault', 'POST', { action: 'degrade', quality: 0.2 }));
  assert.equal(degraded.state, 'degraded');
  ok(await engine.api('/simulation/devices/SIM-T03/fault', 'POST', { action: 'restore' }));
  passed('disconnect clears dependent joint values; reconnect retains calibration; quality faults report degraded');

  ok(await engine.api('/simulation', 'PUT', { motion: 'front_kick' }));
  const active = ok(await engine.api('/recording/start', 'POST', { name: 'Gate 2 front kick' }));
  assert.equal((await engine.api('/recording/start', 'POST', {})).status, 409);
  assert.equal((await engine.api('/playback/start', 'POST', { session_id: active.session_id })).status, 409);
  await live.until(list => list.find(m => m.type === 'frame' && m.t_us > active.started_t_us + 10_000_000), 13_000);
  const session = ok(await engine.api('/recording/stop', 'POST', {}));
  assert(session.duration_us >= 10_000_000);
  assert(session.size_bytes > 1000);
  const file = await readFile(join(scratch, 'sim-full-body/sessions', session.id, 'recording.mcap'));
  const magic = Buffer.from([137, 77, 67, 65, 80, 48, 13, 10]);
  assert.deepEqual(file.subarray(0, 8), magic);
  assert.deepEqual(file.subarray(-8), magic);
  assert(ok(await engine.api('/sessions')).some(s => s.id === session.id && s.name === 'Gate 2 front kick'));
  passed('ten seconds of input recording finalise to indexed MCAP; recording/playback conflicts are refused');

  const snapshots = [];
  let globalSeq = live.messages.filter(m => m.type === 'frame').at(-1).seq;
  for (let pass = 0; pass < 2; pass++) {
    ok(await engine.api('/playback/start', 'POST', { session_id: session.id, speed: 1, looping: false }));
    assert.equal((await engine.api('/recording/start', 'POST', {})).status, 409);
    assert.equal((await engine.api('/calibration/reference-pose', 'POST', { device_ids: null })).status, 409);
    ok(await engine.api('/playback', 'PUT', { paused: true }));
    const poses = [];
    for (const position of [1_000_000, 4_000_000, 8_000_000]) {
      const before = live.messages.filter(m => m.type === 'frame').at(-1)?.seq ?? globalSeq;
      ok(await engine.api('/playback', 'PUT', { paused: true, position_us: position }));
      const target = active.started_t_us + position;
      const frame = await live.until(list => list.find(m => m.type === 'frame' && m.seq > before && Math.abs(m.t_us - target) < 17_000));
      assert(frame.seq > globalSeq);
      globalSeq = frame.seq;
      poses.push({ t_us: frame.t_us, pose: frame.pose, joints: frame.joints, quality: frame.quality });
    }
    snapshots.push(poses);
    ok(await engine.api('/playback/stop', 'POST', {}));
    assert.equal(ok(await engine.api('/state')).engine.mode, 'live');
    assert.equal(device(ok(await engine.api('/state')), 'SIM-T04').assignment, 'left_shin');
  }
  assert.deepEqual(snapshots[0], snapshots[1]);
  assert.equal((await engine.api('/playback/start', 'POST', { session_id: '../outside' })).status, 400);
  passed('two replay passes reproduce fixed virtual ticks; scrubbing, conflicts, mode restoration and IDs are correct');

  const shutdownRecording = ok(await engine.api('/recording/start', 'POST', { name: 'Graceful shutdown' }));
  await live.until(list => list.find(m => m.type === 'frame' && m.t_us > shutdownRecording.started_t_us + 500_000));
  // Leave the WebSocket open: shutdown must close it and finalise the recorder.
  await stop(engine.child);
  live.ws.close();
  engine = await start('sim-full-body');
  assert(ok(await engine.api('/sessions')).some(s => s.id === shutdownRecording.session_id));
  await stop(engine.child);
  passed('Ctrl-C with recording and a connected stream leaves a readable indexed session');
  console.log(`\nGate 2: ${results.length} real-process checks passed.`);
} finally {
  await cleanup();
}
