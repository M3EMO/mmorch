# orchestration-pcb: corre DENTRO de Colab (kernel de la VM, T4). Lee /content/prompts.jsonl y escribe
# /content/salidas.jsonl. Lo lanza `colab exec -f`; ver scripts/colab_verificador.py.
# vLLM no corre dentro del kernel de Jupyter (pide sys.stdout.fileno()); el kernel lo lanza como proceso aparte
# con el log en un archivo e imprime la cola.
import subprocess
import sys

TRABAJO = r'''
import json, time
from vllm import LLM, SamplingParams

MODELO = "Qwen/Qwen3-4B"
filas = [json.loads(x) for x in open("/content/prompts.jsonl", encoding="utf-8") if x.strip()]
t0 = time.time()
llm = LLM(model=MODELO, dtype="half", max_model_len=8192, gpu_memory_utilization=0.9, enforce_eager=True)  # T4: sin bf16
salidas = llm.chat([f["messages"] for f in filas], SamplingParams(temperature=0.6, top_p=0.95, max_tokens=6144),
                   chat_template_kwargs={"enable_thinking": True})
segundos = round(time.time() - t0)
with open("/content/salidas.jsonl", "w", encoding="utf-8") as fh:
    for f, s in zip(filas, salidas, strict=True):
        fh.write(json.dumps({"i": f["i"], "is_correct": f["is_correct"], "texto": s.outputs[0].text,
                             "modelo": MODELO, "segundos_total": segundos}, ensure_ascii=False) + "\n")
print("listo:", len(filas), "items en", segundos, "s")
'''
open("/content/trabajo_vllm.py", "w", encoding="utf-8").write(TRABAJO)
with open("/content/trabajo_vllm.log", "w", encoding="utf-8") as log:
    rc = subprocess.run([sys.executable, "/content/trabajo_vllm.py"], stdout=log, stderr=subprocess.STDOUT).returncode
print("rc", rc)
print(open("/content/trabajo_vllm.log", encoding="utf-8").read()[-4000:])
