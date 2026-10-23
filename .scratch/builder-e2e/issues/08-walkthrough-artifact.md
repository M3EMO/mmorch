# Generador del walkthrough como Artifact
Type: prototype
Status: resolved
Blocked by: 
Map: ../map.md

## Question

¿Cómo genera Opus el walkthrough del ticket 01 como Artifact privado, y cómo registra las preguntas, correcciones y oks del usuario?

- Secciones: idea en una oración, boceto, módulos ordenados con valor × esfuerzo × riesgo, tabla de casos con fuente, verificación por módulo, costo estimado.
- Comentarios por sección y un ok por sección; el builder arranca solo con todos los oks.
- Entendible: qué y por qué, sin código, términos explicados, ETS.
- Primer caso: el walkthrough de F2 sueldo neto, incluida la convención anualizada o acumulativa.

## Answer

Resuelto (2026-10-15) con el primer walkthrough real: https://claude.ai/artifact/9CBZ9NDfu7XKBGA7XquTjG (privado). Fuente: `research/08-walkthrough/f2-walkthrough.html`.

Cómo funciona:

- Opus arma la página a partir del plan del entregable, los contratos, la tabla de casos y las corridas medidas. Las seis secciones son las del ticket 01.
- Cada sección tiene un botón "Comentar", que abre el compositor de comentarios de claude.ai pegado a esa sección (`comments` en modo `composer_only`, sin pedir permiso).
- Cada sección tiene un botón "Dar ok", que guarda `{ok, por, ts}` en la colección `oks` (capacidades `db` y `user`).
- La compuerta: antes de lanzar `python -m sdlc.builder`, Claude lee `oks` con `ArtifactData`; el builder arranca solo con las seis secciones en `ok: true`. Los comentarios se leen con `ArtifactComments`.
- Las decisiones abiertas van marcadas dentro de su sección: la convención anualizada o acumulativa, el tope de ANSES de fuente secundaria y la etiqueta del input que encontró axe.

Verificado: la colección `oks` responde vacía después de publicar. Sin verificar: los clics de ok y de comentario, que solo puede hacer el usuario.

Límite: esta página la escribió Opus a mano. Generalizar a otros entregables necesita una plantilla con las mismas secciones y un procedimiento escrito (por ejemplo, en la skill `/project`).
