export const STATIONARY_DEFAULTS={
  minSeconds:30,minRateHz:90,maxGyroPeak:0.12,maxGravityError:0.8,
  maxGyroStd:0.02,maxAccelStd:0.15,maxGyroBias:0.05,maxOrientationDriftDeg:2,
} as const;
export type StationaryLimits={ [K in keyof typeof STATIONARY_DEFAULTS]:number };
export const STATIONARY_KEYS=Object.keys(STATIONARY_DEFAULTS) as (keyof StationaryLimits)[];
export const STATIONARY_LABELS:Record<keyof StationaryLimits,string>={
  minSeconds:"Minimum duration (s)",minRateHz:"Minimum channel rate (Hz)",
  maxGyroPeak:"Maximum gyro magnitude (rad/s)",maxGravityError:"Maximum gravity magnitude error (m/s²)",
  maxGyroStd:"Maximum gyro axis std dev (rad/s)",maxAccelStd:"Maximum accel axis std dev (m/s²)",
  maxGyroBias:"Maximum gyro mean magnitude (rad/s)",maxOrientationDriftDeg:"Maximum orientation excursion (°)",
};
const KEY="bansayo.studio.stationary-limits.v1";
export function validLimits(value:unknown):value is StationaryLimits{
  if(!value||typeof value!=="object")return false;
  const v=value as StationaryLimits;
  return STATIONARY_KEYS.every(k=>Number.isFinite(v[k])&&v[k]>0)&&v.minSeconds<=3600&&v.minRateHz<=10000;
}
export function loadLimits():StationaryLimits{
  try{const v:unknown=JSON.parse(localStorage.getItem(KEY)??"null");if(validLimits(v))return Object.fromEntries(STATIONARY_KEYS.map(k=>[k,v[k]])) as StationaryLimits;}catch{/* storage unavailable */}
  return {...STATIONARY_DEFAULTS};
}
export function saveLimits(v:StationaryLimits){try{localStorage.setItem(KEY,JSON.stringify(v));}catch{/* storage unavailable */}}
