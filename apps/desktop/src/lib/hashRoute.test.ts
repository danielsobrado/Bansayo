import {it,expect} from "vitest";
import {parseHash} from "./hashRoute";
it("keeps subsegments and ignores the query",()=>{
  expect(parseHash("#/studio/session/7?inspect=1",["studio"],"studio")).toEqual({workspace:"studio",rest:["session","7"]});
  expect(parseHash("#/unknown/anything",["studio"],"studio")).toEqual({workspace:"studio",rest:[]});
  expect(parseHash("",["studio"],"studio")).toEqual({workspace:"studio",rest:[]});
});
