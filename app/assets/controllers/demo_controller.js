import { Controller } from '@hotwired/stimulus';

export default class extends Controller {
  static targets = ['button', 'httpCount', 'sseCount', 'jobCount', 'logs', 'status'];
  static values = {
    topic: String,
  };

  connect() {
    this.httpCount = 0;
    this.sseCount = 0;
    // Seed from the server-rendered DB count so live increments stay accurate.
    this.jobCount = parseInt(this.jobCountTarget.textContent, 10) || 0;
    this.openMercureStream();
  }

  disconnect() {
    if (this.eventSource) {
      this.eventSource.close();
    }
  }

  async dispatch() {
    try {
      this.log('HTTP dispatching...');
      const response = await fetch('/api/dispatch', {
        method: 'POST',
        headers: {
          Accept: 'application/json',
          'X-Requested-With': 'XMLHttpRequest',
        },
      });

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }

      const payload = await response.json();
      this.httpCount += payload.serverCountIncrement ?? 1;
      this.httpCountTarget.textContent = String(this.httpCount);
      // A row was just persisted server-side; reflect it without a reload.
      this.jobCount += 1;
      this.jobCountTarget.textContent = String(this.jobCount);
      this.log(`HTTP accepted ${payload.jobId}`);
    } catch (error) {
      this.log(`Dispatch failed: ${error.message}`);
    }
  }

  openMercureStream() {
    const url = new URL('/.well-known/mercure', window.location.origin);
    url.searchParams.append('topic', this.topicValue);

    this.log(`SSE opening ${url.toString()}`);
    this.eventSource = new EventSource(url);
    this.eventSource.onopen = () => {
      this.statusTarget.textContent = 'Mercure connected';
      this.log('SSE connected');
    };
    this.eventSource.onerror = () => {
      this.statusTarget.textContent = 'Mercure reconnecting...';
      this.log(`SSE connection issue, readyState=${this.eventSource.readyState}`);
    };
    this.eventSource.onmessage = (event) => {
      const payload = JSON.parse(event.data);
      this.sseCount += 1;
      this.sseCountTarget.textContent = String(this.sseCount);
      this.log(`SSE done ${payload.jobId} event=${event.lastEventId || 'none'}`);
    };
  }

  log(message) {
    const item = document.createElement('li');
    const time = new Date().toLocaleTimeString();
    item.textContent = `[${time}] ${message}`;
    this.logsTarget.prepend(item);
  }
}
