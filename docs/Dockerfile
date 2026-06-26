# syntax=docker/dockerfile:1

# ---- build: render the Mintlify docs to a static site ----
FROM node:22-bookworm-slim AS build
WORKDIR /docs
RUN apt-get update \
 && apt-get install -y --no-install-recommends unzip ca-certificates \
 && rm -rf /var/lib/apt/lists/*
# mint pulls puppeteer transitively; the static export does not need Chromium.
ENV PUPPETEER_SKIP_DOWNLOAD=true
RUN npm install -g mint@latest
COPY . /docs
# `mint export` produces a self-contained static site + a zero-dep serve.js
RUN mint export --output /tmp/export.zip \
 && mkdir -p /site \
 && unzip -q /tmp/export.zip -d /site \
 && test -f /site/serve.js \
 && find /site/_next -name "*.css" -exec sed -i 's/font-display:swap/font-display:optional/g' {} + \
 && cp /docs/serve.cjs /site/serve.js

# ---- run: serve the static export (reads $PORT, binds all interfaces) ----
FROM node:22-bookworm-slim AS run
WORKDIR /site
ENV NODE_ENV=production
COPY --from=build /site /site
EXPOSE 3000
CMD ["node", "serve.js"]
