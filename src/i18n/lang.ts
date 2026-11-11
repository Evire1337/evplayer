// The language of the interface. The texts are written in Russian in the code; English comes
// from a dictionary (./en) that the page applies to what is on the screen (./dom). The setting
// is "auto" (the language of the system), "ru" or "en"; more languages would be more
// dictionaries and more entries here.

export type Language = "auto" | "ru" | "en";
export type Lang = "ru" | "en";

export const LANGUAGES: ReadonlyArray<{ id: Language; label: string; english: string }> = [
  { id: "auto", label: "Как в системе", english: "Same as the system" },
  { id: "ru", label: "Русский", english: "Russian" },
  { id: "en", label: "English", english: "English" },
];

/** people who read Russian, whatever their system language (Ukrainian, Belarusian, Kazakh) */
const RUSSIAN_READERS = ["ru", "uk", "be", "kk"];

/** the language of the system: Russian where it is a Russian-reading one, English elsewhere */
export function systemLang(tag: string = typeof navigator !== "undefined" ? navigator.language : "ru"): Lang {
  const base = (tag || "ru").toLowerCase().split("-")[0];
  return RUSSIAN_READERS.includes(base) ? "ru" : "en";
}

export function resolveLang(setting: Language, tag?: string): Lang {
  return setting === "auto" ? systemLang(tag) : setting;
}

/** locale for dates and numbers, by language */
export const localeOf = (lang: Lang): string => (lang === "en" ? "en-GB" : "ru");

let current: Lang = "ru";

/** the language on the screen now (dates and numbers read it) */
export const currentLang = (): Lang => current;
export const setCurrentLang = (lang: Lang): void => {
  current = lang;
};
