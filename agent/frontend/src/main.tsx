import { StrictMode, useEffect } from "react";
import { createRoot } from "react-dom/client";

import "./index.css";
import App from "./App";
import { useStore } from "./store";
import { daemon } from "./lib/ws";

/** Wire the daemon socket into the store once, at startup. */
function Bootstrap() {
  useEffect(() => {
    const offMsg = daemon.onMessage((msg) => useStore.getState().dispatch(msg));
    const offStatus = daemon.onStatus((s) => useStore.setState({ ws: s }));
    daemon.connect();
    const statsTimer = window.setInterval(() => {
      if (daemon.status === "connected") useStore.getState().refreshStats();
    }, 5000);
    return () => {
      offMsg();
      offStatus();
      daemon.close();
      window.clearInterval(statsTimer);
    };
  }, []);
  return null;
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Bootstrap />
    <App />
  </StrictMode>
);
