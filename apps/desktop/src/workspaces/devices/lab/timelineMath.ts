import type {Sample} from "./captureTypes";

/** First sample at or after the target in a timestamp-ordered channel. */
export function lowerTimeBound(points:readonly Sample[],target:number):number {
  let lo=0,hi=points.length;
  while(lo<hi){
    const mid=lo+Math.floor((hi-lo)/2);
    if(points[mid]!.point.t_us<target)lo=mid+1;
    else hi=mid;
  }
  return lo;
}
