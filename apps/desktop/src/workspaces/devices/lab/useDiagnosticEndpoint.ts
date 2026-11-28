import {useEffect,useState} from "react";
import {request} from "../../../engine/http";

const POLL_MS=2000;
export function useDiagnosticEndpoint<T>(path:string,enabled:boolean,parse:(value:unknown)=>T|null,intervalMs=POLL_MS){
  const [state,setState]=useState<{path:string;data:T|null;error:string|null}>({path,data:null,error:null});
  useEffect(()=>{
    if(!enabled){setState({path,data:null,error:null});return;}
    const controller=new AbortController();
    let timer:number|undefined;
    async function poll(){
      const response=await request<unknown>("GET",path,undefined,(url,init)=>fetch(url,{...init,cache:"no-store",signal:AbortSignal.any([controller.signal,AbortSignal.timeout(5000)])}));
      if(controller.signal.aborted)return;
      const data=response.ok?parse(response.data):null;
      setState({path,data,error:data?null:response.ok?"Invalid diagnostic response":response.error.message});
      if(intervalMs>0)timer=window.setTimeout(()=>void poll(),intervalMs);
    }
    void poll();
    return ()=>{controller.abort();window.clearTimeout(timer);};
  },[path,enabled,parse,intervalMs]);
  return enabled&&state.path===path?state:{data:null,error:null};
}
