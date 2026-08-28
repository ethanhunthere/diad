// diadOS first-boot setup — main entry.
//
// A fullscreen wizard shown once, on first boot, before the desktop session.
// Six short steps: Language → Keyboard → Time zone → Account → Wi-Fi → Done.
// On "Finish" it POSTs one JSON payload to the local daemon
// (POST /api/v1/setup/apply), which applies it as root and writes the
// completion marker that closes this window and reveals the desktop.
//
// Rules:
//   * Minimal and human — no marketing, no jargon. The logo is the only
//     branding; the card never spells out "DIAD OS".
//   * Wi-Fi is discovered automatically (GET /api/v1/setup/wifi/scan) and
//     shown as a pickable list — no manual SSID typing.
//   * Every screen has a pre-selected safe default, so you can keep pressing
//     Enter to accept everything.

import { useCallback, useEffect, useMemo, useState } from "react";
import type { CSSProperties } from "react";
import { createRoot } from "react-dom/client";
import { AnimatePresence, motion } from "motion/react";
import logoWhite from "../../../assets/diad-logo-white.png";

// ---------------------------------------------------------------------------
// Palette (same basalt identity as the rest of diadOS)
// ---------------------------------------------------------------------------
const BG = "#090A0A";
const CARD = "rgba(22, 25, 24, 0.96)";
const BORDER = "#2A312D";
const FIELD = "#161B19";
const FIELD_ACTIVE = "rgba(110, 181, 255, 0.10)";
const TEXT = "#E8EDE9";
const TEXT_MUTED = "#8A968E";
const ACCENT = "#6EB5FF";
const SUCCESS = "#4ADE80";
const DANGER = "#F6A0A0";

// ---------------------------------------------------------------------------
// Static data — mirrors the allowlists in diad-identity-setup.sh
// ---------------------------------------------------------------------------
interface Lang {
  id: string;
  label: string;
  kb: string;
  tz: string;
}

const LANGUAGES: Lang[] = [
  { id: "en_US.UTF-8", label: "English (United States)", kb: "us", tz: "America/New_York" },
  { id: "en_GB.UTF-8", label: "English (United Kingdom)", kb: "gb", tz: "Europe/London" },
  { id: "de_DE.UTF-8", label: "Deutsch", kb: "de", tz: "Europe/Berlin" },
  { id: "fr_FR.UTF-8", label: "Français", kb: "fr", tz: "Europe/Paris" },
  { id: "es_ES.UTF-8", label: "Español", kb: "es", tz: "Europe/Madrid" },
  { id: "it_IT.UTF-8", label: "Italiano", kb: "it", tz: "Europe/Rome" },
  { id: "pt_PT.UTF-8", label: "Português", kb: "pt", tz: "Europe/Lisbon" },
  { id: "pt_BR.UTF-8", label: "Português (Brasil)", kb: "br", tz: "America/Sao_Paulo" },
  { id: "nl_NL.UTF-8", label: "Nederlands", kb: "nl", tz: "Europe/Amsterdam" },
  { id: "ru_RU.UTF-8", label: "Русский", kb: "ru", tz: "Europe/Moscow" },
  { id: "ja_JP.UTF-8", label: "日本語", kb: "jp", tz: "Asia/Tokyo" },
  { id: "zh_CN.UTF-8", label: "中文（简体）", kb: "cn", tz: "Asia/Shanghai" },
  { id: "ko_KR.UTF-8", label: "한국어", kb: "kr", tz: "Asia/Seoul" },
  { id: "ar_SA.UTF-8", label: "العربية", kb: "ara", tz: "Asia/Riyadh" },
  { id: "tr_TR.UTF-8", label: "Türkçe", kb: "tr", tz: "Europe/Istanbul" },
];

interface Kb {
  id: string;
  label: string;
}

const KEYBOARDS: Kb[] = [
  { id: "us", label: "US English" },
  { id: "gb", label: "UK English" },
  { id: "de", label: "German" },
  { id: "fr", label: "French" },
  { id: "es", label: "Spanish" },
  { id: "it", label: "Italian" },
  { id: "pt", label: "Portuguese" },
  { id: "br", label: "Portuguese (Brazil, ABNT2)" },
  { id: "nl", label: "Dutch" },
  { id: "ru", label: "Russian" },
  { id: "jp", label: "Japanese" },
  { id: "cn", label: "Chinese" },
  { id: "kr", label: "Korean" },
  { id: "ara", label: "Arabic" },
  { id: "tr", label: "Turkish" },
];

interface Tz {
  id: string;
  label: string;
}

interface TzGroup {
  region: string;
  zones: Tz[];
}

// Comprehensive timezone list, grouped by region. The identity helper accepts
// any real IANA zone (it checks /usr/share/zoneinfo), so this can be wide.
const TIMEZONE_GROUPS: TzGroup[] = [
  {
    region: "Universal",
    zones: [{ id: "UTC", label: "UTC — Coordinated Universal Time" }],
  },
  {
    region: "North America",
    zones: [
      { id: "America/New_York", label: "New York (Eastern)" },
      { id: "America/Chicago", label: "Chicago (Central)" },
      { id: "America/Denver", label: "Denver (Mountain)" },
      { id: "America/Phoenix", label: "Phoenix (Mountain, no DST)" },
      { id: "America/Los_Angeles", label: "Los Angeles (Pacific)" },
      { id: "America/Anchorage", label: "Anchorage (Alaska)" },
      { id: "America/Adak", label: "Adak (Hawaii–Aleutian)" },
      { id: "Pacific/Honolulu", label: "Honolulu (Hawaii)" },
      { id: "America/Toronto", label: "Toronto" },
      { id: "America/Vancouver", label: "Vancouver" },
      { id: "America/Winnipeg", label: "Winnipeg" },
      { id: "America/Halifax", label: "Halifax (Atlantic)" },
      { id: "America/St_Johns", label: "St. John's (Newfoundland)" },
      { id: "America/Mexico_City", label: "Mexico City" },
      { id: "America/Puerto_Rico", label: "Puerto Rico (Atlantic)" },
      { id: "America/Panama", label: "Panama (Eastern)" },
      { id: "America/Costa_Rica", label: "Costa Rica" },
      { id: "America/Guatemala", label: "Guatemala" },
      { id: "America/Havana", label: "Havana" },
      { id: "America/Santo_Domingo", label: "Santo Domingo" },
    ],
  },
  {
    region: "South America",
    zones: [
      { id: "America/Bogota", label: "Bogotá" },
      { id: "America/Lima", label: "Lima" },
      { id: "America/Quito", label: "Quito" },
      { id: "America/La_Paz", label: "La Paz" },
      { id: "America/Caracas", label: "Caracas" },
      { id: "America/Manaus", label: "Manaus" },
      { id: "America/Santiago", label: "Santiago" },
      { id: "America/Asuncion", label: "Asunción" },
      { id: "America/Sao_Paulo", label: "São Paulo" },
      { id: "America/Argentina/Buenos_Aires", label: "Buenos Aires" },
      { id: "America/Montevideo", label: "Montevideo" },
      { id: "America/Guyana", label: "Guyana" },
    ],
  },
  {
    region: "Europe",
    zones: [
      { id: "Atlantic/Reykjavik", label: "Reykjavík" },
      { id: "Europe/Dublin", label: "Dublin" },
      { id: "Europe/London", label: "London" },
      { id: "Europe/Lisbon", label: "Lisbon" },
      { id: "Europe/Madrid", label: "Madrid" },
      { id: "Europe/Paris", label: "Paris" },
      { id: "Europe/Brussels", label: "Brussels" },
      { id: "Europe/Amsterdam", label: "Amsterdam" },
      { id: "Europe/Berlin", label: "Berlin" },
      { id: "Europe/Zurich", label: "Zurich" },
      { id: "Europe/Vienna", label: "Vienna" },
      { id: "Europe/Rome", label: "Rome" },
      { id: "Europe/Stockholm", label: "Stockholm" },
      { id: "Europe/Oslo", label: "Oslo" },
      { id: "Europe/Copenhagen", label: "Copenhagen" },
      { id: "Europe/Helsinki", label: "Helsinki" },
      { id: "Europe/Warsaw", label: "Warsaw" },
      { id: "Europe/Prague", label: "Prague" },
      { id: "Europe/Budapest", label: "Budapest" },
      { id: "Europe/Athens", label: "Athens" },
      { id: "Europe/Bucharest", label: "Bucharest" },
      { id: "Europe/Sofia", label: "Sofia" },
      { id: "Europe/Belgrade", label: "Belgrade" },
      { id: "Europe/Riga", label: "Riga" },
      { id: "Europe/Vilnius", label: "Vilnius" },
      { id: "Europe/Tallinn", label: "Tallinn" },
      { id: "Europe/Kyiv", label: "Kyiv" },
      { id: "Europe/Moscow", label: "Moscow" },
      { id: "Europe/Istanbul", label: "Istanbul" },
      { id: "Europe/Minsk", label: "Minsk" },
    ],
  },
  {
    region: "Africa",
    zones: [
      { id: "Africa/Casablanca", label: "Casablanca" },
      { id: "Africa/Algiers", label: "Algiers" },
      { id: "Africa/Tunis", label: "Tunis" },
      { id: "Africa/Cairo", label: "Cairo" },
      { id: "Africa/Lagos", label: "Lagos" },
      { id: "Africa/Accra", label: "Accra" },
      { id: "Africa/Khartoum", label: "Khartoum" },
      { id: "Africa/Nairobi", label: "Nairobi" },
      { id: "Africa/Addis_Ababa", label: "Addis Ababa" },
      { id: "Africa/Johannesburg", label: "Johannesburg" },
      { id: "Africa/Cape_Town", label: "Cape Town" },
      { id: "Africa/Kinshasa", label: "Kinshasa" },
    ],
  },
  {
    region: "Middle East",
    zones: [
      { id: "Asia/Dubai", label: "Dubai" },
      { id: "Asia/Riyadh", label: "Riyadh" },
      { id: "Asia/Kuwait", label: "Kuwait" },
      { id: "Asia/Qatar", label: "Doha" },
      { id: "Asia/Muscat", label: "Muscat" },
      { id: "Asia/Baghdad", label: "Baghdad" },
      { id: "Asia/Tehran", label: "Tehran" },
      { id: "Asia/Jerusalem", label: "Jerusalem" },
      { id: "Asia/Amman", label: "Amman" },
      { id: "Asia/Beirut", label: "Beirut" },
    ],
  },
  {
    region: "Asia",
    zones: [
      { id: "Asia/Karachi", label: "Karachi" },
      { id: "Asia/Kolkata", label: "Kolkata (India)" },
      { id: "Asia/Colombo", label: "Colombo" },
      { id: "Asia/Dhaka", label: "Dhaka" },
      { id: "Asia/Kathmandu", label: "Kathmandu" },
      { id: "Asia/Yangon", label: "Yangon" },
      { id: "Asia/Bangkok", label: "Bangkok" },
      { id: "Asia/Jakarta", label: "Jakarta" },
      { id: "Asia/Singapore", label: "Singapore" },
      { id: "Asia/Kuala_Lumpur", label: "Kuala Lumpur" },
      { id: "Asia/Phnom_Penh", label: "Phnom Penh" },
      { id: "Asia/Ho_Chi_Minh", label: "Ho Chi Minh City" },
      { id: "Asia/Manila", label: "Manila" },
      { id: "Asia/Hong_Kong", label: "Hong Kong" },
      { id: "Asia/Shanghai", label: "Shanghai" },
      { id: "Asia/Taipei", label: "Taipei" },
      { id: "Asia/Seoul", label: "Seoul" },
      { id: "Asia/Tokyo", label: "Tokyo" },
      { id: "Asia/Ulaanbaatar", label: "Ulaanbaatar" },
      { id: "Asia/Tashkent", label: "Tashkent" },
      { id: "Asia/Almaty", label: "Almaty" },
      { id: "Asia/Baku", label: "Baku" },
    ],
  },
  {
    region: "Oceania",
    zones: [
      { id: "Australia/Perth", label: "Perth" },
      { id: "Australia/Darwin", label: "Darwin" },
      { id: "Australia/Brisbane", label: "Brisbane" },
      { id: "Australia/Sydney", label: "Sydney" },
      { id: "Australia/Melbourne", label: "Melbourne" },
      { id: "Australia/Adelaide", label: "Adelaide" },
      { id: "Australia/Hobart", label: "Hobart" },
      { id: "Pacific/Auckland", label: "Auckland" },
      { id: "Pacific/Fiji", label: "Fiji" },
      { id: "Pacific/Guam", label: "Guam" },
      { id: "Pacific/Port_Moresby", label: "Port Moresby" },
      { id: "Pacific/Tahiti", label: "Tahiti" },
    ],
  },
];

const STEPS = ["Language", "Keyboard", "Time zone", "Account", "Wi-Fi", "Done"];

interface WifiNet {
  ssid: string;
  signal: number;
  security: string;
}

// ---------------------------------------------------------------------------
// Styles
// ---------------------------------------------------------------------------
const pageStyle: CSSProperties = {
  position: "fixed",
  inset: 0,
  background: BG,
  color: TEXT,
  fontFamily: "-apple-system, BlinkMacSystemFont, 'Segoe UI', 'Noto Sans', system-ui, sans-serif",
  overflow: "hidden",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  cursor: "default",
  userSelect: "none",
};

const cardStyle: CSSProperties = {
  position: "relative",
  zIndex: 10,
  display: "flex",
  flexDirection: "column",
  width: 600,
  maxWidth: "94vw",
  maxHeight: "92vh",
  padding: "28px 34px",
  background: CARD,
  backdropFilter: "blur(24px)",
  WebkitBackdropFilter: "blur(24px)",
  border: `1px solid ${BORDER}`,
  borderRadius: 22,
  boxShadow: "0 24px 64px rgba(0, 0, 0, 0.8), 0 0 0 1px rgba(255, 255, 255, 0.03)",
  boxSizing: "border-box",
  overflowY: "auto",
};

const logoStyle: CSSProperties = {
  height: 34,
  width: "auto",
  userSelect: "none",
  display: "block",
};

const stepLabelStyle: CSSProperties = {
  fontSize: 11,
  fontWeight: 700,
  letterSpacing: "0.2em",
  textTransform: "uppercase",
  color: ACCENT,
  marginTop: 18,
};

const titleStyle: CSSProperties = {
  fontSize: 21,
  fontWeight: 700,
  marginTop: 6,
  marginBottom: 4,
};

const bodyStyle: CSSProperties = {
  fontSize: 13.5,
  lineHeight: 1.55,
  color: TEXT_MUTED,
  marginBottom: 16,
};

const listStyle: CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 7,
  marginBottom: 4,
};

const optionStyle = (active: boolean): CSSProperties => ({
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  gap: 12,
  padding: "11px 15px",
  background: active ? FIELD_ACTIVE : FIELD,
  border: `1px solid ${active ? ACCENT : BORDER}`,
  borderRadius: 11,
  color: TEXT,
  fontSize: 14,
  fontWeight: active ? 600 : 400,
  cursor: "pointer",
  transition: "background 120ms ease, border-color 120ms ease",
});

const inputStyle: CSSProperties = {
  width: "100%",
  padding: "11px 13px",
  background: FIELD,
  border: `1px solid ${BORDER}`,
  borderRadius: 11,
  color: TEXT,
  fontSize: 14,
  boxSizing: "border-box",
  outline: "none",
  transition: "border-color 120ms ease",
};

const fieldLabelStyle: CSSProperties = {
  fontSize: 11,
  fontWeight: 700,
  letterSpacing: "0.08em",
  textTransform: "uppercase",
  color: TEXT_MUTED,
  marginBottom: 6,
  marginTop: 12,
};

const hintStyle: CSSProperties = {
  fontSize: 12,
  color: TEXT_MUTED,
  marginTop: 8,
  lineHeight: 1.45,
};

const buttonRowStyle: CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  gap: 12,
  marginTop: 20,
};

const primaryBtnStyle = (disabled: boolean): CSSProperties => ({
  padding: "11px 22px",
  background: disabled ? "#1A1E1C" : ACCENT,
  color: disabled ? TEXT_MUTED : "#08121F",
  border: "none",
  borderRadius: 11,
  fontSize: 14,
  fontWeight: 700,
  cursor: disabled ? "default" : "pointer",
  transition: "all 120ms ease",
});

const ghostBtnStyle = (disabled: boolean): CSSProperties => ({
  padding: "11px 16px",
  background: "transparent",
  color: disabled ? "#3A4742" : TEXT_MUTED,
  border: `1px solid ${BORDER}`,
  borderRadius: 11,
  fontSize: 13.5,
  fontWeight: 600,
  cursor: disabled ? "default" : "pointer",
  transition: "all 120ms ease",
});

const progressStyle: CSSProperties = {
  display: "flex",
  gap: 5,
  marginTop: 18,
};

const dotStyle = (active: boolean, done: boolean): CSSProperties => ({
  height: 5,
  flex: 1,
  borderRadius: 3,
  background: active ? ACCENT : done ? "#3A4742" : "#242A27",
  transition: "background 200ms ease",
});

const checkIconStyle: CSSProperties = {
  color: ACCENT,
  fontSize: 16,
  fontWeight: 800,
};

const regionHeaderStyle: CSSProperties = {
  fontSize: 11,
  fontWeight: 700,
  letterSpacing: "0.12em",
  textTransform: "uppercase",
  color: TEXT_MUTED,
  margin: "12px 0 6px",
};

function signalBars(signal: number): string {
  if (signal >= 80) return "▂▄▆█";
  if (signal >= 55) return "▂▄▆ ";
  if (signal >= 30) return "▂▄  ";
  if (signal >= 1) return "▂   ";
  return "    ";
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------
function SetupApp() {
  const [step, setStep] = useState(0);
  const [language, setLanguage] = useState("en_US.UTF-8");
  const [keyboard, setKeyboard] = useState("us");
  const [timezone, setTimezone] = useState("America/New_York");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [wifiSsid, setWifiSsid] = useState("");
  const [wifiPass, setWifiPass] = useState("");
  const [networks, setNetworks] = useState<WifiNet[]>([]);
  const [scanning, setScanning] = useState(false);
  const [tzFilter, setTzFilter] = useState("");
  const [applying, setApplying] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState(false);

  const activeLang = useMemo(() => LANGUAGES.find((l) => l.id === language), [language]);
  const selectedSecurity = useMemo(
    () => networks.find((n) => n.ssid === wifiSsid)?.security ?? "",
    [networks, wifiSsid],
  );

  // Auto-scan Wi-Fi the moment the Network step is shown — no button press.
  const scanWifi = useCallback(async () => {
    setScanning(true);
    try {
      const res = await fetch("http://127.0.0.1:7860/api/v1/setup/wifi/scan");
      if (!res.ok) throw new Error("scan failed");
      const body = await res.json().catch(() => ({}));
      setNetworks(Array.isArray(body?.networks) ? body.networks : []);
    } catch {
      setNetworks([]);
    } finally {
      setScanning(false);
    }
  }, []);

  useEffect(() => {
    if (step === 4 && networks.length === 0 && !scanning) {
      scanWifi();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [step]);

  const pickLanguage = useCallback((id: string) => {
    setLanguage(id);
    const lang = LANGUAGES.find((l) => l.id === id);
    if (lang) {
      setKeyboard(lang.kb);
      setTimezone(lang.tz);
    }
  }, []);

  const pickNetwork = useCallback((ssid: string, security: string) => {
    setWifiSsid(ssid);
    // Open networks need no password; secured ones reveal the field.
    if (security === "") setWifiPass("");
  }, []);

  const next = useCallback(() => setStep((s) => Math.min(s + 1, STEPS.length - 1)), []);
  const back = useCallback(() => setStep((s) => Math.max(s - 1, 0)), []);

  const submit = useCallback(async () => {
    setApplying(true);
    setError(null);
    const payload = {
      language,
      keyboard,
      timezone,
      username: username.trim(),
      password,
      wifi_ssid: wifiSsid.trim(),
      wifi_password: wifiPass,
    };
    try {
      const res = await fetch("http://127.0.0.1:7860/api/v1/setup/apply", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });
      const body = await res.json().catch(() => ({}));
      if (!res.ok) {
        throw new Error(body?.message || `Something went wrong (${res.status})`);
      }
      if (body && body.ok === false) {
        throw new Error(body.message || "Setup didn't finish");
      }
      setDone(true);
      // The daemon wrote /var/lib/diad/setup-complete; the kiosk launcher will
      // also close us once it sees the marker — this is a fast-fail fallback.
      window.setTimeout(() => window.close(), 900);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setApplying(false);
    }
  }, [language, keyboard, timezone, username, password, wifiSsid, wifiPass]);

  const onPrimary = useCallback(() => {
    if (step === STEPS.length - 1) submit();
    else next();
  }, [step, submit, next]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (done || applying) return;
      if (e.key === "Enter") onPrimary();
      if (e.key === "Escape") back();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onPrimary, back, done, applying]);

  if (done) {
    return (
      <div style={pageStyle}>
        <div style={cardStyle}>
          <div style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 14, padding: "10px 0" }}>
            <div style={{
              width: 56, height: 56, borderRadius: "50%",
              background: "rgba(74, 222, 128, 0.14)", border: `1px solid ${SUCCESS}`,
              color: SUCCESS, display: "flex", alignItems: "center", justifyContent: "center",
              fontSize: 30, fontWeight: 700,
            }}>✓</div>
            <div style={{ fontSize: 15, fontWeight: 700 }}>You're all set</div>
            <div style={{ fontSize: 12.5, color: TEXT_MUTED, textAlign: "center", lineHeight: 1.5 }}>
              Starting your desktop…
            </div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div style={pageStyle}>
      <div style={cardStyle}>
        <img src={logoWhite} alt="" style={logoStyle} />

        <div style={progressStyle}>
          {STEPS.map((_, i) => (
            <div key={i} style={dotStyle(i === step, i < step)} />
          ))}
        </div>

        <div style={stepLabelStyle}>Step {step + 1} of {STEPS.length}</div>

        <AnimatePresence mode="wait">
          <motion.div
            key={step}
            initial={{ opacity: 0, x: 18 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -18 }}
            transition={{ duration: 0.16 }}
          >
            {/* ---- Language ---- */}
            {step === 0 && (
              <>
                <div style={titleStyle}>Choose your language</div>
                <div style={bodyStyle}>This sets the language, keyboard and clock for this computer.</div>
                <div style={listStyle}>
                  {LANGUAGES.map((l) => (
                    <div key={l.id} style={optionStyle(language === l.id)} onClick={() => pickLanguage(l.id)}>
                      <span>{l.label}</span>
                      {language === l.id && <span style={checkIconStyle}>✓</span>}
                    </div>
                  ))}
                </div>
              </>
            )}

            {/* ---- Keyboard ---- */}
            {step === 1 && (
              <>
                <div style={titleStyle}>Keyboard layout</div>
                <div style={bodyStyle}>Pick the layout that matches your keyboard.</div>
                <div style={listStyle}>
                  {KEYBOARDS.map((k) => (
                    <div key={k.id} style={optionStyle(keyboard === k.id)} onClick={() => setKeyboard(k.id)}>
                      <span>{k.label}</span>
                      {keyboard === k.id && <span style={checkIconStyle}>✓</span>}
                    </div>
                  ))}
                </div>
              </>
            )}

            {/* ---- Time zone ---- */}
            {step === 2 && (
              <>
                <div style={titleStyle}>Time zone</div>
                <div style={bodyStyle}>Used for the clock and every timestamp.</div>
                <input
                  style={inputStyle}
                  value={tzFilter}
                  placeholder="Search a city or region…"
                  autoFocus
                  onChange={(e) => setTzFilter(e.target.value)}
                />
                <div style={{ marginTop: 10, maxHeight: 300, overflowY: "auto" }}>
                  {TIMEZONE_GROUPS.map((g) => {
                    const zones = g.zones.filter(
                      (z) =>
                        tzFilter.trim() === "" ||
                        z.label.toLowerCase().includes(tzFilter.toLowerCase()) ||
                        z.id.toLowerCase().includes(tzFilter.toLowerCase()) ||
                        g.region.toLowerCase().includes(tzFilter.toLowerCase()),
                    );
                    if (zones.length === 0) return null;
                    return (
                      <div key={g.region}>
                        <div style={regionHeaderStyle}>{g.region}</div>
                        <div style={listStyle}>
                          {zones.map((z) => (
                            <div key={z.id} style={optionStyle(timezone === z.id)} onClick={() => setTimezone(z.id)}>
                              <span>{z.label}</span>
                              {timezone === z.id && <span style={checkIconStyle}>✓</span>}
                            </div>
                          ))}
                        </div>
                      </div>
                    );
                  })}
                </div>
              </>
            )}

            {/* ---- Account ---- */}
            {step === 3 && (
              <>
                <div style={titleStyle}>Create your account</div>
                <div style={bodyStyle}>Leave both blank to sign in automatically. Otherwise set a name and password.</div>
                <div style={fieldLabelStyle}>Username</div>
                <input
                  style={inputStyle}
                  value={username}
                  placeholder="diad (default)"
                  autoFocus
                  onChange={(e) => setUsername(e.target.value)}
                  maxLength={32}
                />
                <div style={fieldLabelStyle}>Password</div>
                <input
                  style={inputStyle}
                  type="password"
                  value={password}
                  placeholder="Leave blank for none"
                  onChange={(e) => setPassword(e.target.value)}
                  maxLength={128}
                />
                <div style={hintStyle}>
                  {username.trim() === "" && password === ""
                    ? "You'll go straight to the desktop without signing in."
                    : "You'll sign in each time you start this computer."}
                </div>
              </>
            )}

            {/* ---- Wi-Fi ---- */}
            {step === 4 && (
              <>
                <div style={titleStyle}>Wi-Fi</div>
                <div style={bodyStyle}>
                  {scanning
                    ? "Looking for networks…"
                    : networks.length === 0
                      ? "No networks found. You can skip this and connect later."
                      : "Networks near you. Pick one, or continue without Wi-Fi."}
                </div>

                {scanning && <div style={hintStyle}>Scanning automatically…</div>}

                {!scanning && networks.length > 0 && (
                  <div style={{ maxHeight: 230, overflowY: "auto" }}>
                    <div style={listStyle}>
                      {networks.map((n) => {
                        const active = wifiSsid === n.ssid;
                        const secured = n.security !== "";
                        return (
                          <div key={n.ssid} style={optionStyle(active)} onClick={() => pickNetwork(n.ssid, n.security)}>
                            <span style={{ display: "flex", alignItems: "center", gap: 10 }}>
                              <span style={{ fontFamily: "monospace", color: active ? ACCENT : TEXT_MUTED, width: 44, letterSpacing: 1 }}>
                                {signalBars(n.signal)}
                              </span>
                              <span>{n.ssid}</span>
                            </span>
                            <span style={{ display: "flex", alignItems: "center", gap: 8 }}>
                              {secured && <span style={{ fontSize: 13 }}>🔒</span>}
                              {active && <span style={checkIconStyle}>✓</span>}
                            </span>
                          </div>
                        );
                      })}
                    </div>
                  </div>
                )}

                {!scanning && networks.length === 0 && (
                  <div
                    style={{ ...optionStyle(false), justifyContent: "center", cursor: "pointer" }}
                    onClick={scanWifi}
                  >
                    <span style={{ color: ACCENT, fontWeight: 600 }}>Scan again</span>
                  </div>
                )}

                {wifiSsid !== "" && selectedSecurity !== "" && (
                  <>
                    <div style={fieldLabelStyle}>Password for {wifiSsid}</div>
                    <input
                      style={inputStyle}
                      type="password"
                      value={wifiPass}
                      placeholder="Network password"
                      autoFocus
                      onChange={(e) => setWifiPass(e.target.value)}
                      maxLength={128}
                    />
                  </>
                )}
                {wifiSsid !== "" && selectedSecurity === "" && (
                  <div style={hintStyle}>This network is open — no password needed.</div>
                )}
                <div style={hintStyle}>Your password goes only to this computer's own setup service.</div>
              </>
            )}

            {/* ---- Done ---- */}
            {step === 5 && (
              <>
                <div style={titleStyle}>Ready to go</div>
                <div style={bodyStyle}>Here's what will be set up.</div>
                <div style={listStyle}>
                  <SummaryRow label="Language" value={activeLang?.label ?? language} />
                  <SummaryRow label="Keyboard" value={KEYBOARDS.find((k) => k.id === keyboard)?.label ?? keyboard} />
                  <SummaryRow label="Time zone" value={timezone} />
                  <SummaryRow
                    label="Account"
                    value={username.trim() === "" ? "diad — sign in automatically" : `${username.trim()}${password ? " — password set" : " — no password"}`}
                  />
                  <SummaryRow label="Wi-Fi" value={wifiSsid.trim() === "" ? "Skip for now" : wifiSsid.trim()} />
                </div>
              </>
            )}
          </motion.div>
        </AnimatePresence>

        {error && (
          <div style={{ marginTop: 12, color: DANGER, fontSize: 12.5, lineHeight: 1.5 }}>⚠ {error}</div>
        )}

        <div style={buttonRowStyle}>
          <button style={ghostBtnStyle(step === 0 || applying)} onClick={back} disabled={step === 0 || applying}>
            {step === 0 ? " " : "← Back"}
          </button>
          <button style={primaryBtnStyle(applying)} onClick={onPrimary} disabled={applying}>
            {applying ? "Applying…" : step === STEPS.length - 1 ? "Finish" : "Continue"}
          </button>
        </div>
      </div>
    </div>
  );
}

function SummaryRow({ label, value }: { label: string; value: string }) {
  const rowStyle: CSSProperties = {
    display: "flex",
    justifyContent: "space-between",
    gap: 16,
    padding: "11px 15px",
    background: FIELD,
    border: `1px solid ${BORDER}`,
    borderRadius: 11,
    fontSize: 13.5,
  };
  return (
    <div style={rowStyle}>
      <span style={{ color: TEXT_MUTED, fontWeight: 600 }}>{label}</span>
      <span style={{ color: TEXT, fontWeight: 600, textAlign: "right" }}>{value}</span>
    </div>
  );
}

const root = createRoot(document.getElementById("root")!);
root.render(<SetupApp />);
