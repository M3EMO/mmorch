"""server_frontend — the live dashboard HTML, lifted verbatim out of server.py (it is a static string, not logic; keeping it here shrinks the god-module).
"""
FRONTEND = """<!DOCTYPE html><html><head><meta charset=utf-8>
<title>mmorch live</title><meta name=viewport content="width=device-width,initial-scale=1">
<style>
body{font-family:system-ui,sans-serif;margin:0;background:#0d0d0f;color:#e8e8ea}
header{padding:12px 16px;border-bottom:1px solid #26262b;display:flex;gap:12px;align-items:center}
h1{font-size:15px;font-weight:500;margin:0}input,button,textarea{font:inherit;background:#16161a;color:#e8e8ea;border:1px solid #2c2c33;border-radius:8px;padding:6px 10px}
button{cursor:pointer}button:hover{background:#22222a}
main{display:grid;grid-template-columns:1fr 1fr;gap:16px;padding:16px}
.card{background:#141418;border:1px solid #26262b;border-radius:12px;padding:12px}
.ev{font-size:13px;padding:4px 8px;border-left:3px solid #444;margin:3px 0;display:flex;gap:8px;align-items:center}
.running{border-color:#d8a13a}.done{border-color:#3a9e6f}.error{border-color:#c0453a}.gate{border-color:#7f77dd}.pending{border-color:#555}
.dot{width:8px;height:8px;border-radius:50%;flex:none}
.muted{color:#8a8a92;font-size:12px}.row{display:flex;gap:6px;flex-wrap:wrap;align-items:center}
#feed{max-height:60vh;overflow:auto}textarea{width:100%;min-height:60px}
.pill{font-size:11px;padding:2px 8px;border-radius:10px;background:#1e1e24;color:#b8b8c0}
</style></head><body>
<header><h1>mmorch · live</h1><span class=muted id=conn>conectando…</span>
<input id=token placeholder="token" style="margin-left:auto;width:140px"><button onclick=connect()>conectar</button></header>
<main>
<div class=card><div class=row><strong>subagentes</strong><span class=pill id=cnt>0</span>
<button onclick=clearfeed() style="margin-left:auto">limpiar</button></div><div id=feed></div></div>
<div class=card><strong>control</strong>
<div class=row style="margin-top:4px"><span class=muted>destino</span>
<select id=target style="flex:1"><option value=local>local (este host)</option></select></div>
<p class=muted>rubric_loop (tarea + 1 criterio checkable de ejemplo)</p>
<textarea id=task placeholder="implementa inc(x)=x+1"></textarea>
<div class=row style="margin-top:8px"><button onclick=runRubric()>▶ run rubric</button>
<button onclick=runFan()>▶ run fan_out</button><button onclick=loadState()>↻ estado</button></div>
<hr style="border:none;border-top:1px solid #26262b;margin:10px 0">
<p class=muted>project-aware · PRIMARIO mmorch (barato, cero cupo) · claude = escalada (plan)</p>
<div class=row><select id=proj style="flex:1"></select><button onclick=loadProjects()>↻</button></div>
<div class=row style="margin-top:6px"><input id=ptask placeholder="instruccion" style="flex:1"></div>
<div class=row style="margin-top:6px"><input id=pfile placeholder="archivo (ej app.py)" style="flex:1">
<input id=ptest placeholder="test_cmd (ej python -m pytest -q)" style="flex:1"></div>
<div class=row style="margin-top:6px"><button onclick="runMmorch()">▶ mmorch (barato)</button>
<button onclick="runClaude('plan')">claude analizar</button><button onclick="runClaude('edit')">claude editar</button></div>
<pre id=state class=muted style="white-space:pre-wrap;max-height:20vh;overflow:auto"></pre></div>
<div class=card style="grid-column:1/-1"><div class=row><strong>kanban</strong>
<span class=muted>jobs por estado</span><button onclick=loadState() style="margin-left:auto">↻</button></div>
<div id=kanban style="display:grid;grid-template-columns:repeat(5,1fr);gap:8px;margin-top:8px"></div></div>
<div class=card style="grid-column:1/-1"><div class=row><strong>fleet</strong>
<span class=muted>hosts del tailnet</span><button onclick=loadFleet() style="margin-left:auto">↻</button></div>
<div class=row style="margin-top:6px"><input id=hname placeholder="nombre" style="width:90px">
<input id=hurl placeholder="http://100.x.x.x:8787" style="flex:1"><input id=htok placeholder="token" style="width:120px">
<button onclick=addHost()>+ host</button></div>
<div id=fleet style="margin-top:8px"></div></div>
</main>
<script>
let T='';
function connect(){T=document.getElementById('token').value;
 const es=new EventSource('/events?token='+encodeURIComponent(T));
 es.onopen=()=>document.getElementById('conn').textContent='live';
 es.onerror=()=>document.getElementById('conn').textContent='desconectado';
 es.onmessage=e=>addEv(JSON.parse(e.data));loadState();loadProjects();loadFleet();}
function addEv(ev){const f=document.getElementById('feed');const d=document.createElement('div');
 d.className='ev '+(ev.status||'pending');
 d.innerHTML='<span class=dot></span><b>'+(ev.node||ev.type)+'</b><span class=muted>'+ev.status+'</span> '+(ev.detail||'');
 f.prepend(d);document.getElementById('cnt').textContent=f.children.length;}
function clearfeed(){document.getElementById('feed').innerHTML='';document.getElementById('cnt').textContent=0;}
function H(){return {'Content-Type':'application/json','X-Token':T};}
// rutea el job al destino elegido: local -> /run/*, host del fleet -> /fleet/run (server->server)
function submitJob(path,payload){const t=document.getElementById('target').value;
 if(!t||t==='local'){return fetch(path,{method:'POST',headers:H(),body:JSON.stringify(payload)});}
 return fetch('/fleet/run',{method:'POST',headers:H(),body:JSON.stringify({host:t,path:path,payload:payload})});}
function runRubric(){const task=document.getElementById('task').value||'implementa inc(x)=x+1';
 submitJob('/run/rubric',{task,K:5,criteria:[
  {id:'c1',desc:'inc pasa',kind:'checkable',checker:'python_exec',ctx:{code:'{attempt_code}\\nassert inc(1)==2'}}]});}
function runFan(){submitJob('/run/fanout',{prompts:['di hola','di chau','di test']});}
function loadState(){fetch('/state?token='+encodeURIComponent(T)).then(r=>r.json()).then(s=>{
 document.getElementById('state').textContent=JSON.stringify({calls:s.summary&&s.summary.calls,cost:s.summary&&s.summary.total_cost_usd,sections:s.sections,budget:s.budget},null,2);
 renderKanban(s.jobs||{});});}
const COLS=['queued','running','done','error','gate'];
function renderKanban(jobs){const k=document.getElementById('kanban');k.innerHTML='';
 const by={};COLS.forEach(c=>by[c]=[]);
 Object.entries(jobs).forEach(([id,j])=>{const st=COLS.includes(j.status)?j.status:(j.status==='approved'?'done':'running');by[st].push([id,j]);});
 COLS.forEach(c=>{const col=document.createElement('div');col.style='background:#141418;border:1px solid #26262b;border-radius:8px;padding:6px;min-height:60px';
  col.innerHTML='<div class=muted style="font-size:11px;text-transform:uppercase;margin-bottom:4px">'+c+' ('+by[c].length+')</div>';
  by[c].forEach(([id,j])=>{const card=document.createElement('div');card.className='ev '+c;card.style='font-size:11px;margin:3px 0;padding:4px 6px';
   card.innerHTML='<b>'+(j.kind||'')+'</b> '+(j.title||id)+'<br><span class=muted>'+(j.host||'')+(j.engine?(' · '+j.engine):'')+'</span>';col.appendChild(card);});
  k.appendChild(col);});}
function loadFleet(){fetch('/fleet?token='+encodeURIComponent(T)).then(r=>r.json()).then(s=>{
 const f=document.getElementById('fleet');f.innerHTML='';const st=(s.state&&s.state.hosts)||{};
 const tg=document.getElementById('target');const cur=tg.value;          // repuebla el dropdown destino
 tg.innerHTML='<option value=local>local (este host)</option>';
 Object.entries(s.hosts||{}).forEach(([n,h])=>{const hs=st[n]||{};
  const o=document.createElement('option');o.value=n;o.textContent=n;tg.appendChild(o);
  const div=document.createElement('div');div.className='ev '+(hs.ok?'done':'error');
  const calls=hs.summary?hs.summary.calls:'?';
  div.innerHTML='<b>'+n+'</b> '+h.url+' <span class=muted>'+(hs.ok?('ok · '+calls+' calls'):'caido')+'</span>';
  const b=document.createElement('button');b.textContent='usar';b.style='margin-left:auto;font-size:11px;padding:2px 8px';
  b.onclick=()=>{tg.value=n;};div.appendChild(b);f.appendChild(div);});
 tg.value=[...tg.options].some(o=>o.value===cur)?cur:'local';}); }
function addHost(){const name=document.getElementById('hname').value,url=document.getElementById('hurl').value,token=document.getElementById('htok').value;
 if(!name||!url){alert('nombre + url');return;}
 fetch('/fleet',{method:'POST',headers:H(),body:JSON.stringify({name,url,token})}).then(()=>loadFleet());}
function loadProjects(){fetch('/projects?token='+encodeURIComponent(T)).then(r=>r.json()).then(s=>{
 const sel=document.getElementById('proj');sel.innerHTML='';
 Object.keys(s.projects||{}).forEach(n=>{const o=document.createElement('option');o.value=n;o.textContent=n;sel.appendChild(o);});});}
function runMmorch(){const project=document.getElementById('proj').value;const task=document.getElementById('ptask').value;
 const target_file=document.getElementById('pfile').value;const test_cmd=document.getElementById('ptest').value||null;
 if(!project||!task||!target_file){alert('mmorch necesita proyecto + instruccion + archivo');return;}
 submitJob('/run/project',{project,task,engine:'mmorch',target_file,test_cmd,push:true});}
function runClaude(mode){const project=document.getElementById('proj').value;const task=document.getElementById('ptask').value;
 if(!project||!task){alert('elegí proyecto + instruccion');return;}
 submitJob('/run/project',{project,task,engine:'claude',mode,push:mode==='edit'});}
</script></body></html>"""
