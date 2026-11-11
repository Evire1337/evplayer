import { afterEach, describe, expect, it } from "vitest";
import { useSettings } from "@/settings/store";
import { startTranslator } from "./dom";
import { EN } from "./en";
import { localeOf, resolveLang, systemLang } from "./lang";
import { translate } from "./translate";

describe("language", () => {
  it("follows the system: Russian for people who read it, English elsewhere", () => {
    expect(systemLang("ru-RU")).toBe("ru");
    expect(systemLang("uk")).toBe("ru");
    expect(systemLang("pl-PL")).toBe("en");
    expect(systemLang("en-US")).toBe("en");
    expect(resolveLang("auto", "de-DE")).toBe("en");
    expect(resolveLang("ru", "de-DE")).toBe("ru");
    expect(resolveLang("en", "ru-RU")).toBe("en");
    expect(localeOf("en")).toBe("en-GB");
  });
});

describe("translating a text", () => {
  it("leaves Russian alone and finds the English of a known text, keeping its spaces", () => {
    expect(translate("Главная", "ru")).toBe("Главная");
    expect(translate("Главная", "en")).toBe("Home");
    expect(translate("  Поиск ", "en")).toBe("  Search ");
    expect(translate("Search", "en")).toBe("Search");
  });

  it("writes counts, names in quotes and dotted lines in English", () => {
    expect(translate("1 трек", "en")).toBe("1 track");
    expect(translate("6 треков", "en")).toBe("6 tracks");
    expect(translate("Плейлист · 6 треков", "en")).toBe("Playlist · 6 tracks");
    expect(translate("Добавлено в «Любимое»", "en")).toBe('Added to "Любимое"');
    expect(translate("Понравившиеся · 9 треков", "en")).toBe("Liked songs · 9 tracks");
    expect(translate("· создан 10 Nov 2026", "en")).toBe("· created 10 Nov 2026");
    expect(translate("12 МБ на диске", "en")).toBe("12 MB on the disk");
  });

  it("keeps a Russian text that has no translation, as it is", () => {
    expect(translate("Что-то совсем новое", "en")).toBe("Что-то совсем новое");
  });
});

describe("the dictionary", () => {
  it("has English on the right, and something to translate on the left", () => {
    for (const [ru, en] of Object.entries(EN)) {
      expect(en, ru).not.toBe("");
      expect(/[А-Яа-яЁё]/.test(ru), ru).toBe(true);
      // a Russian word on the English side is a slip (names the app shows in Russian are not here)
      expect(/[А-Яа-яЁё]/.test(en.replace(/«[^»]*»|"[^"]*"/g, "")), `${ru} → ${en}`).toBe(false);
    }
  });
});

describe("the page in English", () => {
  let stop: (() => void) | null = null;
  afterEach(() => {
    stop?.();
    useSettings.setState({ language: "ru" });
    document.body.innerHTML = "";
  });
  const tick = () => new Promise((resolve) => setTimeout(resolve, 0));

  it("swaps the interface texts and labels, leaves songs and lyrics, and puts Russian back", async () => {
    document.body.innerHTML = `<main><h1>Настройки</h1><button aria-label="Поиск" data-tip="Скачать">Скачать</button><span class="track-title">Настройки</span><div class="lyrics"><p>Нет</p></div></main>`;
    useSettings.setState({ language: "en" });
    stop = startTranslator();
    expect(document.querySelector("h1")!.textContent).toBe("Settings");
    expect(document.querySelector("button")!.getAttribute("aria-label")).toBe("Search");
    expect(document.querySelector("button")!.textContent).toBe("Download");
    expect(document.querySelector(".track-title")!.textContent).toBe("Настройки");
    expect(document.querySelector(".lyrics p")!.textContent).toBe("Нет");
    expect(document.documentElement.lang).toBe("en");

    // text the app writes later is translated too
    const added = document.createElement("p");
    added.textContent = "Главная";
    document.body.append(added);
    await tick();
    expect(added.textContent).toBe("Home");

    useSettings.setState({ language: "ru" });
    expect(document.querySelector("h1")!.textContent).toBe("Настройки");
    expect(document.querySelector("button")!.getAttribute("aria-label")).toBe("Поиск");
    expect(added.textContent).toBe("Главная");
    expect(document.documentElement.lang).toBe("ru");
  });

  it("reads a run of text nodes as one text, and follows what React writes into one of them", async () => {
    // {done} из {total} · {share}%  is six text nodes
    document.body.innerHTML = "<div id=\"pill\"></div>";
    const pill = document.getElementById("pill")!;
    for (const part of ["46", " из ", "190", " · ", "24", "%"]) pill.append(document.createTextNode(part));
    useSettings.setState({ language: "en" });
    stop = startTranslator();
    expect(pill.textContent).toBe("46 of 190 · 24%");

    // the share changes: React writes the fifth node only
    (pill.childNodes[4] as Text).data = "25";
    await tick();
    expect(pill.textContent).toBe("46 of 190 · 25%");

    useSettings.setState({ language: "ru" });
    expect(pill.textContent).toBe("46 из 190 · 25%");
  });

  it("reads a run of text next to an element too (the session banner)", () => {
    document.body.innerHTML = "<span id=\"t\"></span>";
    const t = document.getElementById("t")!;
    t.append(document.createTextNode("Вместе: "), document.createTextNode("1 слушатель"));
    const small = document.createElement("small");
    small.textContent = "Вы организатор";
    t.append(small);
    useSettings.setState({ language: "en" });
    stop = startTranslator();
    expect(t.textContent).toBe("Together: 1 listenerYou are the host");
    useSettings.setState({ language: "ru" });
    expect(t.textContent).toBe("Вместе: 1 слушательВы организатор");
  });
});
