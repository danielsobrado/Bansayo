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
  let cursor=0;
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
  await page.screenshot({path:info.outputPath("capture-analysis.png")});
  const recording=page.waitForEvent("download");
  await studio.getByRole("button",{name:"Export recording"}).click();
  expect((await recording).suggestedFilename()).toBe("bansayo-full-rate.json");
  await page.setViewportSize({width:390,height:844});
  await views.getByRole("button",{name:"Signals",exact:true}).click();
  await expect(studio.getByLabel("Signal time window")).toBeVisible();
  await page.screenshot({path:info.outputPath("signals-mobile.png")});
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBe(true);
  expect(errors).toEqual([]);
});
