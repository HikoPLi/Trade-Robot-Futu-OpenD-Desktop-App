import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./i18n/en.json";
import zhCN from "./i18n/zh-CN.json";
import zhTW from "./i18n/zh-TW.json";

const STORAGE_KEY = "tr.locale";
export const SUPPORTED_LANGUAGES = ["en", "zh-CN", "zh-TW"] as const;
export type SupportedLanguage = (typeof SUPPORTED_LANGUAGES)[number];

function normalize(lang: string): SupportedLanguage {
  const l = (lang || "").replace("_", "-");
  const lower = l.toLowerCase();
  if (lower === "zh-cn" || lower === "zh-hans" || lower.startsWith("zh-cn") || lower.startsWith("zh-hans")) {
    return "zh-CN";
  }
  if (lower === "zh-tw" || lower === "zh-hant" || lower.startsWith("zh-tw") || lower.startsWith("zh-hant")) {
    return "zh-TW";
  }
  if (lower.startsWith("zh")) {
    // Default Chinese to Simplified unless explicitly Traditional.
    return "zh-CN";
  }
  return "en";
}

export function getPreferredLanguage(): SupportedLanguage {
  try {
    const saved = window.localStorage.getItem(STORAGE_KEY);
    if (saved && (SUPPORTED_LANGUAGES as readonly string[]).includes(saved)) return saved as SupportedLanguage;
  } catch {
    // ignore
  }
  return normalize(navigator.language);
}

export async function setPreferredLanguage(lang: SupportedLanguage) {
  try {
    window.localStorage.setItem(STORAGE_KEY, lang);
  } catch {
    // ignore
  }
  await i18n.changeLanguage(lang);
}

void i18n.use(initReactI18next).init({
  resources: {
    en: { translation: en },
    "zh-CN": { translation: zhCN },
    "zh-TW": { translation: zhTW }
  },
  lng: getPreferredLanguage(),
  fallbackLng: "en",
  interpolation: { escapeValue: false }
});

export default i18n;

