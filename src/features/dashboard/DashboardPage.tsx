import { useTranslation } from "react-i18next";
import { Link } from "react-router-dom";
import { ArrowRight, BarChart3, Clock, Files, ShieldCheck, Star } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { moduleById, QUICK_ACTIONS, TOOL_MODULES } from "@/app/modules";
import { PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { cn } from "@/lib/utils";

export function DashboardPage() {
  const { t } = useTranslation();
  return (
    <div className="pb-8">
      <PageHeader title={t("dashboard.welcome")} description={t("dashboard.subtitle")} />
      <div className="flex flex-col gap-8 px-8">
        <section aria-labelledby="quick-actions">
          <h2 id="quick-actions" className="mb-3 text-xs font-semibold uppercase tracking-wider text-text-secondary">
            {t("dashboard.quickActions")}
          </h2>
          <div className="grid grid-cols-5 gap-3">
            {QUICK_ACTIONS.map(({ key, module }) => {
              const mod = moduleById(module);
              const Icon = mod.icon;
              return (
                <Link
                  key={key}
                  to={mod.path}
                  className="group flex flex-col gap-4 rounded-card border border-border bg-card p-4 shadow-card transition-colors hover:border-accent/50 hover:bg-card-hover"
                >
                  <span
                    className={cn(
                      "flex size-10 items-center justify-center rounded-xl",
                      mod.accent === "accent" ? "bg-accent/15 text-accent" : "bg-accent-2/15 text-accent-2",
                    )}
                  >
                    <Icon className="size-5" />
                  </span>
                  <span className="flex items-center justify-between text-sm font-medium text-text">
                    {t(`dashboard.quick.${key}`)}
                    <ArrowRight className="size-4 -translate-x-1 text-text-secondary opacity-0 transition-all group-hover:translate-x-0 group-hover:opacity-100" />
                  </span>
                </Link>
              );
            })}
          </div>
        </section>

        <div className="grid grid-cols-2 gap-4">
          <EmptyCard icon={Clock} title={t("dashboard.recentProjects")} body={t("dashboard.recentProjectsEmpty")} />
          <EmptyCard icon={Files} title={t("dashboard.recentFiles")} body={t("dashboard.recentFilesEmpty")} />
          <EmptyCard icon={BarChart3} title={t("dashboard.statistics")} body={t("dashboard.statisticsEmpty")} />
          <EmptyCard icon={Star} title={t("dashboard.favoritePresets")} body={t("dashboard.favoritePresetsEmpty")} />
        </div>

        <section aria-labelledby="all-modules">
          <h2 id="all-modules" className="mb-3 text-xs font-semibold uppercase tracking-wider text-text-secondary">
            {t("dashboard.allModules")}
          </h2>
          <div className="grid grid-cols-4 gap-3">
            {TOOL_MODULES.map((m) => {
              const Icon = m.icon;
              return (
                <Link
                  key={m.id}
                  to={m.path}
                  className="flex items-start gap-3 rounded-card border border-border bg-card/60 p-4 transition-colors hover:bg-card-hover"
                >
                  <Icon className={cn("mt-0.5 size-4 shrink-0", m.accent === "accent" ? "text-accent" : "text-accent-2")} />
                  <span>
                    <span className="block text-sm font-medium text-text">{t(`nav.${m.id}`)}</span>
                    <span className="mt-1 block text-xs leading-relaxed text-text-secondary">{t(`modules.${m.id}`)}</span>
                  </span>
                </Link>
              );
            })}
          </div>
        </section>

        <p className="flex items-center gap-2 text-xs text-text-secondary">
          <ShieldCheck className="size-4 text-accent-2" />
          {t("dashboard.privacy")}
        </p>
      </div>
    </div>
  );
}

function EmptyCard({ icon: Icon, title, body }: { icon: LucideIcon; title: string; body: string }) {
  return (
    <Card>
      <CardHeader className="flex-row items-center gap-2">
        <Icon className="size-4 text-text-secondary" />
        <CardTitle>{title}</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="flex h-24 items-center justify-center rounded-lg border border-dashed border-border">
          <CardDescription>{body}</CardDescription>
        </div>
      </CardContent>
    </Card>
  );
}
