import {describe,it,expect} from "vitest";
import {completeCapture,type Capture} from "./captureTypes";
const capture:Capture={samples:[],markers:[],lost:0,resets:0,started:"fixture",reason:"Duration completed",window:{start_us:0,end_us:10000,excluded:0}};
describe("capture acceptance integrity",()=>{
  it("rejects partial, lost, reset and zero-duration captures",()=>{
    expect(completeCapture(capture)).toBe(true);
    expect(completeCapture({...capture,reason:"Imported: Duration completed"})).toBe(true);
    expect(completeCapture({...capture,reason:"Browser capture capacity reached"})).toBe(false);
    expect(completeCapture({...capture,reason:"Duration completed (journal backlog may remain)"})).toBe(false);
    expect(completeCapture({...capture,lost:1})).toBe(false);
    expect(completeCapture({...capture,resets:1})).toBe(false);
    expect(completeCapture({...capture,window:{start_us:0,end_us:0,excluded:0}})).toBe(false);
  });
});
