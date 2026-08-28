// diadOS web lockscreen — main entry.
//
// Clean, industrial basalt card with official DIAD logo and Originkit
// animations. Unlocking requires the account password, verified by the
// daemon's PAM-backed POST /api/v1/auth/verify endpoint.

import { useCallback, useEffect, useRef, useState } from "react";
import type { CSSProperties } from "react";
import { createRoot } from "react-dom/client";
import MetallicCube from "./components/originkit/ui/metal-rubics";
import logoWhite from "../../../assets/diad-logo-white.png";

const BG = "#090A0A";
const SUCCESS = "#38A169";
const TEXT = "#E5EAE6";
const TEXT_MUTED = "#8A968E";

const pageStyle: CSSProperties = {
  position: "fixed",
  inset: 0,
  background: BG,
  color: TEXT,
  fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif",
  overflow: "hidden",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  cursor: "default",
  userSelect: "none",
};

const centerCardStyle: CSSProperties = {
  position: "relative",
  zIndex: 10,
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  gap: 16,
  padding: "40px 48px",
  background: "rgba(22, 25, 24, 0.96)",
  backdropFilter: "blur(24px)",
  WebkitBackdropFilter: "blur(24px)",
  border: "1px solid #202623",
  borderRadius: 24,
  boxShadow: "0 24px 64px rgba(0, 0, 0, 0.85), 0 0 0 1px rgba(255, 255, 255, 0.03)",
  width: 390,
  maxWidth: "92vw",
  boxSizing: "border-box",
  transition: "transform 240ms cubic-bezier(0.16, 1, 0.3, 1), opacity 240ms ease",
};

const logoContainerStyle: CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  userSelect: "none",
};

const subtitleStyle: CSSProperties = {
  fontSize: 12,
  fontWeight: 700,
  letterSpacing: "0.28em",
  color: TEXT_MUTED,
  textTransform: "uppercase",
  marginTop: -4,
};

const unlockPromptStyle: CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 8,
  padding: "12px 24px",
  background: "#1E2321",
  border: "1px solid #202623",
  borderRadius: 12,
  color: TEXT,
  fontSize: 13.5,
  fontWeight: 500,
  letterSpacing: "0.03em",
  marginTop: 4,
  transition: "all 140ms ease",
};

const unlockedStyle: CSSProperties = {
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  gap: 12,
  padding: "12px 0",
};

const checkStyle: CSSProperties = {
  width: 52,
  height: 52,
  borderRadius: "50%",
  background: "rgba(56, 161, 105, 0.15)",
  border: `1px solid ${SUCCESS}`,
  color: SUCCESS,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  fontSize: 28,
  fontWeight: 700,
};

const unlockedTextStyle: CSSProperties = {
  fontSize: 14,
  fontWeight: 700,
  letterSpacing: "0.18em",
  textTransform: "uppercase",
  color: TEXT,
};

function LockApp() {
  const [unlocked, setUnlocked] = useState(false);
  const [password, setPassword] = useState("");
  const [error, setError] = useState(false);
  const [checking, setChecking] = useState(false);

  // Chromium launched with `--app` / `--kiosk` often ignores the `autofocus`
  // attribute because the Wayland window is not "active" at page-load time.
  // That made the lock screen silently swallow keystrokes ("typing not
  // allowed"). Focus the password field explicitly: on mount (with retries to
  // ride out the focus race), when the window regains focus, and whenever the
  // user clicks anywhere on the lock screen.
  const inputRef = useRef<HTMLInputElement>(null);
  const focusInput = useCallback(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    focusInput();
    const t1 = setTimeout(focusInput, 60);
    const t2 = setTimeout(focusInput, 250);
    window.addEventListener("focus", focusInput);
    return () => {
      clearTimeout(t1);
      clearTimeout(t2);
      window.removeEventListener("focus", focusInput);
    };
  }, [focusInput]);

  const unlock = useCallback(() => {
    if (unlocked) return;
    setUnlocked(true);
    setTimeout(() => {
      window.close();
    }, 150);
  }, [unlocked]);

  // Verify against the daemon's PAM-backed endpoint
  // (POST /api/v1/auth/verify → sudo diad-lock-auth). On success the daemon
  // also writes the unlock marker that `diad-lock` waits for.
  const handleSubmit = useCallback(
    async (e?: React.FormEvent) => {
      e?.preventDefault();
      if (checking || unlocked) return;
      setChecking(true);
      setError(false);
      try {
        const res = await fetch("http://127.0.0.1:7860/api/v1/auth/verify", {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ password }),
        });
        if (res.ok) {
          unlock();
        } else {
          setError(true);
          setPassword("");
        }
      } catch {
        setError(true);
      } finally {
        setChecking(false);
        // The field is disabled while `checking`, which drops keyboard focus;
        // restore it so the user can retype after a failed attempt.
        setTimeout(() => inputRef.current?.focus(), 0);
      }
    },
    [password, checking, unlocked, unlock],
  );

  return (
    <div style={pageStyle} onPointerDown={focusInput}>
      <div
        style={{
          ...centerCardStyle,
          transform: unlocked ? "scale(0.96)" : "scale(1)",
          opacity: unlocked ? 0.8 : 1,
        }}
      >
        {/* Originkit 3D Metallic Rubik's Cube inside the card */}
        <div
          style={{
            width: 170,
            height: 170,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            margin: "-4px 0 2px 0",
          }}
        >
          <MetallicCube
            colorMode="metallic"
            tint="#B32032"
            cubeGrid={4}
            gap={20}
            dragSensitivity={1}
            rotation={{ x: 12, y: 11, z: 12 }}
            transition={{
              type: "spring",
              stiffness: 200,
              damping: 20,
              mass: 1,
            }}
            sizePercent={40}
          />
        </div>

        {/* Official DIAD Logo Image Asset */}
        <div style={logoContainerStyle}>
          <img
            src={logoWhite}
            alt="DIAD"
            style={{
              height: 46,
              width: "auto",
              objectFit: "contain",
              filter: "drop-shadow(0 4px 14px rgba(0, 0, 0, 0.5))",
            }}
            draggable={false}
          />
        </div>

        <div style={subtitleStyle}>Secure DIAD</div>

        {unlocked ? (
          <div style={unlockedStyle}>
            <div style={checkStyle}>✓</div>
            <div style={unlockedTextStyle}>Unlocked</div>
          </div>
        ) : (
          <form onSubmit={handleSubmit} style={{ display: "contents" }}>
            <input
              type="password"
              value={password}
              onChange={(e) => {
                setPassword(e.target.value);
                setError(false);
              }}
              placeholder="Password"
              autoFocus
              ref={inputRef}
              disabled={checking}
              aria-label="Lockscreen password"
              style={{
                ...unlockPromptStyle,
                outline: error ? "1px solid #D32F2F" : "none",
                boxShadow: error ? "0 0 0 3px rgba(211, 47, 47, 0.25)" : "none",
                width: "100%",
                boxSizing: "border-box",
                textAlign: "center",
              }}
            />
            {error && (
              <div
                style={{
                  fontSize: 12,
                  fontWeight: 600,
                  letterSpacing: "0.06em",
                  color: "#D32F2F",
                  marginTop: -2,
                }}
              >
                Wrong password — try again
              </div>
            )}
            <button
              type="submit"
              disabled={checking}
              style={{
                padding: "9px 28px",
                background: checking ? "#1E2321" : "#B32032",
                border: "1px solid #202623",
                borderRadius: 12,
                color: TEXT,
                fontSize: 13,
                fontWeight: 700,
                letterSpacing: "0.12em",
                textTransform: "uppercase",
                cursor: checking ? "default" : "pointer",
                opacity: 1,
                marginTop: 2,
                transition: "all 140ms ease",
              }}
            >
              {checking ? "Checking…" : "Unlock"}
            </button>
          </form>
        )}
      </div>
    </div>
  );
}
const rootEl = document.getElementById("root");
if (rootEl) {
  createRoot(rootEl).render(<LockApp />);
}
