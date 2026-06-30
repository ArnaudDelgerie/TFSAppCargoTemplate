// Vanilla demo wiring: submit a username, get a personalised greeting back.
// On async builds the greeting is produced by the worker and pushed over Mercure
// SSE; on sync builds the handler runs inline and the greeting comes straight
// back in the HTTP response. All strings are translated server-side and handed
// in via data-demo-i18n, so the page stays consistent in the rendered locale.

function format(template, replacements) {
  return Object.entries(replacements).reduce(
    (text, [key, value]) => text.replace(`%${key}%`, value),
    template ?? '',
  );
}

export function initDemo() {
  const root = document.querySelector('[data-demo]');
  if (!root) {
    return;
  }

  const topic = root.dataset.demoTopic;
  const asyncEnabled = root.dataset.demoAsync === 'true';
  const i18n = JSON.parse(root.dataset.demoI18n || '{}');

  const target = (name) => root.querySelector(`[data-demo-target="${name}"]`);
  const form = target('form');
  const usernameEl = target('username');
  const errorEl = target('error');
  const greetingEl = target('greeting');
  const httpCountEl = target('httpCount');
  const sseCountEl = target('sseCount');
  const jobCountEl = target('jobCount');
  const statusEl = target('status');
  const logsEl = target('logs');

  let httpCount = 0;
  let sseCount = 0;
  // Seed from the server-rendered DB count so live increments stay accurate.
  let jobCount = parseInt(jobCountEl.textContent, 10) || 0;

  const log = (message) => {
    const item = document.createElement('li');
    item.textContent = `[${new Date().toLocaleTimeString()}] ${message}`;
    logsEl.prepend(item);
  };

  const showGreeting = (sentence) => {
    greetingEl.textContent = sentence;
    greetingEl.classList.add('greeting--filled');
  };

  form.addEventListener('submit', async (event) => {
    event.preventDefault();
    errorEl.hidden = true;

    try {
      log(asyncEnabled ? i18n.dispatchingAsync : i18n.dispatchingSync);

      const response = await fetch('/api/dispatch', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Accept: 'application/json',
          'X-Requested-With': 'XMLHttpRequest',
        },
        body: JSON.stringify({ username: usernameEl.value }),
      });

      if (response.status === 422) {
        errorEl.textContent = i18n.validationError;
        errorEl.hidden = false;
        return;
      }
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }

      const payload = await response.json();
      httpCount += 1;
      httpCountEl.textContent = String(httpCount);
      // A row was just persisted server-side; reflect it without a reload.
      jobCount += 1;
      jobCountEl.textContent = String(jobCount);

      if (payload.mode === 'sync') {
        // Handler ran inline: the greeting is already in the response.
        showGreeting(payload.sentence);
        log(format(i18n.httpDone, { jobId: payload.jobId }));
      } else {
        // Async: the worker will produce the greeting; SSE delivers it.
        log(format(i18n.httpAccepted, { jobId: payload.jobId }));
      }
    } catch (error) {
      log(format(i18n.dispatchFailed, { error: error.message }));
    }
  });

  if (!asyncEnabled) {
    // Sync build: no worker, no SSE. The HTTP response already carries the
    // greeting, so there is nothing to subscribe to.
    statusEl.textContent = i18n.statusSync;
    log(i18n.asyncDisabled);
    return;
  }

  const url = new URL('/.well-known/mercure', window.location.origin);
  url.searchParams.append('topic', topic);
  log(format(i18n.sseOpening, { url: url.toString() }));

  const source = new EventSource(url);
  source.onopen = () => {
    statusEl.textContent = i18n.statusConnected;
    log(i18n.sseConnected);
  };
  source.onerror = () => {
    statusEl.textContent = i18n.statusReconnecting;
    log(format(i18n.sseIssue, { state: source.readyState }));
  };
  source.onmessage = (event) => {
    const payload = JSON.parse(event.data);
    sseCount += 1;
    sseCountEl.textContent = String(sseCount);
    if (payload.sentence) {
      showGreeting(payload.sentence);
    }
    log(format(i18n.sseDone, { jobId: payload.jobId, event: event.lastEventId || 'none' }));
  };
}
