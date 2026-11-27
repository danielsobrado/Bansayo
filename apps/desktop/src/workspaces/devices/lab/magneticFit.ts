type Vec=number[];
function solve(a:number[][],b:number[]):number[]|null {
  const n=b.length,m=a.map((row,i)=>[...row,b[i]!]);
  for(let k=0;k<n;k++){
    let pivot=k;for(let i=k+1;i<n;i++)if(Math.abs(m[i]![k]!)>Math.abs(m[pivot]![k]!))pivot=i;
    if(Math.abs(m[pivot]![k]!)<1e-9)return null;
    [m[k],m[pivot]]=[m[pivot]!,m[k]!];
    const d=m[k]![k]!;for(let j=k;j<=n;j++)m[k]![j]!/=d;
    for(let i=0;i<n;i++)if(i!==k){const v=m[i]![k]!;for(let j=k;j<=n;j++)m[i]![j]!-=v*m[k]![j]!;}
  }
  return m.map(row=>row[n]!);
}
const dot=(a:Vec,b:Vec)=>a.reduce((s,v,i)=>s+v*b[i]!,0);
/** Fit x'Ax+b'x=1 in centered/scaled coordinates, then Cholesky sphere transform. */
export function fitMagnetic(input:readonly Vec[]) {
  const stride=Math.max(1,Math.ceil(input.length/4096));
  const usable=input.filter((p,i)=>i%stride===0&&p.length===3&&p.every(Number.isFinite));
  if(usable.length<75)return null;
  const pts=usable.filter((_,i)=>i%5!==0),validation=usable.filter((_,i)=>i%5===0);
  const mean=[0,1,2].map(i=>pts.reduce((s,p)=>s+p[i]!,0)/pts.length);
  const scale=Math.sqrt(pts.reduce((s,p)=>s+p.reduce((v,x,i)=>v+(x-mean[i]!)**2,0),0)/pts.length);
  if(scale<1e-9)return null;
  const normal=Array.from({length:9},()=>Array<number>(9).fill(0)),rhs=Array<number>(9).fill(0);
  const normalized=pts.map(p=>p.map((v,i)=>(v-mean[i]!)/scale));
  for(const p of normalized){const [x,y,z]=p as [number,number,number];const row=[x*x,y*y,z*z,2*x*y,2*x*z,2*y*z,x,y,z];for(let i=0;i<9;i++){rhs[i]!+=row[i]!;for(let j=0;j<9;j++)normal[i]![j]!+=row[i]!*row[j]!;}}
  const v=solve(normal,rhs);if(!v)return null;
  const a=[[v[0]!,v[3]!,v[4]!],[v[3]!,v[1]!,v[5]!],[v[4]!,v[5]!,v[2]!]];
  const c=solve(a,v.slice(6).map(x=>-x/2));if(!c)return null;
  const k=1+dot(c,a.map(row=>dot(row,c)));if(k<=0)return null;
  const l=Array.from({length:3},()=>[0,0,0]);
  for(let i=0;i<3;i++)for(let j=0;j<=i;j++){let x=a[i]![j]!/k;for(let t=0;t<j;t++)x-=l[i]![t]!*l[j]![t]!;if(i===j){if(x<1e-6)return null;l[i]![j]=Math.sqrt(x);}else l[i]![j]=x/l[j]![j]!;}
  const matrix=[0,1,2].map(i=>[0,1,2].map(j=>l[j]![i]!/scale));
  const center=c.map((x,i)=>x*scale+mean[i]!);
  const corrected=pts.map(p=>matrix.map(row=>dot(row,p.map((x,i)=>x-center[i]!))));
  const octants=new Set(corrected.map(p=>p.reduce((n,x,i)=>n|(x>=0?1<<i:0),0)));
  const rms=Math.sqrt(corrected.reduce((s,p)=>s+(Math.hypot(...p)-1)**2,0)/pts.length);
  if(octants.size<8||!Number.isFinite(rms)||rms>.25)return null;
  const validationErrors=validation.map(p=>Math.abs(Math.hypot(...matrix.map(row=>dot(row,p.map((x,i)=>x-center[i]!))))-1));
  const validationRms=Math.sqrt(validationErrors.reduce((s,v)=>s+v*v,0)/validationErrors.length);
  const validationMax=Math.max(...validationErrors);
  if(!Number.isFinite(validationRms)||!Number.isFinite(validationMax)||validationRms>.25)return null;
  return {center,matrix,rms,count:pts.length,validationRms,validationMax,validationCount:validation.length,corrected:corrected.filter((_,i)=>i%Math.max(1,Math.ceil(pts.length/600))===0)};
}
