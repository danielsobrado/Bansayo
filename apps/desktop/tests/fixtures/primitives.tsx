/** Browser-only fixture; never imported by the application. */
import { StrictMode, useState } from "react";
import { createRoot } from "react-dom/client";
import { Button } from "../../src/ui/Button";
import { Field } from "../../src/ui/Field";
import { Select } from "../../src/ui/Select";
import { Dialog } from "../../src/ui/Dialog";
import { Drawer } from "../../src/ui/Drawer";
import { Readout } from "../../src/ui/Readout";
function Fixture({ initiallyOpen }: {initiallyOpen: boolean}) {
  const [dialog,setDialog]=useState(initiallyOpen);
  const [drawer,setDrawer]=useState(false);
  return <div style={{position:"fixed",inset:0,zIndex:1000,padding:24,background:"var(--surface-0)"}}>
    <Button onClick={()=>setDialog(true)}>Open dialog</Button>
    <Button onClick={()=>setDrawer(true)}>Open drawer</Button>
    <Button disabled>Disabled action</Button>
    <Readout label="Knee angle" value={90} unit="°" />
    <Dialog open={dialog} title="Capture details" description="Set the test capture." onClose={()=>setDialog(false)}>
      <Field label="Height" hint="Metres" defaultValue="1.75" />
      <Select label="Side" options={[{value:"left",label:"Left"},{value:"right",label:"Right"}]} />
    </Dialog>
    <Drawer open={drawer} title="Input details" onClose={()=>setDrawer(false)}><Field label="Device name" /></Drawer>
  </div>;
}
export function mount(initiallyOpen = false) {
  const target=document.createElement("div");document.body.append(target);
  const root=createRoot(target);root.render(<StrictMode><Fixture initiallyOpen={initiallyOpen}/></StrictMode>);
}
