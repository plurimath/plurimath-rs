// Wait for the proof page's actual asynchronous WASM completion signal.
const [port, url] = process.argv.slice(2);
const base = `http://127.0.0.1:${port}`;
const page = await (await fetch(`${base}/json/new?about:blank`, {method:"PUT"})).json();
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve, reject) => {
  ws.addEventListener("open", resolve, {once:true});
  ws.addEventListener("error", reject, {once:true});
});
let serial=0;
const pending = new Map();
const events=[];
ws.addEventListener("message", event => {
  const data=JSON.parse(event.data);
  if (data.id) {
    const op=pending.get(data.id);
    if (op) {pending.delete(data.id); data.error ? op.reject(data.error) : op.resolve(data.result);}
  } else events.push(data);
});
function send(method, params={}) {
  return new Promise((resolve,reject)=>{
    const id=++serial;
    pending.set(id,{resolve,reject});
    ws.send(JSON.stringify({id,method,params}));
  });
}
try {
  await send("Page.enable");
  await send("Page.setLifecycleEventsEnabled",{enabled:true});
  const nav=await send("Page.navigate",{url});
  if(nav.errorText) throw new Error(nav.errorText);
  const until=Date.now()+15000;
  while(!events.some(e=>e.method==="Page.lifecycleEvent"&&e.params.loaderId===nav.loaderId&&e.params.name==="load")){
    if(Date.now()>until)throw new Error("target page did not load");
    await new Promise(r=>setTimeout(r,25));
  }
  const result=await send("Runtime.evaluate",{
    expression:`new Promise(resolve=>{
      const timer=setInterval(()=>{
        try{const report=JSON.parse(document.body.textContent);clearInterval(timer);clearTimeout(deadline);resolve({report,dom:document.documentElement.outerHTML});}catch{}
      },25);
      const deadline=setTimeout(()=>{clearInterval(timer);resolve({report:{error:"no browser completion signal"},dom:document.documentElement.outerHTML});},30000);
    })`,
    awaitPromise:true, returnByValue:true
  });
  if(result.exceptionDetails)throw new Error(JSON.stringify(result.exceptionDetails));
  console.log(JSON.stringify(result.result.value));
} finally {ws.close();await fetch(`${base}/json/close/${page.id}`);}
