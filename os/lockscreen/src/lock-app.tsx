// diadOS web lockscreen — main entry.
//
// Clean, industrial basalt card with official DIAD logo.
// Pressing any key on the keyboard or clicking anywhere unlocks the screen.

import { useCallback, useEffect, useState } from "react";
import type { CSSProperties } from "react";
import { createRoot } from "react-dom/client";
import ScrollWaveField from "./components/originkit/ui/scroll-wave-field";
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
  cursor: "pointer",
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

  const unlock = useCallback(() => {
    if (unlocked) return;
    setUnlocked(true);
    setTimeout(() => {
      window.close();
    }, 150);
  }, [unlocked]);

  useEffect(() => {
    const handleKeyDown = () => {
      unlock();
    };
    const handleClick = () => {
      unlock();
    };

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("click", handleClick);
    window.addEventListener("pointerdown", handleClick);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("click", handleClick);
      window.removeEventListener("pointerdown", handleClick);
    };
  }, [unlock]);

  return (
    <div style={pageStyle} onClick={unlock}>
      {/* Originkit Scroll Wave Field 3D Particle Animation with exact user design */}
      <div style={{ position: "absolute", inset: 0, zIndex: 1, pointerEvents: "none" }}>
        <ScrollWaveField
          background="#080512"
          colors={["#B32032", "#FFC200"]}
          density={169}
          dotSize={2}
          scatter={108}
          cameraHeight={50}
          wave={{ waveSpeed: 250, waveHeight: 200, waveLength: 2070 }}
          tilt={{ rollStart: 0, tiltStart: 45 }}
          cursor={{ cursorRadius: 25, cursorLift: 45 }}
          transition={{ type: "spring", stiffness: 800, damping: 60, mass: 1 }}
          style={{ minWidth: "100%", minHeight: "100%", width: "100%", height: "100%" }}
        />
      </div>

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

        <div style={subtitleStyle}>LOCKED</div>

        {unlocked ? (
          <div style={unlockedStyle}>
            <div style={checkStyle}>✓</div>
            <div style={unlockedTextStyle}>Unlocked</div>
          </div>
        ) : (
          <div style={unlockPromptStyle}>
            <span>Press any key or click to unlock</span>
          </div>
        )}
      </div>
    </div>
  );
}

const rootEl = document.getElementById("root");
if (rootEl) {
  createRoot(rootEl).render(<LockApp />);
}
