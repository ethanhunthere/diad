import { useEffect } from "react";

import { useGlobalShortcuts } from "./hooks";
import { useStore } from "./store";

import TopBar from "./components/TopBar";
import ActivityBar from "./components/ActivityBar";
import StatusBar from "./components/StatusBar";
import ChatPanel from "./components/ChatPanel";
import Explorer from "./components/Explorer";
import Editor from "./components/Editor";
import TerminalPanel from "./components/TerminalPanel";
import AgentsView from "./components/AgentsView";
import SettingsView from "./components/SettingsView";
import CommandPalette from "./components/CommandPalette";
import ProviderPicker from "./components/ProviderPicker";
import Toasts from "./components/Toasts";
import { ConfirmModal, SecretModal } from "./components/modals";
import Onboarding from "./components/Onboarding";

const ONBOARD_KEY = "diad-agent.onboarded.v1";

function MainArea() {
  const view = useStore((s) => s.view);
  const sidebarOpen = useStore((s) => s.sidebarOpen);

  switch (view) {
    case "chat":
      return <ChatPanel />;
    case "explorer":
      return (
        <div className="flex min-h-0 w-full">
          {sidebarOpen && <Explorer />}
          <Editor />
        </div>
      );
    case "editor":
      return <Editor />;
    case "terminal":
      return <TerminalPanel />;
    case "agents":
      return <AgentsView />;
    case "settings":
      return <SettingsView />;
    default:
      return <ChatPanel />;
  }
}

export default function App() {
  useGlobalShortcuts();

  const providers = useStore((s) => s.providers);
  const setOnboarding = useStore((s) => s.setOnboarding);

  // First-launch onboarding: show once providers have been fetched.
  useEffect(() => {
    if (providers.length && !localStorage.getItem(ONBOARD_KEY)) {
      setOnboarding(true);
    }
  }, [providers, setOnboarding]);

  return (
    <div className="flex h-full flex-col bg-obsidian">
      <TopBar />
      <div className="flex min-h-0 flex-1">
        <ActivityBar />
        <main className="flex min-h-0 min-w-0 flex-1 flex-col">
          <MainArea />
        </main>
      </div>
      <StatusBar />

      {/* overlays */}
      <CommandPalette />
      <ProviderPicker />
      <SecretModal />
      <ConfirmModal />
      <Toasts />
      <Onboarding />
    </div>
  );
}
