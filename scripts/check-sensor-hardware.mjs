import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {evaluateHardware} from './sensor-hardware-acceptance.mjs';
const [benchmarkPath,journalPath,output,...args]=process.argv.slice(2);
assert.ok(benchmarkPath&&journalPath&&output,'Usage: node scripts/check-sensor-hardware.mjs BENCHMARK.json CAPTURE-REPORT.json OUTPUT.json --rate 200 [--mag-rate 10]');
function option(name,fallback){
  const index=args.indexOf(name);
  const value=index<0?fallback:Number(args[index+1]);
  assert.ok(Number.isFinite(value),name+' requires a finite number');
  return value;
}
const options={rate:option('--rate',NaN),magRate:option('--mag-rate',0),minRateRatio:option('--min-rate-ratio',.95),maxGapRatio:option('--max-gap-ratio',.01),maxFpsRegression:option('--max-fps-regression',.1),minSeconds:option('--min-seconds',15)};
assert.ok(options.rate>0&&options.magRate>=0&&options.minRateRatio>0&&options.minRateRatio<=1&&options.maxGapRatio>=0&&options.maxGapRatio<=1&&options.maxFpsRegression>=0&&options.maxFpsRegression<=1&&options.minSeconds>0,'Invalid acceptance thresholds');
const [benchmark,journal]=await Promise.all([benchmarkPath,journalPath].map(async p=>JSON.parse(await readFile(p,'utf8'))));
const report=evaluateHardware(benchmark,journal,options);
await writeFile(output,JSON.stringify(report,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({passed:report.passed,failed:report.checks.filter(c=>!c.pass),manual_pending:report.manual_pending,output},null,2));
if(!report.passed)process.exitCode=1;
