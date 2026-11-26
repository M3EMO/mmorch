# Destino y hogar del compresor
Type: grilling
Status: resolved
Blocked by: 
Map: ../map.md

## Question

El mapa tiene un destino propuesto, no confirmado. Confirmá con el usuario tres cosas. Primero, si el destino es una spec o un cambio hecho. Segundo, dónde vive el compresor: Root, este repo en mantenimiento o un repo nuevo. Tercero, qué significa "listo" para cada pieza: un número de ahorro, un piso de fidelidad o los dos.

## Answer

Resuelto con el usuario el 2026-11-26 en un grilling. Captura completa: `vault/brainstorms/2026-11-26-vault-y-compresor-destino.md`.

- El mapa termina en una spec, y ECC la construye (usuario: "Spec para ECC").
- La supersesión vive en `mmorch/vault.py` de este repo, como mantenimiento.
- El compresor vive en Root, como pieza nueva.
- El compresor cubre solo el contexto de arranque. El compresor de notas queda fuera de alcance.
- El compresor está listo con un arranque medido de 55k tokens o menos, sobre el piso de fidelidad.
- La meta era 52k. El usuario la bajó a 55k el 2026-11-26, porque 52k pedía un recorte fuera de lo medido ("Sí, 55k").
- La supersesión está lista cuando un test determinista de pares pasa en el 100%: el recall nunca devuelve una nota reemplazada.
