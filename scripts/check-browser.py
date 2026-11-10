"""Actual Chrome/WASM conformance with explicit completion; loopback only."""
from pathlib import Path
import argparse, functools, http.server, threading, subprocess, json, time
parser=argparse.ArgumentParser()
parser.add_argument("--output",required=True)
parser.add_argument("--chrome",default="/usr/bin/google-chrome")
parser.add_argument("--fixture",action="append",default=[])
args=parser.parse_args()
root=Path(__file__).resolve().parents[1]
output=Path(args.output)
class Quiet(http.server.SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
server=http.server.ThreadingHTTPServer(("127.0.0.1",0),functools.partial(Quiet,directory=str(root)))
threading.Thread(target=server.serve_forever,daemon=True).start()
profile=root/"tools/chrome-cdp-profile"
portfile=profile/"DevToolsActivePort"
started=time.time()
command=[args.chrome,"--headless","--no-sandbox","--disable-gpu","--disable-dev-shm-usage","--no-first-run","--no-default-browser-check","--disable-background-networking","--remote-debugging-port=0","--remote-debugging-address=127.0.0.1","--remote-allow-origins=*","--user-data-dir="+str(profile),"about:blank"]
rows=[]
with output.with_suffix(".chrome.log").open("w") as log:
 chrome=subprocess.Popen(command,stdout=log,stderr=subprocess.STDOUT)
 try:
  deadline=time.monotonic()+20
  while not portfile.exists() or portfile.stat().st_mtime<started:
   if chrome.poll() is not None or time.monotonic()>=deadline:raise RuntimeError("Chrome did not expose task-local DevTools")
   time.sleep(.05)
  port=portfile.read_text().splitlines()[0]
  for fixture in args.fixture or ["numeric-v1.json","numeric-supplemental-v1.json","numeric-nonfinite-v1.json","numeric-bignum-v1.json","numeric-validation-v1.json"]:
   url=f"http://127.0.0.1:{server.server_port}/scripts/browser-proof.html?fixture={fixture}"
   argv=["node","--experimental-websocket",str(root/"scripts/browser-cdp.mjs"),port,url]
   p=subprocess.run(argv,capture_output=True,text=True,timeout=60)
   prefix=output.parent/(output.stem+"-"+fixture)
   prefix.with_suffix(".stdout").write_text(p.stdout)
   prefix.with_suffix(".stderr").write_text(p.stderr)
   try:
    payload=json.loads(p.stdout)
    prefix.with_suffix(".html").write_text(payload["dom"])
    report=payload["report"]
   except Exception:report={"error":"browser did not produce a JSON report"}
   rows.append({"fixture":fixture,"exit":p.returncode,"report":report})
 finally:
  chrome.terminate()
  try:chrome.wait(timeout=10)
  except subprocess.TimeoutExpired:chrome.kill();chrome.wait()
  server.shutdown();server.server_close()
output.write_text(json.dumps(rows,indent=2)+"\n")
print(json.dumps(rows))
raise SystemExit(0 if all(r["exit"]==0 and not r["report"].get("failures") and not r["report"].get("error") for r in rows) else 1)
