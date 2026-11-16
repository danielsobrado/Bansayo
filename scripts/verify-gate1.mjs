/** Independent, real-process Gate 1 checks. Node 22+; no npm dependencies. */
import assert from 'node:assert/strict';
import { command, start, stop, stream, cleanup } from './engine-test-harness.mjs';
const results = [];

function passed(name) {
  results.push(name);
  console.log(`PASS ${name}`);
}

try {
  await command(['build', '-p', 'kinexa-engine']);
  const full = await start('sim-full-body');
  const live = await stream(full.base);
  const hello = live.messages[0];
  assert.equal(hello.conventions.quaternion_order, 'xyzw');
  assert.equal(hello.skeleton.model_id, 'kinexa.body.v1');
  const initial = await live.until(list => list.find(m => m.type === 'frame' && m.pose));
  assert(initial.pose.segments.every(segment => segment.provenance === 'inferred'));
  passed('fresh full-body stream starts with hello and inferred segments');

  const foreign = await full.api('/simulation', 'PUT', { motion: 'squat' }, { Origin: 'https://foreign.example' });
  assert.equal(foreign.status, 403);
  assert.equal(foreign.body.error.code, 'forbidden_origin');
  const local = await full.api('/state', 'GET', undefined, { Origin: 'http://localhost:5173' });
  assert.equal(local.headers.get('access-control-allow-origin'), 'http://localhost:5173');
  const malformed = await fetch(`${full.base}/simulation`, { method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: '{' });
  assert.equal(malformed.status, 400);
  assert.equal((await malformed.json()).error.code, 'invalid_request');
  const badMotion = await full.api('/simulation', 'PUT', { motion: 'unknown-motion' });
  assert.equal(badMotion.status, 400);
  passed('origin allow-list, browser CORS, malformed body and invalid motion are enforced');

  assert.equal((await full.api('/simulation', 'PUT', { motion: 'reference_pose', paused: false, speed: 1 })).status, 200);
  const capture = await full.api('/calibration/reference-pose', 'POST', { device_ids: null });
  assert.equal(capture.status, 200, JSON.stringify(capture.body));
  const trackerCount = hello.state.devices.filter(device => device.descriptor.kind === 'imu_tracker' && device.assignment).length;
  assert.equal(capture.body.data.length, trackerCount);
  const calibrated = await live.until(list => list.find(m => m.type === 'frame' && m.pose && m.pose.segments.filter(s => s.provenance === 'derived').length === trackerCount));
  assert(calibrated.quality.observed_segments === trackerCount);
  passed('reference-pose API capture derives every assigned tracker');

  const lastSeq = live.messages.filter(m => m.type === 'frame').at(-1).seq;
  assert.equal((await full.api('/simulation', 'PUT', { motion: 'squat' })).status, 200);
  await live.until(list => {
    const values = list.filter(m => m.type === 'frame' && m.seq > lastSeq).map(m => m.joints[0]?.value).filter(v => typeof v === 'number');
    return values.length > 20 && Math.max(...values) - Math.min(...values) > 0.5;
  });
  const frames = live.messages.filter(m => m.type === 'frame');
  for (let index = 1; index < frames.length; index++) {
    assert(frames[index].seq > frames[index - 1].seq);
    assert(frames[index].t_us > frames[index - 1].t_us);
  }
  const observedRate = (frames.length - 1) * 1e6 / (frames.at(-1).t_us - frames[0].t_us);
  assert(observedRate <= 21, `requested cap 20 Hz, observed ${observedRate}`);
  passed('squat changes live joint values; frame sequence/time increase and 20 Hz cap holds');

  await full.api('/simulation', 'PUT', { motion: 'front_kick' });
  const moving = await full.api('/calibration/reference-pose', 'POST', { device_ids: null });
  assert.equal(moving.body.ok, false, 'moving capture must fail');
  assert.match(JSON.stringify(moving.body).toLowerCase(), /movement/);
  live.ws.close();
  await stop(full.child);
  const restarted = await start('sim-full-body');
  const restored = await restarted.api('/state');
  assert.equal(restored.body.data.calibration.filter(profile => profile.kind === 'mounting' && profile.state === 'valid').length, trackerCount);
  await stop(restarted.child);
  passed('moving capture explains failure; mounting profiles survive engine restart');

  for (const name of ['sim-six-trackers', 'sim-camera-only', 'empty']) {
    const engine = await start(name);
    const client = await stream(engine.base);
    const frame = await client.until(list => list.find(m => m.type === 'frame'));
    if (name === 'sim-camera-only') {
      assert.equal(frame.pose, null);
      const landmarks = await client.until(list => list.find(m => m.type === 'landmarks_2d'));
      assert(landmarks.landmarks.points.some(point => point !== null));
      const feature = client.messages[0].state.features.find(feature => feature.id === 'skeleton_pose');
      assert.equal(feature.state, 'unavailable');
      assert(feature.reasons.length > 0);
    } else if (name === 'empty') {
      assert.equal(frame.pose, null);
      assert.equal(client.messages[0].state.devices.length, 0);
    } else {
      assert.equal(client.messages[0].state.devices.filter(device => device.descriptor.kind === 'imu_tracker').length, 6);
      assert(frame.pose);
    }
    client.ws.close();
    await stop(engine.child);
    passed(`${name}: healthy API and truthful stream`);
  }
  console.log(`\nGate 1: ${results.length} real-process checks passed.`);
} finally {
  await cleanup();
}
