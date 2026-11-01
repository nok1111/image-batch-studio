import { useEffect } from "react";
import { createHashRouter, RouterProvider } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { AlertTriangle, Loader2 } from "lucide-react";
import { TOOL_MODULES } from "@/app/modules";
import { AppLayout } from "@/components/layout/AppLayout";
import { DashboardPage } from "@/features/dashboard/DashboardPage";
import { ModulePage } from "@/features/modules/ModulePage";
import { SettingsPage } from "@/features/settings/SettingsPage";
import { resolveLocale } from "@/i18n";
import { OnboardingWizard } from "@/features/onboarding/OnboardingWizard";
import { useLicenseStore } from "@/stores/license";
import { useSettingsStore } from "@/stores/settings";

const router = createHashRouter([
  {
    path: "/",
    element: <AppLayout />,
    children: [
      { index: true, element: <DashboardPage /> },
      ...TOOL_MODULES.map((m) => ({ path: m.path.slice(1), element: <ModulePage id={m.id} /> })),
      { path: "settings", element: <SettingsPage /> },
    ],
  },
]);

export default function App() {
  const { t, i18n } = useTranslation();
  const { status, error, settings, load } = useSettingsStore();

  useEffect(() => {
    void load();
    void useLicenseStore.getState().load();
  }, [load]);

  useEffect(() => {
    if (!settings) return;
    const locale = resolveLocale(settings.language, navigator.languages);
    if (i18n.language !== locale) void i18n.changeLanguage(locale);
    document.documentElement.lang = locale;
    document.documentElement.classList.toggle("dark", settings.theme === "dark");
    document.documentElement.classList.toggle("light", settings.theme === "light");
  }, [settings, i18n]);

  if (status === "error") {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 p-8 text-center">
        <AlertTriangle className="size-8 text-danger" />
        <p className="max-w-md text-sm text-text">{t("settings.loadError", { message: error })}</p>
      </div>
    );
  }
  if (!settings) {
    return (
      <div className="flex h-full items-center justify-center">
        <Loader2 className="size-6 animate-spin text-text-secondary" />
      </div>
    );
  }
  return settings.onboardingDone ? <RouterProvider router={router} /> : <OnboardingWizard />;
}
