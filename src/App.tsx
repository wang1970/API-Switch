import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import {
  Layers,
  Route,
  FileText,
  KeyRound,
  BarChart3,
  Settings,
  Power,
  ExternalLink,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { ApiPoolPage } from "@/pages/ApiPoolPage";
import { ChannelPage } from "@/pages/ChannelPage";
import { TokenPage } from "@/pages/TokenPage";
import { LogPage } from "@/pages/LogPage";
import { DashboardPage } from "@/pages/DashboardPage";
import { SettingsPage } from "@/pages/SettingsPage";
import { WelcomeGuide } from "@/components/WelcomeGuide";
import { useQuery } from "@tanstack/react-query";
import { getSettings, updateSettings, checkUpdate } from "@/lib/api";

type Page = "apiPool" | "channels" | "tokens" | "logs" | "dashboard" | "settings";

const NAV_ITEMS: { key: Page; icon: typeof Layers; labelKey: string }[] = [
  { key: "apiPool", icon: Layers, labelKey: "nav.apiPool" },
  { key: "channels", icon: Route, labelKey: "nav.channels" },
  { key: "tokens", icon: KeyRound, labelKey: "nav.tokens" },
  { key: "logs", icon: FileText, labelKey: "nav.logs" },
  { key: "dashboard", icon: BarChart3, labelKey: "nav.dashboard" },
  { key: "settings", icon: Settings, labelKey: "nav.settings" },
];

export default function App() {
  const { t, i18n } = useTranslation();
  const [currentPage, setCurrentPage] = useState<Page>("apiPool");

  const { data: settings } = useQuery({
    queryKey: ["settings"],
    queryFn: getSettings,
  });

  const [guideOpen, setGuideOpen] = useState(true);
  const [updateInfo, setUpdateInfo] = useState<{ current: string; latest: string; url: string } | null>(null);

  // Check for updates on startup
  useEffect(() => {
    let mounted = true;
    (async () => {
      const unlisten = await listen<{ current: string; latest: string; url: string }>(
        "update-available",
        (event) => {
          if (mounted) setUpdateInfo(event.payload);
        }
      );
      // Now listener is ready, check for updates
      await checkUpdate();
      // Cleanup
      return () => {
        mounted = false;
        unlisten();
      };
    })();
  }, []);

  // Apply locale and theme
  useEffect(() => {
    if (!settings) return;

    // Apply locale from DB
    const saved = localStorage.getItem("api-switch-locale");
    if (!saved && settings.locale) {
      i18n.changeLanguage(settings.locale);
    }

    // Apply theme
    const root = document.documentElement;
    if (settings.theme === "dark") {
      root.classList.add("dark");
    } else if (settings.theme === "light") {
      root.classList.remove("dark");
    } else {
      if (window.matchMedia("(prefers-color-scheme: dark)").matches) {
        root.classList.add("dark");
      } else {
        root.classList.remove("dark");
      }
    }
  }, [settings]);

  const handleGuideDismiss = (dontShowAgain: boolean) => {
    if (dontShowAgain) {
      updateSettings({ ...settings!, show_guide: false });
    }
  };

  const renderPage = () => {
    switch (currentPage) {
      case "apiPool":
        return <ApiPoolPage />;
      case "channels":
        return <ChannelPage />;
      case "tokens":
        return <TokenPage />;
      case "logs":
        return <LogPage />;
      case "dashboard":
        return <DashboardPage />;
      case "settings":
        return <SettingsPage />;
    }
  };

  return (
    <div className="flex flex-col h-screen bg-background">
      {/* Update banner */}
      {updateInfo && (
        <div className="flex items-center justify-center gap-2 bg-primary/10 text-primary px-3 py-1.5 text-xs shrink-0">
          <span>
            {t("update.newVersion", { version: updateInfo.latest })}
          </span>
          <a
            href={updateInfo.url}
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-1 font-medium underline-offset-2 hover:underline"
          >
            {t("update.goDownload")}
            <ExternalLink className="h-3 w-3" />
          </a>
          <button
            onClick={() => setUpdateInfo(null)}
            className="ml-1 opacity-60 hover:opacity-100"
          >
            ✕
          </button>
        </div>
      )}
      <div className="flex flex-1 min-h-0">
      {/* Sidebar */}
      <aside className="flex w-56 flex-col border-r border-sidebar-border bg-sidebar-background">
        {/* Logo */}
        <div className="flex items-center gap-2 px-4 py-4">
          <Power className="h-5 w-5 text-primary" />
          <span className="text-lg font-semibold">API Switch</span>
        </div>

        <Separator />

        {/* Navigation */}
        <ScrollArea className="flex-1 px-2 py-2">
          <nav className="flex flex-col gap-1">
            {NAV_ITEMS.map(({ key, icon: Icon, labelKey }) => (
              <Button
                key={key}
                variant={currentPage === key ? "secondary" : "ghost"}
                className={cn(
                  "justify-start gap-2 px-3",
                  currentPage === key && "bg-sidebar-accent text-sidebar-accent-foreground"
                )}
                onClick={() => setCurrentPage(key)}
              >
                <Icon className="h-4 w-4" />
                {t(labelKey)}
              </Button>
            ))}
          </nav>
        </ScrollArea>

        {/* Star on GitHub */}
        <div className="flex justify-center pb-4">
          <a href="https://github.com/wang1970/API-Switch" target="_blank" rel="noopener noreferrer">
            <img src="/star.jpg" alt="Star on GitHub" className="cursor-pointer hover:opacity-80 transition-opacity" />
          </a>
        </div>
      </aside>

      {/* Main Content */}
      <main className="flex-1 overflow-auto">
        {renderPage()}
      </main>

      {/* Welcome Guide - show on first launch */}
      {settings?.show_guide !== false && (
        <WelcomeGuide
          open={guideOpen}
          onOpenChange={setGuideOpen}
          onDismiss={handleGuideDismiss}
        />
      )}
      </div>
    </div>
  );
}
