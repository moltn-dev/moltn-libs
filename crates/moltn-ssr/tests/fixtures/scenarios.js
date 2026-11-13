// Bundle de test : chaque composant déclenche un scénario de robustesse.
globalThis.__moltnRender = async (page) => {
  switch (page.component) {
    case 'loop':
      while (true) {}
    case 'throw':
      throw new Error('intentional boom');
    case 'console-error':
      // Ce que fait Vue quand un composant lève : il logge et rend une page partielle.
      console.error(new Error('error swallowed by the framework'));
      return { head: [], body: '<p>partiel</p>' };
    case 'oom': {
      const chunks = [];
      while (true) chunks.push(new Array(1e5).fill(1));
    }
    case 'pending':
      return new Promise(() => {});
    case 'deep': {
      const dive = (n) => dive(n + 1) + 1;
      return { head: [], body: String(dive(0)) };
    }
    case 'microtask-loop': {
      // Aucune boucle JS : chaque microtâche en programme une autre, sans fin.
      const again = () => Promise.resolve().then(again);
      again();
      return new Promise(() => {});
    }
    case 'late-error':
      // Le rendu est déjà résolu quand cette microtâche s'exécute.
      Promise.resolve().then(() => console.error('late error'));
      return { head: [], body: 'late' };
    case 'timers': {
      let fired = false;
      const timeout = setTimeout(() => { fired = true; }, 0);
      const interval = setInterval(() => { fired = true; }, 10);
      clearTimeout(timeout);
      clearInterval(interval);
      await Promise.resolve();
      return { head: [], body: `${typeof timeout},${timeout !== interval},${fired}` };
    }
    case 'await-timer':
      await new Promise((resolve) => setTimeout(resolve, 10));
      return { head: [], body: 'jamais' };
    case 'intl': {
      const date = new Intl.DateTimeFormat('fr', { day: '2-digit', month: 'short', year: 'numeric', timeZone: 'UTC' })
        .format(new Date(Date.UTC(2010, 9, 31, 12)));
      const number = (1234.5).toLocaleString('fr');
      const plural = new Intl.PluralRules('fr').select(2) === 'other' ? '2 éléments' : '?';
      return { head: [], body: [date, number, plural].join('|') };
    }
    case 'bad-output':
      return { html: '<p>mauvaise forme</p>' };
    case 'env':
      return {
        head: [],
        body: [
          typeof URL,
          process.env.NODE_ENV,
          new FormData() instanceof FormData,
          typeof MessageChannel,
        ].join(','),
      };
    case 'utf8':
      return { head: [], body: Array.from(new TextEncoder().encode('aé€😀\ud800')).join(',') };
    default:
      await Promise.resolve();
      return { head: [`<title>${page.component}</title>`], body: `<div id="app">${page.props.n ?? ''}</div>` };
  }
};

export default globalThis.__moltnRender;
