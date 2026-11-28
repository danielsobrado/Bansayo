/** Evaluate explicit engineering gates, never infer physical measurement accuracy. */
export function evaluateHardware(benchmark,journal,options){
  const checks=[];
  const check=(name,pass,detail)=>checks.push({name,pass:!!pass,detail});
  const rate=options.rate,magRate=options.magRate,minRate=options.minRateRatio,maxLoss=options.maxGapRatio,maxFpsLoss=options.maxFpsRegression;
  const valid=v=>typeof v==='number'&&Number.isFinite(v);
  const devices=benchmark.devices??[],window=journal.window,raw=benchmark.recording_status;
  check('physical sources',devices.length>0&&devices.every(d=>d.origin==='live'&&d.kind==='imu_tracker'),devices.map(d=>({id:d.id,origin:d.origin})));
  check('complete journal',journal.complete===true&&['Stopped by operator','Duration completed'].includes(journal.footer?.reason),journal.footer?.reason??null);
  check('same engine and recording',typeof benchmark.session_epoch_utc==='string'&&benchmark.session_epoch_utc===journal.header?.session_epoch_utc&&raw?.observations===journal.observations&&raw?.lost===journal.journal_lost&&raw?.window_start_us===window?.start_us&&raw?.window_end_us===window?.end_us,{benchmark:raw?.file,epoch:benchmark.session_epoch_utc});
  check('common window',valid(window?.duration_s)&&window.duration_s>=options.minSeconds,window?.duration_s??null);
  check('journal overrun',journal.journal_lost===0,journal.journal_lost);
  const wanted=[['orientation',rate],['gyro',rate],['accel',rate],...(magRate>0?[['mag0',magRate],['mag1',magRate]]:[])];
  for(const device of devices){
    const d=benchmark.recording?.devices?.find(x=>x.id===device.id);
    check(device.id+' clock settled',d?.time_sync?.settled===true,d?.time_sync??null);
    for(const [channel,expected] of wanted){
      const c=window?.channels?.find(x=>x.device===device.id&&x.channel===channel);
      const hz=c?.window_rate_hz;
      const gaps=c?.sequence_gaps;
      const ratio=c&&valid(gaps)?gaps/Math.max(1,c.count+gaps):null;
      check(device.id+'/'+channel+' rate',valid(hz)&&hz>=expected*minRate&&hz<=expected/minRate,{actual_hz:hz??null,expected_hz:expected,min_ratio:minRate});
      check(device.id+'/'+channel+' continuity',valid(ratio)&&ratio<=maxLoss&&c.nonpositive_intervals===0,{gap_ratio:ratio,nonpositive_intervals:c?.nonpositive_intervals??null,max_interval_us:c?.max_interval_us??null});
    }
  }
  const before=benchmark.baseline?.received_fps,after=benchmark.recording?.received_fps;
  const fpsLoss=valid(before)&&before>0&&valid(after)?Math.max(0,1-after/before):null;
  check('recording frame throughput',valid(fpsLoss)&&fpsLoss<=maxFpsLoss,{before,after,loss_ratio:fpsLoss});
  check('stage timing evidence',(benchmark.baseline?.diagnostics_windows?.length??0)>0&&(benchmark.recording?.diagnostics_windows?.length??0)>0,{baseline:benchmark.baseline?.diagnostics_windows?.length??0,recording:benchmark.recording?.diagnostics_windows?.length??0});
  return {format:'bansayo-hardware-acceptance-v1',passed:checks.every(c=>c.pass),options,checks,manual_pending:['Physical angle and distance accuracy','Hardware timestamp alignment and motion-to-photon latency','Wi-Fi interruption and recalibration behavior','CPU/GPU utilization and thermal stability','Magnetometer reference field and enclosure effects']};
}
