import { useStore } from "./stores/useStore";
import { useWebSocket } from "./hooks/useWebSocket";
import { TopBar } from "./components/TopBar";
import { ActivityBar } from "./components/ActivityBar";
import { ChatPanel } from "./panels/ChatPanel";
import { ExplorerPanel } from "./panels/ExplorerPanel";
import { EditorPanel } from "./panels/EditorPanel";
import { TerminalPanel } from "./panels/TerminalPanel";
import { AgentsPanel } from "./panels/AgentsPanel";
import { SettingsPanel } from "./panels/SettingsPanel";
import { GitPanel } from "./panels/GitPanel";
import { DebugPanel } from "./panels/DebugPanel";
import { PreviewPanel } from "./panels/PreviewPanel";
import { ProjectsPanel } from "./panels/ProjectsPanel";
import { StatusBar } from "./components/StatusBar";
import { CommandPalette } from "./components/CommandPalette";
import { ToastStack } from "./components/ToastStack";
import { OnboardingWizard } from "./panels/OnboardingWizard";
import { MaskedInputModal } from "./components/MaskedInputModal";
import { ConfirmModal } from "./components/ConfirmModal";
import { useGlobalKeys } from "./hooks/useGlobalKeys";

/**
 * DIAD AGENT — the graphical face of the DIAD intent-driven OS.
 * Speaks protocol v1 over WebSocket to the diad-agent daemon.
 */
export default function App() {
  useWebSocket();
  useGlobalKeys();

  const view = useStore((s) => s.view);
  const activityBarCollapsed = useStore((s) => s.activityBarCollapsed);
  const pendingSecret = useStore((s) => s.pendingSecret);
  const pendingConfirm = useStore((s) => s.pendingConfirm);
  const onboarding = useStore((s) => s.onboarding);
  const paletteOpen = useStore((s) => s.paletteOpen);

  return (
    <div className="flex h-full flex-col bg-obsidian text-ink">
      <TopBar />
      <div className="flex min-h-0 flex-1">
        <ActivityBar collapsed={activityBarCollapsed} />
        <main className="min-w-0 flex-1 overflow-hidden">
          {view === "chat" && <ChatPanel />}
          {view === "explorer" && <ExplorerPanel />}
          {view === "editor" && <EditorPanel />}
          {view === "terminal" && <TerminalPanel />}
          {view === "agents" && <AgentsPanel />}
          {view === "settings" && <SettingsPanel />}
          {view === "git" && <GitPanel />}
          {view === "debug" && <DebugPanel />}
          {view === "preview" && <PreviewPanel />}
          {view === "projects" && <ProjectsPanel />}
        </main>
      </div>
      <StatusBar />

      {/* Overlays */}
      {paletteOpen && <CommandPalette />}
      <ToastStack />
      {onboarding && <OnboardingWizard />}
      {pendingSecret && <MaskedInputModal />}
      {pendingConfirm && <ConfirmModal />}
    </div>
  );
}
