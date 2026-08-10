import { useEffect, useRef } from "react";
import type { Envelope, ServerMsg } from "../lib/types";
import { nextId } from "../lib/format";
import { useStore } from "../stores/useStore";

/**
 * Owns the WebSocket connection to the DIAD AGENT daemon
 * (ws://127.0.0.1:7860/ws, protocol v1) and fans every ServerMsg
 * into the zustand store. Auto-reconnects with a short backoff.
 */
export function useWebSocket(url = "ws://127.0.0.1:7860/ws"): void {
  const socketRef = useRef<WebSocket | null>(null);
  const reconnectRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const closedRef = useRef(false);

  useEffect(() => {
    let disposed = false;

    const dispatch = (raw: unknown) => {
      const envelope = raw as Envelope<ServerMsg>;
      if (!envelope || typeof envelope.v !== "number" || !envelope.msg) return;
      if (envelope.v !== 1) {
        console.warn(`[diad] unknown protocol version ${envelope.v}`);
        return;
      }
      const m = envelope.msg;
      const store = useStore.getState();
      switch (m.type) {
        case "connected": {
          store.setWsConnected(true);
          store.setProviderModel({
            provider: m.provider,
            model: m.model,
            connected: m.connected,
            streaming: m.streaming,
            version: m.daemon_version,
          });
          // Prime the UI with live data.
          store.send?.({ v: 1, msg: { type: "providers.list", id: nextId("pl") } });
          store.send?.({ v: 1, msg: { type: "stats.get", id: nextId("st") } });
          store.send?.({ v: 1, msg: { type: "agents.list", id: nextId("ag") } });
          break;
        }
        case "token":
          store.appendToken(m.id, m.text);
          break;
        case "stream.done":
          store.finishStream(m.id);
          break;
        case "status":
          store.setProviderModel({
            provider: m.provider,
            model: m.model,
            connected: m.connected,
            streaming: m.streaming,
            version: m.daemon_version,
          });
          break;
        case "providers":
          store.setProviders(m.entries);
          // First-run wizard: show when no provider has a stored key.
          if (store.onboarding === null && !localStorage.getItem("diad.onboarded")) {
            if (!m.entries.some((e) => e.has_key)) {
              store.setOnboarding({ kind: "welcome" });
            } else {
              localStorage.setItem("diad.onboarded", "1");
            }
          }
          // Auto-advance onboarding if we're waiting for a provider.
          if (store.onboarding?.kind === "provider") {
            const anyKeyed = m.entries.some((e) => e.has_key);
            if (anyKeyed) store.setOnboarding({ kind: "wifi" });
          }
          break;
        case "models":
          store.setModels(m.models, m.current);
          break;
        case "stats": {
          store.setStats({
            tokens: m.tokens,
            latency_ms: m.latency_ms,
            battery: m.battery,
            wifi: m.wifi,
          });
          if (m.cwd && m.cwd !== store.workspace) store.setWorkspace(m.cwd);
          break;
        }
        case "term.output":
          store.appendTermOutput(m.data);
          break;
        case "secret.request":
          store.setPendingSecret({ id: m.id, label: m.label });
          break;
        case "confirm.request":
          store.setPendingConfirm({ id: m.id, label: m.label });
          break;
        case "system": {
          const id = store.appendMessage({
            role: "system",
            content: m.message,
          });
          void id;
          break;
        }
        case "error": {
          const id = store.appendMessage({
            role: "system",
            content: `⚠ ${m.message}`,
          });
          void id;
          useStore.setState({ streaming: false });
          store.setPendingSecret(null);
          store.setPendingConfirm(null);
          store.setPendingConnectProvider(null);
          break;
        }
        case "agents":
          store.setAgents(m.agents);
          break;
        case "tool_use": {
          // Tool calls belong to the most recent assistant message.
          const msgs = useStore.getState().messages;
          const last = msgs[msgs.length - 1];
          const msgId = last?.id ?? "msg-fallback";
          store.addToolCard(msgId, {
            requestId: m.id,
            callId: m.call_id,
            name: m.name,
            args: m.args,
            confirmRequired: m.confirm_required,
          });
          break;
        }
        case "tool_result":
          store.resolveToolCard(m.id, "done", {
            success: m.success,
            output: m.output,
            status_line: m.status_line,
          });
          break;
        case "edit_diff": {
          const msgs = useStore.getState().messages;
          const last = msgs[msgs.length - 1];
          const msgId = last?.id ?? "msg-fallback";
          store.addDiffCard(msgId, { requestId: m.id, path: m.path, diff: m.diff });
          break;
        }
        case "file.content":
          store.openFile(m.path, m.content);
          break;
        case "file.written":
          store.markFileSaved(m.path);
          break;
        case "files":
          store.setExplorer(m.path, m.entries, false);
          break;
        default:
          break;
      }
    };

    let attempts = 0;
    let ws: WebSocket | null = null;

    const connect = () => {
      if (disposed) return;
      try {
        ws = new WebSocket(url);
      } catch {
        scheduleReconnect();
        return;
      }
      socketRef.current = ws;

      ws.onopen = () => {
        attempts = 0;
      };

      ws.onmessage = (ev) => {
        try {
          dispatch(JSON.parse(ev.data as string));
        } catch (err) {
          console.warn("[diad] bad frame", err);
        }
      };

      ws.onclose = () => {
        if (socketRef.current === ws) socketRef.current = null;
        if (!closedRef.current) useStore.getState().setWsConnected(false);
        scheduleReconnect();
      };

      ws.onerror = () => {
        ws?.close();
      };
    };

    const scheduleReconnect = () => {
      if (disposed || closedRef.current) return;
      if (reconnectRef.current) clearTimeout(reconnectRef.current);
      attempts += 1;
      const delay = Math.min(500 * 2 ** Math.min(attempts, 5), 8000);
      reconnectRef.current = setTimeout(connect, delay);
    };

    connect();

    // Register the send function with the store.
    useStore.getState().setSend((msg) => {
      const sock = socketRef.current;
      if (sock && sock.readyState === WebSocket.OPEN) {
        sock.send(JSON.stringify(msg));
      }
    });

    return () => {
      disposed = true;
      closedRef.current = true;
      if (reconnectRef.current) clearTimeout(reconnectRef.current);
      ws?.close();
    };
  }, [url]);
}
