import {useEffect,useRef,useState} from "react";
import type {DeviceStatus} from "@contracts";
import {RELIABILITY as C} from "./reliabilityConfig";
import {historyRow,isReliabilityRow,retainRows,type ReliabilityRow} from "./reliabilityMath";
function read(){
  try{const v:unknown=JSON.parse(localStorage.getItem(C.storageKey)??"null");return Array.isArray(v)?retainRows(v.filter(isReliabilityRow),Date.now()):[];}catch{return [];}
}
function persist(rows:ReliabilityRow[]){try{localStorage.setItem(C.storageKey,JSON.stringify(rows));return true;}catch{return false;}}
export function useReliability(devices:DeviceStatus[],live:boolean){
  const [rows,setRows]=useState(read),[storageError,setStorageError]=useState(false);
  const session=useRef(""),last=useRef(0);
  useEffect(()=>{
    if(!live){session.current="";last.current=0;return;}
    if(document.hidden)return;
    const now=Date.now();
    if(!session.current||now<last.current){session.current=Array.from(crypto.getRandomValues(new Uint32Array(4)),v=>v.toString(16).padStart(8,"0")).join("");last.current=0;}
    if(now-last.current<C.sampleMs)return;
    last.current=now;
    const batch=devices.map(d=>historyRow(d,session.current,now)).filter(isReliabilityRow);
    if(batch.length)setRows(old=>retainRows([...old,...batch],now));
  },[devices,live]);
  useEffect(()=>{setStorageError(!persist(rows));},[rows]);
  return {rows,storageError,clear:()=>{last.current=0;session.current="";setRows([]);}};
}
