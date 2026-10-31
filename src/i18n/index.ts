import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import type { Language } from "@/lib/ipc/bindings/Language";
import en from "./locales/en.json";
import es from "./locales/es.json";
import ptBR from "./locales/pt-BR.json";

export const resources = {
  en: { translation: en },
  es: { translation: es },
  "pt-BR": { translation: ptBR },
} as const;

export type SupportedLocale = keyof typeof resources;
export const SUPPORTED_LOCALES = Object.keys(resources) as SupportedLocale[];

/** Language names are shown in their own language, never translated. */
export const LOCALE_NATIVE_NAMES: Record<SupportedLocale, string> = {
  en: "English",
  es: "Español",
  "pt-BR": "Português (Brasil)",
};

export function resolveLocale(pref: Language, systemLanguages: readonly string[]): SupportedLocale {
  if (pref !== "system") return pref;
  for (const tag of systemLanguages) {
    const lower = tag.toLowerCase();
    if (lower.startsWith("pt")) return "pt-BR";
    if (lower.startsWith("es")) return "es";
    if (lower.startsWith("en")) return "en";
  }
  return "en";
}

void i18n.use(initReactI18next).init({
  resources,
  lng: resolveLocale("system", typeof navigator === "undefined" ? [] : navigator.languages),
  fallbackLng: "en",
  interpolation: { escapeValue: false },
  returnNull: false,
});

export default i18n;
