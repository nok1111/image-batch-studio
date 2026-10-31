import { useTranslation } from "react-i18next";
import { Hammer } from "lucide-react";
import { moduleById, type ModuleId } from "@/app/modules";
import { PageHeader } from "@/components/layout/PageHeader";
import { Card } from "@/components/ui/card";
import { Workspace } from "@/features/workspace/Workspace";

/** Shell for a tool module. Modules render a notice until they are implemented and tested. */
export function ModulePage({ id }: { id: ModuleId }) {
  const { t } = useTranslation();
  const mod = moduleById(id);
  const Icon = mod.icon;
  if (mod.available) {
    return (
      <div className="flex h-full min-h-0 flex-col">
        <PageHeader title={t(`nav.${id}`)} description={t(`modules.${id}`)} />
        <Workspace />
      </div>
    );
  }
  return (
    <div>
      <PageHeader title={t(`nav.${id}`)} description={t(`modules.${id}`)} />
      <div className="px-8 pb-8">
        <Card className="flex flex-col items-center gap-3 px-8 py-16 text-center">
          <div className="relative">
            <Icon className="size-10 text-text-secondary/60" />
            <Hammer className="absolute -bottom-1 -right-2 size-5 text-accent" />
          </div>
          <h2 className="text-base font-semibold text-text">{t("modules.unavailableTitle")}</h2>
          <p className="max-w-md text-sm text-text-secondary">{t("modules.unavailableBody")}</p>
        </Card>
      </div>
    </div>
  );
}
