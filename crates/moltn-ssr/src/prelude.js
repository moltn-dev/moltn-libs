// Environnement minimal fourni au bundle SSR avant son évaluation.
// URL / URLSearchParams sont fournis par le bundle lui-même (shim de @moltn/vite).

globalThis.process ??= { env: { NODE_ENV: 'production' } };

// Fourni par QuickJS, pas par V8 nu (c'est une API web).
globalThis.queueMicrotask ??= (task) => { Promise.resolve().then(task); };

// Noms seulement testés par `instanceof` pendant un rendu serveur (ex. <Link> d'Inertia).
for (const name of [
  'FormData', 'Blob', 'File', 'FileList', 'DOMException', 'Document', 'Element',
  'HTMLElement', 'HTMLTitleElement', 'SVGElement', 'MathMLElement', 'ShadowRoot',
]) {
  if (typeof globalThis[name] === 'undefined') globalThis[name] = class {};
}

// Minuteurs inertes : l'appel réussit, le rappel n'est jamais exécuté. Un rendu serveur
// s'arrête dès que le HTML est produit ; même sous Node, le minuteur partirait après la réponse.
{
  let id = 0;
  globalThis.setTimeout ??= () => ++id;
  globalThis.setInterval ??= () => ++id;
  globalThis.clearTimeout ??= () => {};
  globalThis.clearInterval ??= () => {};
}

// react-dom/server.browser crée un MessageChannel dès son chargement.
globalThis.MessageChannel ??= class {
  constructor() {
    this.port1 = { onmessage: null };
    this.port2 = { postMessage: (data) => queueMicrotask(() => this.port1.onmessage?.({ data })) };
  }
};

// react-dom/server.browser crée un TextEncoder dès son chargement (seul `encode` est utilisé).
globalThis.TextEncoder ??= class {
  get encoding() { return 'utf-8'; }
  encode(input = '') {
    const s = String(input);
    const out = [];
    for (let i = 0; i < s.length; i++) {
      let c = s.charCodeAt(i);
      if (c >= 0xd800 && c <= 0xdbff && i + 1 < s.length) {
        const d = s.charCodeAt(i + 1);
        if (d >= 0xdc00 && d <= 0xdfff) {
          c = 0x10000 + ((c - 0xd800) << 10) + (d - 0xdc00);
          i++;
        }
      }
      if (c >= 0xd800 && c <= 0xdfff) c = 0xfffd; // surrogate isolé
      if (c < 0x80) out.push(c);
      else if (c < 0x800) out.push(0xc0 | (c >> 6), 0x80 | (c & 63));
      else if (c < 0x10000) out.push(0xe0 | (c >> 12), 0x80 | ((c >> 6) & 63), 0x80 | (c & 63));
      else out.push(0xf0 | (c >> 18), 0x80 | ((c >> 12) & 63), 0x80 | ((c >> 6) & 63), 0x80 | (c & 63));
    }
    return new Uint8Array(out);
  }
};

{
  const log = globalThis.__moltnLog;
  const json = (x) => {
    try { return JSON.stringify(x); } catch { return String(x); }
  };
  // La pile QuickJS ne contient pas le message : on logge les deux.
  const format = (args) => args
    .map((x) => (x instanceof Error ? `${x}\n${x.stack ?? ''}` : typeof x === 'string' ? x : json(x)))
    .join(' ');
  // Un console.error pendant un rendu le fait échouer : Vue attrape les exceptions
  // des composants et les signale ainsi au lieu de rejeter le rendu.
  const errors = [];
  globalThis.__moltnTakeErrors = () => errors.splice(0);
  globalThis.console = {
    log: (...a) => log('info', format(a)),
    info: (...a) => log('info', format(a)),
    debug: (...a) => log('debug', format(a)),
    warn: (...a) => log('warn', format(a)),
    error: (...a) => {
      const message = format(a);
      errors.push(message);
      log('error', message);
    },
  };
}
