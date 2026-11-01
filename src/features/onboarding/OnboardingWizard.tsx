import { useState } from "react";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import { ArrowLeft, ArrowRight, FolderOpen, Images, KeyRound, X } from "lucide-react";
import { TOOL_MODULES } from "@/app/modules";
import { Button } from "@/components/ui/button";
import { Segmented } from "@/components/ui/segmented";
import { LOCALE_NATIVE_NAMES, SUPPORTED_LOCALES } from "@/i18n";
import { cn } from "@/lib/utils";
import { useLicenseStore } from "@/stores/license";
import { useSettingsStore } from "@/stores/settings";

const STEPS = 3;

export function OnboardingWizard() {
  const { t } = useTranslation();
  const { settings, update, saving, error } = useSettingsStore();
  const license = useLicenseStore((s) => s.status);
  const [step, setStep] = useState(0);

  if (!settings) return null;
  const finish = () => void update({ onboardingDone: true });

  const pickOutputDir = async () => {
    const dir = await open({
      directory: true,
      multiple: false,
      defaultPath: settings.defaultOutputDir ?? undefined,
    });
    if (typeof dir === "string") await update({ defaultOutputDir: dir });
  };

  return (
    <div className="flex h-full flex-col items-center justify-center bg-bg px-8">
      <div className="flex w-full max-w-2xl flex-1 flex-col justify-center gap-8 py-10">
        {step === 0 && (
          <div className="flex flex-col items-center gap-5 text-center">
            <span className="flex size-16 items-center justify-center rounded-2xl bg-accent/15 text-accent">
              <Images className="size-8" />
            </span>
            <div>
              <h1 className="text-2xl font-semibold text-text">{t("onboarding.welcomeTitle")}</h1>
              <p className="mt-2 text-sm text-text-secondary">{t("app.tagline")}</p>
            </div>
            {license && (
              <span
                className={cn(
                  "inline-flex items-center gap-1.5 rounded-full border px-3 py-1 text-xs font-medium",
                  license.state === "licensed"
                    ? "border-success/40 text-success"
                    : license.state === "expired"
                      ? "border-danger/40 text-danger"
                      : "border-warning/40 text-warning",
                )}
              >
                <KeyRound className="size-3.5" />
                {license.state === "licensed"
                  ? t("license.licensed")
                  : license.state === "expired"
                    ? t("license.expired")
                    : t("license.trialDays", { count: license.daysLeft })}
              </span>
            )}
          </div>
        )}

        {step === 1 && (
          <div className="flex flex-col gap-6">
            <h1 className="text-xl font-semibold text-text">{t("onboarding.prefsTitle")}</h1>
            <Field label={t("settings.language")}>
              <Segmented
                aria-label={t("settings.language")}
                value={settings.language}
                onValueChange={(language) => void update({ language })}
                options={[
                  { value: "system", label: t("settings.languageSystem") },
                  ...SUPPORTED_LOCALES.map((l) => ({ value: l, label: LOCALE_NATIVE_NAMES[l] })),
                ]}
              />
            </Field>
            <Field label={t("settings.theme")}>
              <Segmented
                aria-label={t("settings.theme")}
                value={settings.theme}
                onValueChange={(theme) => void update({ theme })}
                options={[
                  { value: "dark", label: t("settings.themeDark") },
                  { value: "light", label: t("settings.themeLight") },
                ]}
              />
            </Field>
            <Field label={t("settings.outputDir")}>
              <div className="flex items-center gap-2">
                <Button variant="secondary" onClick={() => void pickOutputDir()}>
                  <FolderOpen /> {t("settings.choose")}
                </Button>
                <span className="truncate text-xs text-text-secondary">
                  {settings.defaultOutputDir ?? t("settings.outputDirNone")}
                </span>
                {settings.defaultOutputDir && (
                  <Button
                    variant="ghost"
                    size="icon"
                    aria-label={t("settings.clear")}
                    onClick={() => void update({ defaultOutputDir: null })}
                  >
                    <X />
                  </Button>
                )}
              </div>
            </Field>
          </div>
        )}

        {step === 2 && (
          <div className="flex flex-col gap-5">
            <div>
              <h1 className="text-xl font-semibold text-text">{t("onboarding.tourTitle")}</h1>
              <p className="mt-1 text-sm text-text-secondary">{t("onboarding.tourBody")}</p>
            </div>
            <div className="grid max-h-72 grid-cols-3 gap-2 overflow-y-auto pr-1">
              {TOOL_MODULES.filter((m) => m.available).map((m) => {
                const Icon = m.icon;
                return (
                  <div key={m.id} className="rounded-lg border border-border bg-card p-3">
                    <Icon
                      className={cn(
                        "mb-2 size-4",
                        m.accent === "accent" ? "text-accent" : "text-accent-2",
                      )}
                    />
                    <p className="text-xs font-medium text-text">{t(`nav.${m.id}`)}</p>
                    <p className="mt-1 line-clamp-3 text-[11px] leading-snug text-text-secondary">
                      {t(`modules.${m.id}`)}
                    </p>
                  </div>
                );
              })}
            </div>
            <p className="text-xs text-text-secondary">{t("onboarding.licenseNote")}</p>
          </div>
        )}

        <div className="flex items-center justify-between">
          {error && (
            <p role="alert" className="mr-4 text-xs text-danger">
              {t("settings.saveError", { message: error })}
            </p>
          )}
          <Button variant="ghost" onClick={finish} disabled={saving}>
            {t("onboarding.skip")}
          </Button>
          <div className="flex items-center gap-4">
            <div className="flex gap-1.5" aria-hidden="true">
              {Array.from({ length: STEPS }, (_, i) => (
                <span
                  key={i}
                  className={cn(
                    "size-1.5 rounded-full",
                    i === step ? "bg-accent" : "bg-border",
                  )}
                />
              ))}
            </div>
            {step > 0 && (
              <Button variant="secondary" onClick={() => setStep(step - 1)} disabled={saving}>
                <ArrowLeft /> {t("onboarding.back")}
              </Button>
            )}
            {step < STEPS - 1 ? (
              <Button onClick={() => setStep(step + 1)}>
                {t("onboarding.next")} <ArrowRight />
              </Button>
            ) : (
              <Button onClick={finish} disabled={saving}>
                {t("onboarding.start")} <ArrowRight />
              </Button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-6">
      <span className="text-sm text-text">{label}</span>
      {children}
    </div>
  );
}
