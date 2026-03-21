import { t, type TranslationKey } from "../i18n";

export function useTranslation() {
  return {
    t: (key: TranslationKey, params?: Record<string, string | number>) =>
      t(key, params),
  };
}
