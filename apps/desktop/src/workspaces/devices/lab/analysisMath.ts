import type { Point } from "./trace";
export interface Uniform {values:number[];hz:number}
/** Latest contiguous run; reject gaps and timing irregularity rather than invent samples. */
export function uniformRun(points:readonly Point[],axis:number):Uniform|null {
  if(points.length<32)return null;
  const recent=points.slice(-32768);
  const intervals=recent.slice(1).map((p,i)=>p.t_us-recent[i]!.t_us).filter(v=>v>0).sort((a,b)=>a-b);
  const dt=intervals[Math.floor(intervals.length/2)];
  if(!dt||dt<=0)return null;
  let start=recent.length-1;
  while(start>0&&Math.abs(recent[start]!.t_us-recent[start-1]!.t_us-dt)<=dt*.2)start--;
  const run=recent.slice(start);
  if(run.length<32||run.some(p=>!Number.isFinite(p.values[axis])))return null;
  return {values:run.map(p=>p.values[axis]!),hz:1e6/dt};
}
function transform(run:Uniform) {
  const n=2**Math.floor(Math.log2(Math.min(run.values.length,4096)));
  if(n<32||!Number.isFinite(run.hz)||run.hz<=0)return null;
  const values=run.values.slice(-n),mean=values.reduce((s,v)=>s+v,0)/n;
  const real=new Float64Array(n),imag=new Float64Array(n);
  let gain=0,energy=0;
  for(let i=0;i<n;i++){const w=.5-.5*Math.cos(2*Math.PI*i/(n-1));real[i]=(values[i]!-mean)*w;gain+=w;energy+=w*w;}
  for(let i=1,j=0;i<n;i++){let bit=n>>1;while(j&bit){j^=bit;bit>>=1;}j^=bit;if(i<j){const v=real[i]!;real[i]=real[j]!;real[j]=v;}}
  for(let size=2;size<=n;size*=2)for(let k=0;k<n;k+=size)for(let j=0;j<size/2;j++){
    const angle=-2*Math.PI*j/size,c=Math.cos(angle),s=Math.sin(angle),a=k+j,b=a+size/2;
    const r=real[b]!*c-imag[b]!*s,q=real[b]!*s+imag[b]!*c;
    real[b]=real[a]!-r;imag[b]=imag[a]!-q;real[a]=real[a]!+r;imag[a]=imag[a]!+q;
  }
  return {n,real,imag,gain,energy};
}
/** One-sided Hann amplitude spectrum and periodogram density (input units squared per Hz). */
export function spectralAnalysis(run:Uniform) {
  const t=transform(run);
  if(!t)return {amplitude:[],density:[],resolutionHz:0};
  const {n,real,imag,gain,energy}=t;
  const resolutionHz=run.hz/n;
  const amplitude:{x:number;y:number}[]=[],density:{x:number;y:number}[]=[];
  for(let i=0;i<=n/2;i++){
    const power=real[i]!**2+imag[i]!**2;
    const factor=i===0||i===n/2?1:2;
    const x=i*resolutionHz;
    amplitude.push({x,y:Math.sqrt(power)*factor/gain});
    density.push({x,y:power*factor/(run.hz*energy)});
  }
  return {amplitude,density,resolutionHz};
}
export function spectrum(run:Uniform){return spectralAnalysis(run).amplitude;}
export function allan(run:Uniform) {
  const n=run.values.length,prefix=new Float64Array(n+1);
  for(let i=0;i<n;i++)prefix[i+1]=prefix[i]!+run.values[i]!;
  const out:{x:number;y:number}[]=[];
  for(let m=1;m<=n/10;m*=2){let sum=0,count=0;
    for(let i=0;i+2*m<=n;i++){const d=(prefix[i+2*m]!-2*prefix[i+m]!+prefix[i]!)/m;sum+=d*d;count++;}
    if(count)out.push({x:m/run.hz,y:Math.sqrt(sum/(2*count))});
  }
  return out;
}
