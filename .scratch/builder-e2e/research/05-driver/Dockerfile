# Imagen del oraculo agente (tickets 05 y 11): Chromium de Playwright + el paquete de Python de la MISMA version,
# mas Node 22 con Lighthouse y axe-core. Se construye con red; el driver corre despues con --network none.
FROM mcr.microsoft.com/playwright/python:v1.62.0-noble
RUN pip install --no-cache-dir --break-system-packages playwright==1.62.0
ADD https://nodejs.org/dist/v22.23.1/node-v22.23.1-linux-x64.tar.gz /tmp/node.tar.gz
RUN tar -xzf /tmp/node.tar.gz -C /usr/local --strip-components=1 && rm /tmp/node.tar.gz \
 && npm install -g lighthouse@12 axe-core@4 && npm cache clean --force
