import { Observation } from './api';

export type ObservationHandler = (obs: Observation) => void;
export type ConnectionStatusHandler = (connected: boolean) => void;

export class TelemetryWebSocketClient {
  private ws: WebSocket | null = null;
  private listeners: Set<ObservationHandler> = new Set();
  private statusListeners: Set<ConnectionStatusHandler> = new Set();
  private reconnectTimeout: number | null = null;
  private backoffMs = 1000;
  private maxBackoffMs = 30000;
  private isExplicitlyClosed = false;

  constructor() {
    this.connect();
  }

  public subscribe(handler: ObservationHandler): () => void {
    this.listeners.add(handler);
    return () => this.listeners.delete(handler);
  }

  public onStatusChange(handler: ConnectionStatusHandler): () => void {
    this.statusListeners.add(handler);
    handler(this.isConnected());
    return () => this.statusListeners.delete(handler);
  }

  public isConnected(): boolean {
    return this.ws !== null && this.ws.readyState === WebSocket.OPEN;
  }

  private getWebSocketUrl(): string {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    return `${protocol}//${window.location.host}/api/v1/ws`;
  }

  private connect() {
    if (this.isExplicitlyClosed) return;

    try {
      const url = this.getWebSocketUrl();
      console.log(`[HUMBOLDT WS] Connecting to live observation bus: ${url}`);
      this.ws = new WebSocket(url);

      this.ws.onopen = () => {
        console.log('[HUMBOLDT WS] Live bus connection established.');
        this.backoffMs = 1000;
        this.notifyStatus(true);
      };

      this.ws.onmessage = (event: MessageEvent) => {
        try {
          const payload = JSON.parse(event.data);
          if (payload.type === 'observation' && payload.data) {
            const obs = payload.data as Observation;
            for (const listener of this.listeners) {
              try {
                listener(obs);
              } catch (e) {
                console.error('[HUMBOLDT WS] Error in observation listener:', e);
              }
            }
          }
        } catch (e) {
          console.warn('[HUMBOLDT WS] Malformed incoming message:', event.data);
        }
      };

      this.ws.onclose = () => {
        console.warn('[HUMBOLDT WS] Connection lost. Scheduling reconnection...');
        this.notifyStatus(false);
        this.scheduleReconnect();
      };

      this.ws.onerror = (err) => {
        console.error('[HUMBOLDT WS] Socket error:', err);
        if (this.ws) {
          this.ws.close();
        }
      };
    } catch (e) {
      console.error('[HUMBOLDT WS] Connection initialization failed:', e);
      this.scheduleReconnect();
    }
  }

  private scheduleReconnect() {
    if (this.reconnectTimeout !== null || this.isExplicitlyClosed) return;

    this.reconnectTimeout = window.setTimeout(() => {
      this.reconnectTimeout = null;
      this.backoffMs = Math.min(this.backoffMs * 1.5, this.maxBackoffMs);
      this.connect();
    }, this.backoffMs);
  }

  private notifyStatus(connected: boolean) {
    for (const listener of this.statusListeners) {
      listener(connected);
    }
  }

  public close() {
    this.isExplicitlyClosed = true;
    if (this.reconnectTimeout !== null) {
      clearTimeout(this.reconnectTimeout);
      this.reconnectTimeout = null;
    }
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
    this.notifyStatus(false);
  }
}

export const telemetryWs = new TelemetryWebSocketClient();
