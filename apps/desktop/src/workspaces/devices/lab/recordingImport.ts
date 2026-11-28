import {MAX_ROWS,parseBatch,continuesBatch,type Capture} from "./captureTypes";
const MAX_FILE=256*1024*1024;
const MAX_LINE=8*1024*1024;

/** Strict, bounded reader for completed engine journals. */
export class JournalReader {
  private result:Capture={samples:[],markers:[],lost:0,resets:0,started:"",reason:"Imported recording"};
  private header=false;
  private footer=false;
  private cursors=new Map<string,{epoch:number;next:number}>();
  private devices=new Set<string>();
  private windowStart:number|undefined;
  private scheduledEnd:number|undefined;
  accept(line:string) {
    if(!line.trim())return;
    const v=JSON.parse(line);
    if(!v||typeof v!=="object"||this.footer)throw Error("Unexpected journal record.");
    if(!this.header){
      if(v.type!=="header"||v.format!=="bansayo-diagnostic-journal-v1"||typeof v.started_utc!=="string"||!Array.isArray(v.devices)||v.devices.length<1||v.devices.length>4||!v.devices.every((x:unknown)=>typeof x==="string"))throw Error("Unsupported journal header.");
      this.devices=new Set(v.devices);
      if(this.devices.size!==v.devices.length)throw Error("Duplicate journal devices.");
      if(v.window_start_us!==undefined||v.scheduled_end_us!==undefined){
        if(!Number.isSafeInteger(v.window_start_us)||v.window_start_us<0||!Number.isSafeInteger(v.scheduled_end_us)||v.scheduled_end_us<v.window_start_us)throw Error("Invalid journal window.");
        this.windowStart=v.window_start_us;this.scheduledEnd=v.scheduled_end_us;
      }
      this.header=true;this.result.started=v.started_utc;return;
    }
    if(v.type==="footer"){
      if(v.observations!==this.result.samples.length||v.lost!==this.result.lost||typeof v.reason!=="string")throw Error("Journal footer counts do not match.");
      if(this.windowStart!==undefined){
        if(!Number.isSafeInteger(v.window_end_us)||v.window_end_us<this.windowStart||v.window_end_us>this.scheduledEnd!)throw Error("Invalid recording end boundary.");
        const start=this.windowStart,end:number=v.window_end_us;
        const count=this.result.samples.length;
        this.result.samples=this.result.samples.filter(s=>s.point.t_us>=start&&s.point.t_us<end);
        this.result.window={start_us:start,end_us:end,excluded:count-this.result.samples.length};
      }
      this.footer=true;this.result.reason="Imported: "+v.reason;
      if(v.reason==="Stream reference reset")this.result.resets++;
      return;
    }
    const b=parseBatch(v.batch);
    if(v.type!=="batch"||typeof v.device!=="string"||!this.devices.has(v.device)||!b||b.reset)throw Error("Invalid journal batch.");
    const prev=this.cursors.get(v.device);
    if(prev&&!continuesBatch(prev,b))throw Error("Journal cursor or reference is inconsistent.");
    if(this.result.samples.length+b.entries.length>MAX_ROWS)throw Error("Recording exceeds browser analysis limit. Use the streaming command-line analyzer.");
    this.cursors.set(v.device,{epoch:b.epoch,next:b.next});
    this.result.lost+=b.lost;
    if(b.lost&&b.entries.length)this.result.markers.push({t_us:b.entries[0]!.point.t_us,device:v.device,label:"Journal overrun: "+b.lost});
    for(const e of b.entries)this.result.samples.push({...e,device:v.device,epoch:b.epoch});
  }
  finish():Capture {
    if(!this.header||!this.footer)throw Error("Incomplete recording: header or footer missing. Use the recovery analyzer for partial files.");
    return this.result;
  }
}
export async function readJournal(file:File):Promise<Capture>{
  if(file.size>MAX_FILE)throw Error("File exceeds supported size.");
  const parser=new JournalReader(),reader=file.stream().pipeThrough(new TextDecoderStream()).getReader();
  let pending="";
  try{
    for(;;){
      const {value,done}=await reader.read();
      if(done)break;
      pending+=value;
      let end=pending.indexOf("\n");
      while(end>=0){if(end>MAX_LINE)throw Error("Journal line too large.");parser.accept(pending.slice(0,end));pending=pending.slice(end+1);end=pending.indexOf("\n");}
      if(pending.length>MAX_LINE)throw Error("Journal line too large.");
    }
    parser.accept(pending);
    return parser.finish();
  }finally{await reader.cancel();reader.releaseLock();}
}
