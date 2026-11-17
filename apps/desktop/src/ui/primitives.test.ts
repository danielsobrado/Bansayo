import { createElement as h } from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { Button } from "./Button";
import { Field } from "./Field";
import { Select } from "./Select";
import { Dialog } from "./Dialog";
import { Drawer } from "./Drawer";
import { Readout } from "./Readout";
describe("shared UI primitives",()=>{
  it("uses native button semantics and preserves disabled state",()=>{
    expect(renderToStaticMarkup(h(Button,{disabled:true},"Capture"))).toMatch(/type="button".*disabled/);
  });
  it("associates labels, help and errors with native form controls",()=>{
    const field=renderToStaticMarkup(h(Field,{id:"height",label:"Height",hint:"Metres",error:"Enter a height"}));
    expect(field).toContain('for="height"');expect(field).toContain('aria-invalid="true"');expect(field).toContain('aria-describedby="height-hint height-error"');
    const select=renderToStaticMarkup(h(Select,{id:"side",label:"Side",options:[{value:"left",label:"Left"}]}));
    expect(select).toContain('for="side"');expect(select).toContain('<option value="left">Left</option>');
  });
  it("labels modal surfaces and keeps numeric formatting in presentation",()=>{
    expect(renderToStaticMarkup(h(Dialog,{open:false,title:"Capture",onClose:()=>{},children:"Body"}))).toContain('aria-labelledby=');
    expect(renderToStaticMarkup(h(Drawer,{open:false,title:"Details",onClose:()=>{},children:"Body"}))).toContain('<dialog');
    expect(renderToStaticMarkup(h(Readout,{label:"Angle",value:null,unit:"°"}))).toContain("—");
    expect(renderToStaticMarkup(h(Readout,{label:"Angle",value:Number.NaN}))).toContain("—");
  });
});
