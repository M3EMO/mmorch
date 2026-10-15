# Imagen del oraculo agente (ticket 05): Chromium de Playwright + el paquete de Python de la MISMA version.
# La imagen oficial trae los navegadores en /ms-playwright pero no el paquete pip.
FROM mcr.microsoft.com/playwright/python:v1.62.0-noble
RUN pip install --no-cache-dir --break-system-packages playwright==1.62.0
