import { useState } from "react";
import {
  ArrowRight,
  Check,
  ChevronLeft,
  KeyRound,
  Wifi,
  Sparkles,
  ShieldCheck,
} from "lucide-react";
import { useStore, sendMsg } from "../store/useStore";
import { nextId } from "../lib/format";

/**
 * Onboarding wizard — welcome → connect provider → Wi-Fi → done.
 * Shown on first run (no provider keys stored yet).
 */
export function OnboardingWizard() {
  const onboarding = useStore((s) => s.onboarding);
  const setOnboarding = useStore((s) => s.setOnboarding);
  const providers = useStore((s) => s.providers);
  const setPendingConnectProvider = useStore((s) => s.setPendingConnectProvider);
  const [ssid, setSsid] = useState("");
  const [wifiSent, setWifiSent] = useState(false);

  if (!onboarding) return null;

  const anyProvider = providers.some((p) => p.has_key);

  const connectWifi = () => {
    const name = ssid.trim();
    if (!name) return;
    // Two-step daemon flow: the first message triggers a network scan and
    // arms the "next chat.send is the SSID" flag; the second message is
    // captured as the SSID, after which the daemon asks for the password
    // via secret.request (masked modal).
    sendMsg({ type: "chat.send", id: nextId("wifi1"), text: "connect to wifi" });
    sendMsg({ type: "chat.send", id: nextId("wifi2"), text: name });
    setWifiSent(true);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm animate-fade-in">
      <div className="glass w-[480px] rounded-panel p-6 shadow-2xl">
        <div className="mb-1 flex items-center gap-2 text-[11px] uppercase tracking-wider text-faint">
          <span>Step {onboarding.kind === "welcome" ? 1 : onboarding.kind === "provider" ? 2 : onboarding.kind === "wifi" ? 3 : 4} of 4</span>
          <span className="flex-1" />
          <span className="flex items-center gap-1.5">
            <ShieldCheck size={12} className="text-success" /> loopback only
          </span>
        </div>
        <div className="mb-4 h-1 rounded-full bg-surface-2">
          <div
            className="h-1 rounded-full bg-copper transition-all duration-300 ease-out"
            style={{
              width:
                onboarding.kind === "welcome"
                  ? "25%"
                  : onboarding.kind === "provider"
                    ? "50%"
                    : onboarding.kind === "wifi"
                      ? "75%"
                      : "100%",
            }}
          />
        </div>

        {/* WELCOME */}
        {onboarding.kind === "welcome" && (
          <div>
            <div className="flex h-12 w-12 items-center justify-center rounded-panel border border-copper/40 bg-copper/15 text-2xl text-copper-hover">
              ◈
            </div>
            <h1 className="mt-3 text-lg font-semibold text-ink">Welcome to DIAD AGENT</h1>
            <p className="mt-1.5 text-[12.5px] leading-relaxed text-dim">
              The graphical face of the DIAD intent-driven OS. This window talks to
              your local daemon on <span className="font-mono text-ink/80">127.0.0.1:7860</span> —
              everything stays on this machine.
            </p>
            <ul className="mt-3 space-y-1 text-[12px] text-dim">
              <li className="flex items-center gap-2">
                <Sparkles size={12} className="text-copper-hover" /> Streaming chat with 10+ universal providers
              </li>
              <li className="flex items-center gap-2">
                <KeyRound size={12} className="text-copper-hover" /> Secrets stored in the OS keyring
              </li>
              <li className="flex items-center gap-2">
                <Wifi size={12} className="text-copper-hover" /> Offline intent router — still useful with no network
              </li>
            </ul>
            <div className="mt-5 flex justify-end">
              <button className="btn-primary" onClick={() => setOnboarding({ kind: "provider" })}>
                Get started <ArrowRight size={14} />
              </button>
            </div>
          </div>
        )}

        {/* PROVIDER */}
        {onboarding.kind === "provider" && (
          <div>
            <h1 className="text-lg font-semibold text-ink">Connect a provider</h1>
            <p className="mt-1.5 text-[12.5px] text-dim">
              Pick one to start chatting. Your API key is sent straight to the daemon
              and stored in the OS keyring — never in this window.
            </p>
            <div className="mt-3 max-h-[260px] space-y-1.5 overflow-y-auto pr-1">
              {providers.map((p) => (
                <button
                  key={p.name}
                  className="flex w-full items-center gap-2.5 rounded-control border border-border-subtle bg-surface-2/50 px-3 py-2 text-left transition-all duration-150 ease-out hover:border-copper/40 hover:bg-surface-2"
                  onClick={() => {
                    if (p.has_key) {
                      sendMsg({ type: "provider.switch", name: p.name });
                      setOnboarding({ kind: "wifi" });
                    } else {
                      setPendingConnectProvider(p.name);
                    }
                  }}
                >
                  <span className="font-mono text-[13px] text-ink">{p.name}</span>
                  <span className="flex-1" />
                  {p.has_key ? (
                    <span className="flex items-center gap-1 text-[10.5px] text-success">
                      <Check size={11} /> switch
                    </span>
                  ) : (
                    <span className="text-[10.5px] text-faint">connect…</span>
                  )}
                </button>
              ))}
            </div>
            {anyProvider && (
              <p className="mt-2 text-[11px] text-success">
                ✓ A provider is ready — continue.
              </p>
            )}
            <div className="mt-5 flex justify-between">
              <button className="btn-ghost" onClick={() => setOnboarding({ kind: "welcome" })}>
                <ChevronLeft size={14} /> Back
              </button>
              <button
                className="btn-primary"
                disabled={!anyProvider}
                onClick={() => setOnboarding({ kind: "wifi" })}
              >
                Continue <ArrowRight size={14} />
              </button>
            </div>
          </div>
        )}

        {/* WIFI */}
        {onboarding.kind === "wifi" && (
          <div>
            <h1 className="text-lg font-semibold text-ink">Connect to Wi-Fi</h1>
            <p className="mt-1.5 text-[12.5px] text-dim">
              Optional — DIAD works offline too. Enter a network name and the daemon
              will ask for the password (masked).
            </p>
            <div className="mt-3 flex gap-2">
              <input
                className="control flex-1"
                placeholder="Network SSID"
                value={ssid}
                disabled={wifiSent}
                onChange={(e) => setSsid(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && connectWifi()}
              />
              <button
                className="btn-primary"
                disabled={wifiSent || !ssid.trim()}
                onClick={connectWifi}
              >
                <Wifi size={14} /> {wifiSent ? "Waiting…" : "Connect"}
              </button>
            </div>
            {wifiSent && (
              <p className="mt-2 text-[11px] text-warn">
                If a password prompt appears, enter it (it stays on this machine).
              </p>
            )}
            <div className="mt-5 flex justify-between">
              <button className="btn-ghost" onClick={() => setOnboarding({ kind: "provider" })}>
                <ChevronLeft size={14} /> Back
              </button>
              <button
                className="btn-primary"
                onClick={() => setOnboarding({ kind: "done" })}
              >
                {wifiSent ? "I'm connected — finish" : "Skip — finish"} <ArrowRight size={14} />
              </button>
            </div>
          </div>
        )}

        {/* DONE */}
        {onboarding.kind === "done" && (
          <div className="text-center">
            <div className="mx-auto flex h-12 w-12 items-center justify-center rounded-full border border-success/40 bg-success/10 text-success">
              <Check size={22} />
            </div>
            <h1 className="mt-3 text-lg font-semibold text-ink">You're all set</h1>
            <p className="mt-1.5 text-[12.5px] text-dim">
              Ask DIAD AGENT anything. Use <span className="kbd">Ctrl K</span> for the
              palette or <span className="kbd">F6</span> to switch providers any time.
            </p>
            <div className="mt-5">
              <button
                className="btn-primary"
                onClick={() => {
                  localStorage.setItem("diad.onboarded", "1");
                  setOnboarding(null);
                  sendMsg({ type: "chat.send", id: nextId("hello"), text: "/help" });
                }}
              >
                Start chatting <ArrowRight size={14} />
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
