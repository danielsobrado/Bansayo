import { createRoot } from "react-dom/client";
import { StrictMode } from "react";
import type { DeviceStatus } from "@contracts";
import "@fontsource/ibm-plex-sans/400.css";
import "@fontsource/ibm-plex-sans/500.css";
import "@fontsource/ibm-plex-mono/400.css";
import "../../src/styles/tokens.css";
import "../../src/styles/themes.css";
import "../../src/styles/base.css";
import "../../src/styles/app.css";
import "../../src/styles/workbench.css";
import { DeviceLab } from "../../src/workspaces/devices/lab/DeviceLab";

const device:DeviceStatus={
  descriptor:{id:"MOSAICO-01",source_id:"mosaico",kind:"imu_tracker",origin:"live",model:"BIMU v2 fixture",firmware:"fixture",transport:"udp",capabilities:["orientation","angular_velocity","acceleration","battery","signal_strength"],nominal_rate_hz:200,camera:null},
  state:"tracking",reasons:[],assignment:"right_shin",
  stream:{measured_rate_hz:198.4,last_seen_t_us:10_000_000,received:5000,dropped:2,time_quality:"estimated"},
  battery:{level:.82,charging:false},signal:{rssi_dbm:-48},quality:.95,
  calibration:[{kind:"mounting",state:"valid",profile_id:null}],
  time_sync:{time_quality:"estimated",offset_us:0,uncertainty_us:1800,drift_ppm:5,settled:true,resets:0},
};
const offline:DeviceStatus={...device,descriptor:{...device.descriptor,id:"TRACKER-02",source_id:"other",model:"Generic tracker"},state:"disconnected",battery:null,signal:null,assignment:null};
createRoot(document.getElementById("root")!).render(<StrictMode><div style={{height:"100dvh",display:"flex",flexDirection:"column",padding:8}}><DeviceLab devices={[device,offline]} live skeleton={null}/></div></StrictMode>);
