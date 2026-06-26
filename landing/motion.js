(() => {
  const root = document.documentElement;
  const reduce = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  const coarse = window.matchMedia('(pointer: coarse)').matches;
  if (!reduce) root.classList.add('js-motion');

  /* ── Archetype intensity ── */
  const arch = (document.body.className.match(/archetype-([a-z-]+)/) || [])[1] || '';
  const restrained = /editorial-yohaku|apple-minimal|scandi-calm/.test(arch);
  const crisp = /achromatic-devtool|swiss-international|data-dashboard/.test(arch);
  const sharp = /poster-brutalist|bauhaus-constructivist/.test(arch);
  const warm = /warm-magazine/.test(arch);
  const revealY = restrained ? 10 : (crisp ? 8 : (sharp ? 20 : (warm ? 12 : 14)));
  const staggerMs = restrained ? 28 : (crisp ? 18 : (sharp ? 14 : (warm ? 24 : 22)));
  const wordStaggerMs = restrained ? 18 : (crisp ? 12 : (sharp ? 8 : (warm ? 16 : 14)));

  /* ── Live canvas hero field: WebGL shader -> 2D Bayer -> CSS/SVG dither fallback ── */
  function initCanvasHero() {
    var hero = document.querySelector('[data-canvas-hero]');
    if (!hero) return;
    var layer = hero.querySelector('.canvas-hero-field');
    var webglCanvas = hero.querySelector('canvas[data-canvas-hero-field]');
    if (!layer || !webglCanvas) return;

    var active = !reduce && !coarse;
    var finePointer = active && window.matchMedia && window.matchMedia('(pointer: fine)').matches;
    var raf = 0;
    var mode = 'svg';
    var gl = null;
    var glProgram = null;
    var glBuffer = null;
    var glUniforms = null;
    var twoCanvas = null;
    var twoCtx = null;
    var dpr = 1;
    var cell = 8;
    var scrollMix = 0;
    var pointer = { x: 0.5, y: 0.5, live: 0 };
    var BAYER_4 = [0,8,2,10,12,4,14,6,3,11,1,9,15,7,13,5];
    var css = window.getComputedStyle(root);
    var colors = [];

    var vertexShaderSource = [
      'attribute vec2 aPosition;',
      'varying vec2 vUv;',
      'void main() {',
      '  vUv = aPosition * 0.5 + 0.5;',
      '  gl_Position = vec4(aPosition, 0.0, 1.0);',
      '}'
    ].join('\n');

    var fragmentShaderSource = [
      'precision mediump float;',
      'uniform vec2 uResolution;',
      'uniform float uTime;',
      'uniform float uScroll;',
      'uniform float uActive;',
      'uniform vec2 uPointer;',
      'uniform vec3 uField;',
      'uniform vec3 uAccent;',
      'uniform vec3 uInk;',
      'varying vec2 vUv;',
      'float hash21(vec2 p) {',
      '  p = fract(p * vec2(123.34, 345.45));',
      '  p += dot(p, p + 34.345);',
      '  return fract(p.x * p.y);',
      '}',
      'float valueNoise(vec2 p) {',
      '  vec2 i = floor(p);',
      '  vec2 f = fract(p);',
      '  f = f * f * (3.0 - 2.0 * f);',
      '  float a = hash21(i);',
      '  float b = hash21(i + vec2(1.0, 0.0));',
      '  float c = hash21(i + vec2(0.0, 1.0));',
      '  float d = hash21(i + vec2(1.0, 1.0));',
      '  return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);',
      '}',
      'float fbm(vec2 p) {',
      '  float v = 0.0;',
      '  float amp = 0.5;',
      '  mat2 turn = mat2(0.82, -0.57, 0.57, 0.82);',
      '  for (int i = 0; i < 5; i++) {',
      '    v += amp * valueNoise(p);',
      '    p = turn * p * 2.03 + vec2(11.7, 3.9);',
      '    amp *= 0.5;',
      '  }',
      '  return v;',
      '}',
      'float bayer4(vec2 frag) {',
      '  vec2 p = mod(floor(frag), 4.0);',
      '  float x = p.x;',
      '  float y = p.y;',
      '  float v = 0.0;',
      '  if (y < 0.5) {',
      '    if (x < 0.5) v = 0.0; else if (x < 1.5) v = 8.0; else if (x < 2.5) v = 2.0; else v = 10.0;',
      '  } else if (y < 1.5) {',
      '    if (x < 0.5) v = 12.0; else if (x < 1.5) v = 4.0; else if (x < 2.5) v = 14.0; else v = 6.0;',
      '  } else if (y < 2.5) {',
      '    if (x < 0.5) v = 3.0; else if (x < 1.5) v = 11.0; else if (x < 2.5) v = 1.0; else v = 9.0;',
      '  } else {',
      '    if (x < 0.5) v = 15.0; else if (x < 1.5) v = 7.0; else if (x < 2.5) v = 13.0; else v = 5.0;',
      '  }',
      '  return (v + 0.5) / 16.0;',
      '}',
      'vec3 steppedPalette(float level) {',
      '  vec3 a = uField;',
      '  vec3 b = mix(uField, uAccent, 0.18);',
      '  vec3 c = mix(uField, uInk, 0.16);',
      '  vec3 d = mix(uField, uAccent, 0.32);',
      '  vec3 e = mix(uField, uInk, 0.25);',
      '  if (level < 0.5) return a;',
      '  if (level < 1.5) return b;',
      '  if (level < 2.5) return c;',
      '  if (level < 3.5) return d;',
      '  return e;',
      '}',
      'void main() {',
      '  vec2 uv = gl_FragCoord.xy / max(uResolution, vec2(1.0));',
      '  vec2 aspect = vec2(uResolution.x / max(uResolution.y, 1.0), 1.0);',
      '  vec2 p = (uv - 0.5) * aspect;',
      '  vec2 pointerPull = (uPointer - 0.5) * (0.10 * uActive);',
      '  float drift = uTime * 0.035 * uActive;',
      '  float scroll = uScroll * uActive;',
      '  p += pointerPull;',
      '  vec2 warp = vec2(',
      '    fbm(p * 1.85 + vec2(drift + scroll * 0.24, 1.7)),',
      '    fbm(p * 1.85 + vec2(6.2, -drift + scroll * 0.18))',
      '  );',
      '  vec2 domainWarp = p + (warp - 0.5) * (0.48 + 0.12 * uActive);',
      '  float grain = fbm(domainWarp * 2.35 + warp * 1.7 + drift);',
      '  float bands = abs(sin((domainWarp.x - domainWarp.y + scroll * 0.20) * 4.2));',
      '  float field = clamp(grain * 0.74 + bands * 0.18 + warp.x * 0.08, 0.0, 1.0);',
      '  float threshold = bayer4(gl_FragCoord.xy);',
      '  float quantized = clamp(floor(field * 5.0 + threshold - 0.5), 0.0, 4.0);',
      '  vec3 color = steppedPalette(quantized);',
      '  color = floor(color * 8.0 + 0.5) / 8.0;',
      '  gl_FragColor = vec4(color, 0.96);',
      '}'
    ].join('\n');

    function force2d() {
      return /(?:\?|&)force2d=1(?:&|$)/.test(window.location.search) || window.DESIGN_FORCE_2D_CANVAS_HERO === true;
    }
    function stop() {
      if (raf) window.cancelAnimationFrame(raf);
      raf = 0;
    }
    function mark(state) {
      hero.setAttribute('data-canvas-state', state);
      window.__designCanvasHeroTier = state;
    }
    function show(canvas, state) {
      layer.classList.remove('is-fallback');
      webglCanvas.hidden = canvas !== webglCanvas;
      if (twoCanvas) twoCanvas.hidden = canvas !== twoCanvas;
      canvas.hidden = false;
      mark(state);
    }
    function svgFallback() {
      stop();
      mode = 'svg';
      layer.classList.add('is-fallback');
      webglCanvas.hidden = true;
      if (twoCanvas) twoCanvas.hidden = true;
      mark('svg');
    }
    function clamp(n, lo, hi) { return Math.max(lo, Math.min(hi, n)); }
    function parseColor(value) {
      var v = String(value || '').trim();
      var m;
      if ((m = /^#([0-9a-f]{3})$/i.exec(v))) {
        return [parseInt(m[1][0] + m[1][0], 16), parseInt(m[1][1] + m[1][1], 16), parseInt(m[1][2] + m[1][2], 16)];
      }
      if ((m = /^#([0-9a-f]{6})$/i.exec(v))) {
        return [parseInt(m[1].slice(0, 2), 16), parseInt(m[1].slice(2, 4), 16), parseInt(m[1].slice(4, 6), 16)];
      }
      if ((m = /^rgba?\(([^)]+)\)$/i.exec(v))) {
        var parts = m[1].split(',').slice(0, 3).map(function(x) { return clamp(parseFloat(x), 0, 255); });
        if (parts.length === 3 && parts.every(Number.isFinite)) return parts;
      }
      return null;
    }
    function role(name, fallback) {
      return parseColor(css.getPropertyValue(name)) || parseColor(fallback);
    }
    function mix(a, b, t) {
      return [
        Math.round(a[0] + (b[0] - a[0]) * t),
        Math.round(a[1] + (b[1] - a[1]) * t),
        Math.round(a[2] + (b[2] - a[2]) * t)
      ];
    }
    function rgb(c) { return 'rgb(' + c[0] + ',' + c[1] + ',' + c[2] + ')'; }
    function rgbFloat(c) { return new Float32Array([c[0] / 255, c[1] / 255, c[2] / 255]); }
    function refreshPalette() {
      css = window.getComputedStyle(root);
      var field = role('--field', '#0B0B0F');
      var accent = role('--accent', '#5CB8AE');
      var ink = role('--ink', '#E0DDD8');
      colors = [field, mix(field, accent, .16), mix(field, ink, .14), mix(field, accent, .30), mix(field, ink, .24)];
      return { field: field, accent: accent, ink: ink };
    }
    function updateScroll() {
      if (!active) { scrollMix = 0; return; }
      var max = Math.max(1, document.documentElement.scrollHeight - window.innerHeight);
      scrollMix = clamp(window.scrollY / max, 0, 1);
    }
    function resizeCanvas(target) {
      var box = target.getBoundingClientRect();
      dpr = Math.min(2, Math.max(1, window.devicePixelRatio || 1));
      var w = Math.max(1, Math.ceil(box.width * dpr));
      var h = Math.max(1, Math.ceil(box.height * dpr));
      if (target.width !== w || target.height !== h) {
        target.width = w;
        target.height = h;
      }
      cell = Math.max(coarse ? 12 : 7, Math.round((coarse ? 12 : 8) * dpr));
    }
    function ensure2dCanvas() {
      if (!twoCanvas) {
        twoCanvas = document.createElement('canvas');
        twoCanvas.className = 'canvas-hero-canvas canvas-hero-2d-canvas';
        twoCanvas.setAttribute('data-canvas-hero-field-2d', '');
        twoCanvas.setAttribute('aria-hidden', 'true');
        twoCanvas.hidden = true;
        layer.appendChild(twoCanvas);
        twoCanvas.addEventListener('contextlost', function(event) { if (event.preventDefault) event.preventDefault(); svgFallback(); }, false);
      }
      return twoCanvas;
    }
    function draw2d(time) {
      var target = ensure2dCanvas();
      if (!twoCtx) twoCtx = target.getContext && target.getContext('2d', { alpha: true, desynchronized: true });
      if (!twoCtx) { svgFallback(); return; }
      resizeCanvas(target);
      if (!colors.length) refreshPalette();
      var cols = Math.ceil(target.width / cell);
      var rows = Math.ceil(target.height / cell);
      var drift = active ? (time || 0) * 0.00008 : 0;
      var px = active ? (pointer.x - 0.5) * 0.55 : 0;
      var py = active ? (pointer.y - 0.5) * 0.55 : 0;
      twoCtx.clearRect(0, 0, target.width, target.height);
      for (var y = 0; y < rows; y++) {
        for (var x = 0; x < cols; x++) {
          var nx = x / Math.max(1, cols - 1);
          var ny = y / Math.max(1, rows - 1);
          var warpX = Math.sin((ny + py) * 5.6 + drift * 5.0 + scrollMix * 1.4);
          var warpY = Math.cos((nx + px) * 4.9 - drift * 4.0 - scrollMix * 1.1);
          var wave = Math.sin((nx * 6.2) + warpX * .75 + drift * 6.0) + Math.cos((ny * 6.7) + warpY * .70 - drift * 5.0) + Math.sin((nx + ny + scrollMix * .22) * 8.5 + drift * 3.0);
          var raw = clamp((wave + 3) / 6, 0, 1);
          var ordered = (BAYER_4[(x & 3) + ((y & 3) << 2)] + .5) / 16;
          var level = clamp(Math.floor(raw * colors.length + (ordered - .5)), 0, colors.length - 1);
          twoCtx.fillStyle = rgb(colors[level]);
          twoCtx.fillRect(x * cell, y * cell, cell, cell);
        }
      }
      show(target, active ? '2d' : '2d-static');
    }
    function tick2d(time) {
      draw2d(time || 0);
      if (!reduce && !coarse) raf = window.requestAnimationFrame(tick2d);
    }
    function start2d() {
      stop();
      mode = '2d';
      refreshPalette();
      tick2d(0);
    }
    function shader(type, source) {
      var s = gl.createShader(type);
      gl.shaderSource(s, source);
      gl.compileShader(s);
      if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) {
        throw new Error(gl.getShaderInfoLog(s) || 'shader compile failed');
      }
      return s;
    }
    function linkProgram() {
      var vs = shader(gl.VERTEX_SHADER, vertexShaderSource);
      var fs = shader(gl.FRAGMENT_SHADER, fragmentShaderSource);
      var p = gl.createProgram();
      gl.attachShader(p, vs);
      gl.attachShader(p, fs);
      gl.linkProgram(p);
      if (!gl.getProgramParameter(p, gl.LINK_STATUS)) {
        throw new Error(gl.getProgramInfoLog(p) || 'shader link failed');
      }
      return p;
    }
    function initGlObjects() {
      glProgram = linkProgram();
      gl.useProgram(glProgram);
      glBuffer = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, glBuffer);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1,-1, 1,-1, -1,1, -1,1, 1,-1, 1,1]), gl.STATIC_DRAW);
      var pos = gl.getAttribLocation(glProgram, 'aPosition');
      gl.enableVertexAttribArray(pos);
      gl.vertexAttribPointer(pos, 2, gl.FLOAT, false, 0, 0);
      glUniforms = {
        resolution: gl.getUniformLocation(glProgram, 'uResolution'),
        time: gl.getUniformLocation(glProgram, 'uTime'),
        scroll: gl.getUniformLocation(glProgram, 'uScroll'),
        active: gl.getUniformLocation(glProgram, 'uActive'),
        pointer: gl.getUniformLocation(glProgram, 'uPointer'),
        field: gl.getUniformLocation(glProgram, 'uField'),
        accent: gl.getUniformLocation(glProgram, 'uAccent'),
        ink: gl.getUniformLocation(glProgram, 'uInk')
      };
    }
    function drawWebGl(time) {
      if (!gl || !glProgram) { start2d(); return; }
      resizeCanvas(webglCanvas);
      var pal = refreshPalette();
      gl.viewport(0, 0, webglCanvas.width, webglCanvas.height);
      gl.useProgram(glProgram);
      gl.bindBuffer(gl.ARRAY_BUFFER, glBuffer);
      gl.uniform2f(glUniforms.resolution, webglCanvas.width, webglCanvas.height);
      gl.uniform1f(glUniforms.time, active ? (time || 0) * 0.001 : 0);
      gl.uniform1f(glUniforms.scroll, scrollMix);
      gl.uniform1f(glUniforms.active, active ? 1 : 0);
      gl.uniform2f(glUniforms.pointer, pointer.x, pointer.y);
      gl.uniform3fv(glUniforms.field, rgbFloat(pal.field));
      gl.uniform3fv(glUniforms.accent, rgbFloat(pal.accent));
      gl.uniform3fv(glUniforms.ink, rgbFloat(pal.ink));
      gl.drawArrays(gl.TRIANGLES, 0, 6);
      show(webglCanvas, active ? 'webgl' : 'webgl-static');
    }
    function tickWebGl(time) {
      drawWebGl(time || 0);
      if (!reduce && !coarse) raf = window.requestAnimationFrame(tickWebGl);
    }
    function startWebGl() {
      stop();
      mode = 'webgl';
      if (force2d()) { start2d(); return; }
      gl = webglCanvas.getContext && (webglCanvas.getContext('webgl', { alpha: true, antialias: false, depth: false, stencil: false, premultipliedAlpha: true, preserveDrawingBuffer: false }) || webglCanvas.getContext('experimental-webgl', { alpha: true, antialias: false, depth: false, stencil: false }));
      if (!gl || !gl.getParameter(gl.VERSION)) { start2d(); return; }
      initGlObjects();
      tickWebGl(0);
    }

    try {
      if (!webglCanvas.getContext) { svgFallback(); return; }
      refreshPalette();
      updateScroll();
      if (active) window.addEventListener('scroll', function() { updateScroll(); }, { passive: true });
      if (finePointer) {
        window.addEventListener('pointermove', function(event) {
          if (event.pointerType === 'touch') return;
          pointer.x = clamp(event.clientX / Math.max(1, window.innerWidth), 0, 1);
          pointer.y = clamp(event.clientY / Math.max(1, window.innerHeight), 0, 1);
          pointer.live = 1;
        }, { passive: true });
      }
      webglCanvas.addEventListener('webglcontextlost', function(event) { if (event.preventDefault) event.preventDefault(); start2d(); }, false);
      webglCanvas.addEventListener('contextlost', function(event) { if (event.preventDefault) event.preventDefault(); start2d(); }, false);
      window.addEventListener('resize', function() { if (mode === 'webgl') drawWebGl(0); else if (mode === '2d') draw2d(0); }, { passive: true });
      window.addEventListener('themechange', function() { refreshPalette(); if (mode === 'webgl') drawWebGl(0); else if (mode === '2d') draw2d(0); }, false);
      startWebGl();
    } catch (err) {
      try { start2d(); } catch (fallbackErr) { svgFallback(); }
    }
  }

  /* ── Split-text reveal for section headings ── */
  function splitText(heading) {
    const text = heading.textContent || '';
    if (!text.trim()) return;
    heading.setAttribute('aria-label', text);
    const words = text.split(/(\s+)/);
    heading.innerHTML = '';
    var wordIdx = 0;
    words.forEach(function(w) {
      if (/^\s+$/.test(w)) {
        heading.appendChild(document.createTextNode(w));
      } else {
        var span = document.createElement('span');
        span.className = 'word';
        span.textContent = w;
        span.style.transitionDelay = (wordIdx * wordStaggerMs) + 'ms';
        span.setAttribute('aria-hidden', 'true');
        heading.appendChild(span);
        wordIdx++;
      }
    });
    heading.classList.add('split-ready');
  }

  var headings = Array.from(document.querySelectorAll('.section-lead h2'));
  if (!reduce) headings.forEach(splitText);

  initCanvasHero();

  /* ── Scroll reveal via IntersectionObserver ── */
  var reveal = Array.from(document.querySelectorAll('[data-reveal]'));
  if (reduce || !('IntersectionObserver' in window)) {
    reveal.forEach(function(el) { el.classList.add('is-visible'); });
    headings.forEach(function(h) {
      h.querySelectorAll('.word').forEach(function(w) { w.classList.add('is-visible'); });
    });
    return;
  }

  /* rootMargin '-16%' => trigger at viewport top ~84% */
  var observer = new IntersectionObserver(function(entries) {
    entries.forEach(function(entry) {
      if (!entry.isIntersecting) return;
      var el = entry.target;
      el.classList.add('is-visible');
      /* Stagger children with data-reveal-child */
      var children = el.querySelectorAll('[data-reveal-child]');
      children.forEach(function(c, i) {
        c.style.transitionDelay = (i * staggerMs) + 'ms';
        c.classList.add('is-visible');
      });
      /* Reveal split-text words inside this section */
      var section = el.closest('section') || el;
      var words = section.querySelectorAll('.section-lead h2.split-ready .word');
      words.forEach(function(w) { w.classList.add('is-visible'); });
      observer.unobserve(el);
    });
  }, { threshold: 0.12, rootMargin: '0px 0px -16% 0px' });

  reveal.forEach(function(el) {
    if (el.closest('.site-masthead')) {
      el.classList.add('is-visible');
      /* Masthead headings reveal immediately */
      el.querySelectorAll('.word').forEach(function(w) { w.classList.add('is-visible'); });
    } else {
      observer.observe(el);
    }
  });
})();
