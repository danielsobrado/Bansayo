import { Icon } from "../../../ui/Icon";
import { DEFAULT_LAYOUT, EXPLORER_WIDTH, INSPECTOR_WIDTH, type StudioLayout } from "./studioConfig";
export function LayoutControls({layout,onChange}:{layout:StudioLayout;onChange:(value:StudioLayout)=>void}) {
  return <details className="studio-layout-menu">
    <summary title="Studio layout"><Icon name="grid"/> Layout</summary>
    <div className="studio-layout-popover">
      <div className="studio-panel-title">LAYOUT PRESETS</div>
      <button onClick={()=>onChange({...DEFAULT_LAYOUT})}>Bench</button>
      <button onClick={()=>onChange({...layout,explorer:true,inspector:false})}>Wide instruments</button>
      <button onClick={()=>onChange({...layout,explorer:false,inspector:false})}>Focus</button>
      <label>Explorer width <span className="mono">{layout.explorerWidth}px</span><input type="range" aria-label="Explorer width" min={EXPLORER_WIDTH.min} max={EXPLORER_WIDTH.max} value={layout.explorerWidth} onChange={e=>onChange({...layout,explorerWidth:Number(e.target.value)})}/></label>
      <label>Inspector width <span className="mono">{layout.inspectorWidth}px</span><input type="range" aria-label="Inspector width" min={INSPECTOR_WIDTH.min} max={INSPECTOR_WIDTH.max} value={layout.inspectorWidth} onChange={e=>onChange({...layout,inspectorWidth:Number(e.target.value)})}/></label>
      <button onClick={()=>onChange({...DEFAULT_LAYOUT})}>Reset layout</button>
    </div>
  </details>;
}
