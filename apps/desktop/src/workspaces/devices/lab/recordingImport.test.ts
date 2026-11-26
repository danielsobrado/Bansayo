import {describe,it,expect} from "vitest";
import {JournalReader} from "./recordingImport";
const header={type:"header",format:"bansayo-diagnostic-journal-v1",started_utc:"2026-01-01T00:00:00Z",devices:["A"]};
const batch={type:"batch",device:"A",batch:{epoch:1,next:1,lost:0,reset:false,entries:[{cursor:1,channel:"gyro",point:{t_us:100,seq:1,device_time_us:null,values:[0,0,1]}}]}};
describe("persistent recording import",()=>{
  it("validates a finished journal",()=>{const p=new JournalReader();for(const v of [header,batch,{type:"footer",reason:"Stopped",observations:1,lost:0}])p.accept(JSON.stringify(v));expect(p.finish().samples).toHaveLength(1);});
  it("rejects missing footer and duplicate cursor",()=>{const p=new JournalReader();p.accept(JSON.stringify(header));p.accept(JSON.stringify(batch));expect(()=>p.finish()).toThrow();expect(()=>p.accept(JSON.stringify(batch))).toThrow();});
});
