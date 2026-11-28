import { test,expect } from "@playwright/test";

const channel=(values:(i:number)=>number[])=>({age_ms:0,points:Array.from({length:251},(_,i)=>({t_us:5_000_000+i*20_000,seq:i+1,device_time_us:5_000_000+i*20_000,values:values(i)}))});
const snapshot={channels:{
  orientation:channel(i=>[0,Math.sin(i*.001),0,Math.cos(i*.001)]),
  gyro:channel(i=>[Math.sin(i*.04),Math.cos(i*.03),Math.sin(i*.02)*.2]),
  accel:channel(i=>[Math.sin(i*.04)*.2,9.80665,Math.cos(i*.04)*.1]),
  mag0:channel(i=>[30*Math.cos(i*.02),30*Math.sin(i*.02),15]),
  mag1:channel(i=>[25*Math.cos(i*.02),25*Math.sin(i*.02),14]),
},timing:{intervals_ms:Array.from({length:128},(_,i)=>[5,5+Math.sin(i)*.6]),sequence_gaps:2}};

test("sensor studio instrumentation and responsive layouts",async({page},info)=>{
  const errors:string[]=[];page.on("pageerror",error=>errors.push(error.message));
  await page.route("**/api/v1/devices/*/inspection",route=>route.fulfill({json:{ok:true,data:snapshot}}));
  let engineRecording={running:false,file:null as string|null,observations:0,lost:0,bytes:0,reason:"Idle"};
  await page.route(/\/inspection\/recording(?:\/stop)?$/,route=>{
    if(route.request().method()==="POST"){
      const stop=route.request().url().endsWith("/stop");
      engineRecording={running:!stop,file:stop?"diagnostics/capture-test.jsonl":"diagnostics/capture-test.jsonl.partial",observations:stop?128:0,lost:0,bytes:stop?4096:0,reason:stop?"Stopped by operator":"Recording"};
    }
    return route.fulfill({json:{ok:true,data:engineRecording}});
  });
  await page.route("**/inspection/performance",route=>route.fulfill({json:{ok:true,data:{enabled:true,timing_enabled:true,epoch:"fixture",samples:[{t_us:1000,cpu_percent_one_core:null,resident_bytes:10485760},{t_us:2001000,cpu_percent_one_core:130,resident_bytes:20971520}],stages:[{stage:"solve",p50_us:1000,p95_us:3000,p99_us:4000,max_us:5000,count:10,budget_us:2000}]}}}));
  await page.route("**/inspection/reliability*",route=>route.fulfill({json:{ok:true,data:{enabled:true,rows:[{epoch:"fixture",at_ms:1000,t_us:1000,devices:[{device:"MOSAICO-01",origin:"fixture",state:"tracking",stream:{measured_rate_hz:200,received:100,dropped:1}}]}]}}}));
  await page.goto("/tests/fixtures/sensor-studio.html");
  const studio=page.getByRole("region",{name:"Sensor Studio"});
  const views=studio.getByRole("navigation",{name:"Sensor Studio views"});
  await expect(studio.getByRole("img",{name:"Gyroscope synchronized signal plot"})).toBeVisible();
  await expect(studio.getByRole("button",{name:"Freeze traces"})).toBeEnabled();
  await page.screenshot({path:info.outputPath("overview-dark.png")});
  await studio.getByRole("button",{name:"Freeze traces"}).click();
  await views.getByRole("button",{name:"Signals",exact:true}).click();
  await studio.getByLabel("Signal component").selectOption("gyro");
  await studio.getByLabel("Signal time window").selectOption("2");
  await studio.getByLabel("Shared signal cursor").focus();
  await page.keyboard.press("Home");
  await studio.getByRole("button",{name:"Maximize Gyroscope"}).click();
  await expect(studio.getByRole("columnheader",{name:"RMS"})).toBeVisible();
  const downloadPromise=page.waitForEvent("download");
  await studio.getByRole("button",{name:"Export CSV"}).click();
  expect((await downloadPromise).suggestedFilename()).toBe("MOSAICO-01-signals.csv");
  await page.screenshot({path:info.outputPath("signals-dark.png")});
  await studio.getByRole("button",{name:"Resume traces"}).click();
  await views.getByRole("button",{name:"Calibration",exact:true}).click();
  await expect(studio.getByRole("img",{name:"Reference: point +X vertically upward"})).toBeVisible();
  await page.screenshot({path:info.outputPath("calibration-dark.png")});
  await page.evaluate(()=>document.documentElement.dataset.theme="silver");
  await views.getByRole("button",{name:"Overview",exact:true}).click();
  await page.screenshot({path:info.outputPath("overview-light.png")});
  await views.getByRole("button",{name:"Events",exact:true}).click();
  await expect(studio.getByText("Observed device events")).toBeVisible();
  await expect(studio.getByRole("cell",{name:"MOSAICO-01",exact:true}).first()).toBeVisible();
  await views.getByRole("button",{name:"Capture",exact:true}).click();
  await studio.getByRole("checkbox",{name:"Persistent MOSAICO-01",exact:true}).check();
  await studio.getByRole("button",{name:"Start engine recording"}).click();
  await expect(studio.getByRole("button",{name:"Stop engine recording"})).toBeEnabled();
  await studio.getByRole("button",{name:"Stop engine recording"}).click();
  await expect(studio.getByText("diagnostics/capture-test.jsonl",{exact:true})).toBeVisible();
  let cursor=0,clockUs=1_000_000;
  await page.route("**/inspection/clock",route=>{
    const t_us=clockUs;
    clockUs=Math.max(clockUs+320_000,1_000_000+cursor*5000+1);
    return route.fulfill({json:{ok:true,data:{session_epoch_utc:"2026-11-28T00:00:00Z",t_us}}});
  });
  await page.route("**/inspection/batch*",route=>{
    const url=new URL(route.request().url());
    if(!url.searchParams.has("epoch"))return route.fulfill({json:{ok:true,data:{epoch:1,next:0,lost:0,reset:false,entries:[]}}});
    const entries=Array.from({length:64},()=>{const n=++cursor;return {cursor:n,channel:"gyro",point:{t_us:1_000_000+n*5000,seq:n,device_time_us:null,values:[Math.sin(n*.2),0,0]}};});
    return route.fulfill({json:{ok:true,data:{epoch:1,next:cursor,lost:0,reset:false,entries}}});
  });
  await studio.getByRole("checkbox",{name:"MOSAICO-01",exact:true}).check();
  await studio.getByRole("button",{name:"Start full-rate capture"}).click();
  await expect(studio.getByText(/64 observations/)).toBeVisible();
  await studio.getByRole("button",{name:"Add event marker"}).click();
  await studio.getByRole("button",{name:"Stop capture"}).click();
  await expect(studio.getByRole("img",{name:"Recorded signal and event markers"})).toBeVisible();
  await expect(studio.getByRole("img",{name:"FFT amplitude spectrum"})).toBeVisible();
  await expect(studio.getByRole("img",{name:"Power spectral density"})).toBeVisible();
  await expect(studio.getByRole("heading",{name:"Multi-device timing and signal health"})).toBeVisible();
  await expect(studio.getByRole("heading",{name:"Stationary noise and drift acceptance"})).toBeVisible();
  await expect(studio.getByRole("heading",{name:"Camera-to-IMU timing"})).toBeVisible();
  await expect(studio.getByText("Enable a camera landmark pair before starting browser capture.",{exact:false})).toBeVisible();
  await studio.getByLabel("Confirm this entire recording was stationary").check();
  await expect(studio.getByText("MOSAICO-01 · INCONCLUSIVE")).toBeVisible();
  const stationaryDownload=page.waitForEvent("download");
  await studio.getByRole("button",{name:"Export stationary report"}).click();
  expect((await stationaryDownload).suggestedFilename()).toBe("bansayo-stationary-report.json");
  await expect(studio.getByText("Record at least two devices with the selected channel for timing comparison.")).toBeVisible();
  const syncReport=page.waitForEvent("download");
  await studio.getByRole("button",{name:"Export sync report"}).click();
  expect((await syncReport).suggestedFilename()).toBe("bansayo-sensor-sync.json");
  await page.screenshot({path:info.outputPath("capture-analysis.png")});
  const recording=page.waitForEvent("download");
  await studio.getByRole("button",{name:"Export recording"}).click();
  expect((await recording).suggestedFilename()).toBe("bansayo-full-rate.json");
  await views.getByRole("button",{name:"Reliability",exact:true}).click();
  await expect(studio.getByRole("heading",{name:"Sensor reliability history"})).toBeVisible();
  await expect(studio.getByRole("heading",{name:"Engine-owned reliability archive"})).toBeVisible();
  await expect(studio.getByText("1 snapshots on this page")).toBeVisible();
  await expect(studio.getByRole("img",{name:"Engine clock drift (ppm) history"})).toBeVisible();
  const reliability=page.waitForEvent("download");
  await studio.getByRole("button",{name:"Export reliability history"}).click();
  expect((await reliability).suggestedFilename()).toBe("bansayo-reliability.json");
  await views.getByRole("button",{name:"Performance",exact:true}).click();
  await expect(studio.getByRole("img",{name:"Engine CPU (%) history"})).toBeVisible();
  await expect(studio.getByRole("cell",{name:"solve",exact:true})).toBeVisible();
  const performance=page.waitForEvent("download");
  await studio.getByRole("button",{name:"Export performance report"}).click();
  expect((await performance).suggestedFilename()).toBe("bansayo-performance.json");
  await views.getByRole("button",{name:"Calibration",exact:true}).click();
  await expect(studio.getByText(/Calibration bench score: Incomplete/)).toBeVisible();
  const shelf=studio.getByRole("complementary",{name:"Pinned instruments"});
  await shelf.getByLabel("Dock instrument").selectOption("timing");
  await shelf.getByRole("button",{name:"Pin",exact:true}).click();
  await expect(shelf.getByRole("button",{name:"Unpin timing"})).toBeVisible();
  await views.getByRole("button",{name:"Overview",exact:true}).click();
  await expect(shelf.getByRole("button",{name:"Unpin timing"})).toBeVisible();
  await page.screenshot({path:info.outputPath("pinned-instruments.png")});
  await shelf.getByRole("button",{name:"Float timing"}).click();
  const floating=shelf.locator(".studio-instrument-window[data-floating=true]");
  await expect(floating).toBeVisible();
  const before=await floating.boundingBox();
  await floating.getByRole("button",{name:"Move floating instrument"}).press("ArrowLeft");
  await expect.poll(async()=>(await floating.boundingBox())?.x??0).toBeLessThan(before!.x);
  const moved=await floating.boundingBox();
  await floating.getByRole("button",{name:"Resize floating instrument"}).press("ArrowLeft");
  await expect.poll(async()=>(await floating.boundingBox())?.width??0).toBeLessThan(moved!.width);
  await views.getByRole("button",{name:"Events",exact:true}).click();
  await expect(floating).toBeVisible();
  await page.screenshot({path:info.outputPath("floating-instrument.png")});
  await page.reload();
  await expect(floating).toBeVisible();
  await expect.poll(async()=>Math.abs(((await floating.boundingBox())?.width??0)-(moved!.width-20))).toBeLessThan(2);
  await page.setViewportSize({width:390,height:844});
  await expect(floating.getByRole("button",{name:"Move floating instrument"})).toBeHidden();
  await floating.getByRole("button",{name:"Dock timing"}).click();
  await expect(floating).toHaveCount(0);
  await shelf.getByRole("button",{name:"Unpin timing"}).click();
  await page.setViewportSize({width:390,height:844});
  await views.getByRole("button",{name:"Signals",exact:true}).click();
  await expect(studio.getByLabel("Signal time window")).toBeVisible();
  await page.screenshot({path:info.outputPath("signals-mobile.png")});
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBe(true);
  expect(errors).toEqual([]);
});
