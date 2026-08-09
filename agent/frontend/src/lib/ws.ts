// ---------------------------------------------------------------------------
// WebSocket client for the DIAD AGENT daemon (ws://127.0.0.1:7860/ws)
// ---------------------------------------------------------------------------

import {
  ClientMsg,
  Envelope,
  PROTOCOL_VERSION,
  ServerMsg,
  WsStatus,
} from "./protocol";

export const WS_URL = `ws://${location.hostname}:7860/ws`;

let nextId = 0;
export function newId(prefix = "req"): string {
  nextId += 1;
  return `${prefix}-${Date.now().toString(36)}-${nextId}`;
}

export type MsgHandler = (msg: ServerMsg) => void;

/** A resilient WebSocket connection to the DIAD AGENT daemon. */
export class DaemonSocket {
  private ws: WebSocket | null = null;
  private handlers = new Set<MsgHandler>();
  private statusHandlers = new Set<(s: WsStatus) => void>();
  private reconnectDelay = 500;
  private closedByUs = false;
  status: WsStatus = "connecting";

  connect(): void {
    this.closedByUs = false;
    this.setStatus("connecting");
    try {
      this.ws = new WebSocket(WS_URL);
    } catch {
      this.scheduleReconnect();
      return;
    }
    const ws = this.ws;
    ws.onopen = () => {
      this.reconnectDelay = 500;
      this.setStatus("connected");
    };
    ws.onmessage = (ev) => {
      try {
        const msg = JSON.parse(ev.data as string) as ServerMsg;
        for (const h of this.handlers) h(msg);
      } catch (err) {
        console.error("DIAD: bad frame", err);
      }
    };
    ws.onclose = () => {
      this.setStatus("disconnected");
      if (!this.closedByUs) this.scheduleReconnect();
    };
    ws.onerror = () => {
      // onclose follows; reconnect handled there.
    };
  }

  private scheduleReconnect(): void {
    setTimeout(() => {
      if (!this.closedByUs) this.connect();
    }, this.reconnectDelay);
    this.reconnectDelay = Math.min(this.reconnectDelay * 2, 8000);
  }

  close(): void {
    this.closedByUs = true;
    this.ws?.close();
    this.setStatus("disconnected");
  }

  /** Send a versioned client message. */
  send(msg: ClientMsg): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      console.warn("DIAD: socket not open, dropping", msg.type);
      return;
    }
    const env: Envelope = { v: PROTOCOL_VERSION, msg };
    this.ws.send(JSON.stringify(env));
  }

  onMessage(h: MsgHandler): () => void {
    this.handlers.add(h);
    return () => this.handlers.delete(h);
  }

  onStatus(h: (s: WsStatus) => void): () => void {
    this.statusHandlers.add(h);
    return () => this.statusHandlers.delete(h);
  }

  private setStatus(s: WsStatus): void {
    this.status = s;
    for (const h of this.statusHandlers) h(s);
  }
}

/** Singleton daemon socket shared by the whole app. */
export const daemon = new DaemonSocket();
