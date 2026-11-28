/** Streaming JSONL validation and per-channel timing/sequence statistics. Node 22+. */
import { createReadStream } from 'node:fs';
import { createInterface } from 'node:readline';
import { writeFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
import {validHeader,validateBatch} from './diagnostic-journal-validation.mjs';
const [file,output]=process.argv.slice(2);
assert.ok(file&&output,'Usage: node scripts/analyze-diagnostic-recording.mjs INPUT.jsonl REPORT.json');
const channels=new Map(),streams=new Map();
let header=null,footer=null,rows=0,lost=0,lineNumber=0,parseError=null;
for await(const line of createInterface({input:createReadStream(file),crlfDelay:Infinity})){
  lineNumber++;if(!line.trim())continue;
  let row;try{row=JSON.parse(line);}catch{parseError='Invalid JSON at line '+lineNumber;break;}
  if(footer){parseError='Unexpected rows after footer';break;}
  if(!header){
    if(!validHeader(row)){parseError='Missing or invalid header';break;}
    header=row;continue;
  }
  if(row?.type==='footer'){
    if(typeof row.reason!=='string'||!Number.isSafeInteger(row.observations)||!Number.isSafeInteger(row.lost)){parseError='Invalid footer';break;}
    footer=row;continue;
  }
  const invalid=validateBatch(row,header.devices,streams.get(row?.device));
  if(invalid){parseError=invalid+' at line '+lineNumber;break;}
  const b=row.batch;
  streams.set(row.device,{epoch:b.epoch,next:b.next});
  lost+=b.lost;
  for(const e of b.entries){
    const p=e.point,key=row.device+'/'+e.channel;
    let s=channels.get(key);
    if(!s){s={device:row.device,channel:e.channel,count:0,first_us:p.t_us,last_us:p.t_us,last_seq:null,sequence_gaps:0,nonpositive_intervals:0,interval_sum:0,interval_count:0,min_interval_us:Infinity,max_interval_us:0};channels.set(key,s);}
    if(s.count){
      const dt=p.t_us-s.last_us;
      if(dt<=0)s.nonpositive_intervals++;else{s.interval_sum+=dt;s.interval_count++;s.min_interval_us=Math.min(s.min_interval_us,dt);s.max_interval_us=Math.max(s.max_interval_us,dt);}
      if(p.seq>s.last_seq)s.sequence_gaps+=Math.max(0,p.seq-s.last_seq-1);
    }
    s.count++;s.last_us=p.t_us;s.last_seq=p.seq;rows++;
  }
  if(parseError)break;
}
if(!parseError&&footer&&(footer.observations!==rows||footer.lost!==lost))parseError='Footer counts do not match';
let window=null;
if(!parseError&&header?.window_start_us!==undefined&&footer){
  const start=header.window_start_us,end=footer.window_end_us,scheduled=header.scheduled_end_us;
  if(!Number.isSafeInteger(start)||start<0||!Number.isSafeInteger(end)||!Number.isSafeInteger(scheduled)||end<start||end>scheduled)parseError='Invalid recording window';
  else{
    const counts=new Map();let excluded=0;
    for await(const line of createInterface({input:createReadStream(file),crlfDelay:Infinity})){
      if(!line.trim())continue;
      const row=JSON.parse(line);
      if(row.type!=='batch')continue;
      for(const e of row.batch.entries){
        if(e.point.t_us<start||e.point.t_us>=end){excluded++;continue;}
        const key=row.device+'/'+e.channel;
        const current=counts.get(key)??{device:row.device,channel:e.channel,count:0};
        current.count++;counts.set(key,current);
      }
    }
    window={start_us:start,end_us:end,duration_s:(end-start)/1e6,excluded,channels:[...counts.values()]};
  }
}
const complete=!!header&&!!footer&&!parseError;
const report={file,complete,header,footer,parse_error:parseError,observations:rows,journal_lost:lost,window,channels:[...channels.values()].map(({interval_sum,interval_count,last_seq,...s})=>({...s,min_interval_us:Number.isFinite(s.min_interval_us)?s.min_interval_us:null,mean_interval_us:interval_count?interval_sum/interval_count:null,observed_rate_hz:s.last_us>s.first_us?(s.count-1)*1e6/(s.last_us-s.first_us):null}))};
await writeFile(output,JSON.stringify(report,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({complete,observations:rows,journal_lost:lost,output}));
if(!complete)process.exitCode=1;
