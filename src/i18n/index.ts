import de, { type TranslationKey } from "./de";

const translations: Record<string, Record<string, string>> = { de };

let currentLocale = "de";

export function t(key: TranslationKey, params?: Record<string, string | number>): string {
  let value = translations[currentLocale]?.[key] ?? key;
  if (params) {
    for (const [k, v] of Object.entries(params)) {
      value = value.replace(`{${k}}`, String(v));
    }
  }
  return value;
}

export function setLocale(locale: string) {
  currentLocale = locale;
}

export { type TranslationKey };
