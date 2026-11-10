"""Run actual Chrome/WASM conformance against explicit fixtures; loopback only."""
from pathlib import Path
import argparse,functools,http.server,threading,subprocess,json,re,html
parser=argparse.ArgumentParser();parser.add_argument("--output",required=True);parser.add_argument("--chrome",default="/usr/bin/google-chrome");parser.add_argument("--fixture",action="append",default=[]);args=parser.parse_args()
root=Path(__file__).resolve().parents[1]
class Quiet(http.server.SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
server=http.server.ThreadingHTTPServer(("127.0.0.1",0),functools.partial(Quiet,directory=str(root)));threading.Thread(target=server.serve_forever,daemon=True).start();rows=[]
try:
 for fixture in args.fixture or ["numeric-v1.json","numeric-supplemental-v1.json"]:
  url=f"http://127.0.0.1:{server.server_port}/scripts/browser-proof.html?fixture={fixture}"
  command=[args.chrome,"--headless","--no-sandbox","--disable-gpu","--disable-dev-shm-usage","--no-first-run","--no-default-browser-check","--user-data-dir="+str(root/"tools/chrome-profile"),"--virtual-time-budget=15000","--dump-dom",url]
  p=subprocess.run(command,capture_output=True,text=True,timeout=60)
  body=re.search(r"<body[^>]*>(.*?)</body>",p.stdout,re.S)
  try:report=json.loads(html.unescape(body.group(1)))
  except Exception:report={"error":"browser did not produce a JSON report","stderr":p.stderr[-1000:]}
  rows.append({"fixture":fixture,"exit":p.returncode,"report":report})
finally:server.shutdown();server.server_close()
Path(args.output).write_text(json.dumps(rows,indent=2)+"\n");print(json.dumps(rows))
raise SystemExit(0 if all(r["exit"]==0 and not r["report"].get("failures") and not r["report"].get("error") for r in rows) else 1)
