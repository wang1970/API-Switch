import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import zh from "./locales/zh.json";
import en from "./locales/en.json";

function detectLocale(): string {
  // 1. 用户之前选择的
  const saved = localStorage.getItem("api-switch-locale");
  if (saved === "zh" || saved === "en") return saved;
  // 2. 系统语言
  const nav = navigator.language || "";
  if (nav.startsWith("zh")) return "zh";
  // 3. 非中文一律英文
  return "en";
}

i18n.use(initReactI18next).init({
  resources: {
    zh: { translation: zh },
    en: { translation: en },
  },
  lng: detectLocale(),
  fallbackLng: "en",
  interpolation: {
    escapeValue: false,
  },
});

export default i18n;
