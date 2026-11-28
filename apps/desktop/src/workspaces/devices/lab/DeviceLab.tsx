import { useEffect, useState, type CSSProperties } from "react";
import type { DeviceStatus, SkeletonDefinition } from "@contracts";
import { Icon } from "../../../ui/Icon";
import { DeviceExplorer } from "./DeviceExplorer";
import { DeviceInspector } from "./DeviceInspector";
import { SolverBench } from "./SolverBench";
import { TrackerComparison } from "./TrackerComparison";
import {useReliability} from "./useReliability";
import {ReliabilityPanel} from "./ReliabilityPanel";
import { StudioEvents, useStudioEvents } from "./StudioEvents";
import { DockShelf } from "./DockShelf";
import { CaptureStudio } from "./CaptureStudio";
import { LayoutControls } from "./LayoutControls";
import { type StudioView, type SignalFocus, loadLayout, saveLayout } from "./studioConfig";
import "./lab.css";
import "./studio.css";

export function DeviceLab({ devices, live, skeleton }: {
  devices: DeviceStatus[]; live: boolean; skeleton: SkeletonDefinition | null;
}) {
  const [layout, setLayout] = useState(loadLayout);
  const [view, setView] = useState<StudioView>("overview");
  const [selected, setSelected] = useState("");
  const [focus, setFocus] = useState<SignalFocus>("all");
  const [comparison, setComparison] = useState<"solver" | "tracker">("solver");
  const events = useStudioEvents(devices,live);
  const reliability=useReliability(devices,live);
  const device = devices.find(d => d.descriptor.id === selected) ?? devices[0];
  useEffect(() => saveLayout(layout), [layout]);
  function select(id: string, component: SignalFocus = "all") {
    setSelected(id);
    setFocus(component);
    setView(component === "all" ? "overview" : "signals");
  }
  return <section className="sensor-studio" aria-label="Sensor Studio" style={{"--studio-explorer-width":layout.explorerWidth+"px","--studio-inspector-width":layout.inspectorWidth+"px"} as CSSProperties}>
    <header className="studio-top">
      <div className="studio-title"><Icon name="sliders" size={20}/><div><h2>Sensor Studio</h2><small>Instrumentation / Diagnostics</small></div></div>
      <nav className="studio-tabs" aria-label="Sensor Studio views">
        {(["overview","signals","calibration","timing","compare","optical","events","capture","reliability"] as const).map(tab =>
          <button key={tab} aria-pressed={view === tab} onClick={() => setView(tab)}>{tab[0]!.toUpperCase()+tab.slice(1)}</button>)}
      </nav>
      <div className="studio-controls">
        <span className="studio-connection" data-live={live}>{live ? "Live engine" : "Offline / replay"}</span>
        <LayoutControls layout={layout} onChange={setLayout}/>
        <button title="Toggle device explorer" aria-label="Toggle device explorer" aria-pressed={layout.explorer} onClick={() => setLayout(l => ({...l, explorer:!l.explorer}))}><Icon name="panel-left"/></button>
        <button title="Toggle inspector" aria-label="Toggle inspector" aria-pressed={layout.inspector} onClick={() => setLayout(l => ({...l, inspector:!l.inspector}))}><Icon name="panel-right"/></button>
      </div>
    </header>
    <div className="studio-body" data-explorer={layout.explorer}>
      {layout.explorer && <DeviceExplorer devices={devices} selected={device?.descriptor.id ?? ""} focus={focus} view={view} onSelect={select}/>}
      <section className="studio-workspace" aria-label="Sensor workspace">
        <DockShelf devices={devices} live={live}><div hidden={view!=="capture"}><CaptureStudio devices={devices} live={live} active={view==="capture"}/></div>
        {view==="capture"?null:view==="reliability"?<ReliabilityPanel devices={devices} {...reliability}/>:view==="events"?<StudioEvents events={events}/>:!device ? <div className="studio-empty"><Icon name="usb" size={28}/><h3>No configured devices</h3><p>Connect or configure a sensor source to begin inspection.</p></div> :
          view === "compare" ? <><div className="studio-subbar"><button aria-pressed={comparison==="solver"} onClick={()=>setComparison("solver")}>Solver comparison</button><button aria-pressed={comparison==="tracker"} onClick={()=>setComparison("tracker")}>Tracker comparison</button></div>
          {comparison==="solver"?<SolverBench devices={devices} live={live}/>:<TrackerComparison devices={devices} live={live}/>}</> :
          <DeviceInspector key={device.descriptor.id + ":" + live} device={device} devices={devices} live={live} skeleton={skeleton} view={view} focus={focus} onFocus={setFocus} showInspector={layout.inspector} onSelectCamera={id=>{setSelected(id);setFocus("all");}}/>}
        </DockShelf>
      </section>
    </div>
    <footer className="studio-footer"><span>{devices.length} configured · {devices.filter(d=>d.state==="tracking").length} tracking</span><span>View isolation does not mute live solver inputs</span><span>Diagnostic history · up to 50 Hz</span></footer>
  </section>;
}
