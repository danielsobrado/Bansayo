const CHANNEL_AXES={orientation:4,gyro:3,accel:3,position:3,pose_rotation:4,pose_position:3,distance:1,contact:1,mag0:3,mag1:3};
const integer=value=>Number.isSafeInteger(value)&&value>=0;
export function validHeader(row){
  return row?.type==='header'&&row.format==='bansayo-diagnostic-journal-v1'&&
    typeof row.started_utc==='string'&&Array.isArray(row.devices)&&row.devices.length>=1&&row.devices.length<=4&&
    row.devices.every(id=>typeof id==='string'&&id.length>0)&&new Set(row.devices).size===row.devices.length;
}
export function validateBatch(row,devices,prior){
  const b=row?.batch;
  if(row?.type!=='batch'||!devices.includes(row.device)||!b||!integer(b.epoch)||b.epoch===0||
    !integer(b.next)||!integer(b.lost)||b.reset!==false||!Array.isArray(b.entries)||b.entries.length>4096)return 'Invalid batch';
  if(prior&&(prior.epoch!==b.epoch||b.next!==prior.next+b.lost+b.entries.length))return 'Inconsistent reference or cursor';
  let previous=prior?prior.next+b.lost:null;
  for(const entry of b.entries){
    const point=entry?.point;
    if(!entry||!integer(entry.cursor)||entry.cursor===0||entry.cursor>b.next||(previous!==null&&entry.cursor!==previous+1)||
      !Object.hasOwn(CHANNEL_AXES,entry.channel)||!point||!integer(point.t_us)||!integer(point.seq)||
      !(point.device_time_us===null||integer(point.device_time_us))||!Array.isArray(point.values)||
      point.values.length!==CHANNEL_AXES[entry.channel]||!point.values.every(Number.isFinite))return 'Invalid observation or cursor';
    previous=entry.cursor;
  }
  if(b.entries.length&&previous!==b.next)return 'Batch cursor does not match last entry';
  return null;
}
