# EvPlayer — как устроен код

Этот файл для людей и для ИИ-ассистентов (Claude Code, Codex, Cursor и других), которые берутся за проект. Здесь описано, где что лежит, как пишется код и как его проверять. Как присылать изменения — в [CONTRIBUTING.md](CONTRIBUTING.md).

## Коротко

- **Что это:** музыкальный плеер для Windows, Linux и Android. Играет SoundCloud, YouTube Music и локальные файлы; переносит плейлисты из Spotify, Яндекс Музыки, Deezer и VK.
- **Стек:** Tauri 2 (Rust) — нативная оболочка; React 19 + TypeScript + Vite — весь интерфейс и вся логика плеера; Zustand — состояние; Kotlin — Android-часть.
- **Основная ветка — `tauri`.** В `main` кода нет. Все ветки и pull request'ы делаются от `tauri` и в `tauri`.
- **Язык:** интерфейс на русском. Код, комментарии, имена и сообщения коммитов — на английском.

## Структура

```
src/                    интерфейс и логика (TypeScript, React)
  main.tsx              точка входа: миграция из Electron → восстановление сессии → bridge → рендер
  App.tsx               каркас окна, выбор страницы, диалоги; «засыпание» интерфейса в фоне
  types.ts              общие типы: Track, Artist, коллекции
  platform/             всё, что знает о Tauri и Android
    api.ts              интерфейс window.evplayer (EvPlayerApi) — единственный путь к нативной части
    tauri.ts            реализация EvPlayerApi через invoke() и события Tauri
    net.ts              запросы к сервисам через прокси evproxy; список разрешённых хостов
    androidMedia.ts     связь с Kotlin (window.EvPlayerAndroid)
    device.ts           isPhone и прочие признаки устройства
  app/                  то, что связывает части между собой
    bridge.ts           держит в курсе Rust-часть: трей, глобальные клавиши, статус Discord
    actions.ts          runAction(): что делает каждая команда (клавиши, трей, мини-плеер)
    session.ts          сохранение и восстановление очереди и позиции
  player/               ядро воспроизведения
    engine.ts           две <audio>-«деки», hls.js, кроссфейд, Media Session; объект `player`
    store.ts            usePlayer: очередь, индекс, статус, позиция
    streams.ts          выбор потока; если не играет — копия трека на другом сервисе
    match.ts            «та же ли это запись»: сравнение названий, артистов, длительности
    radio.ts, effects.ts  похожие треки, скорость/тон/реверберация
  audio/, equalizer/    граф Web Audio: эквалайзер, лимитер, выравнивание громкости, тон
  services/             клиенты сервисов: soundcloud, youtube, spotify, yandex, lyrics
  library/              лайки, плейлисты, импорт по ссылке, коды плейлистов
  offline/              скачанные треки и списки, которые держатся полными
  stats/                история, статистика прослушиваний, умные плейлисты
  jam/                  «Слушать вместе» через Nostr-релеи
    protocol.ts         чистые правила (коды, позиция, окно очереди) — покрыты тестами
    engine.ts           сессия: хост ведёт, гости подстраиваются (`jam`)
    link.ts             сеть и шифрование; грузится только когда нужна
  pages/                страницы (HomePage, SearchPage, …) и вкладки настроек
  ui/                   компоненты, навигация (ui/navigation.ts), тосты, WebGL-фон
  overlay/              мини-плеер поверх окон (отдельная страница overlay.html)
  settings/             useSettings, горячие клавиши, резервная копия
  styles/               base.css, app.css, phone.css (вёрстка телефона)

src-tauri/              нативная часть (Rust)
  src/lib.rs            точка входа: команды (#[tauri::command]), плагины, окно, setup
  src/proxy.rs          протокол evproxy:// — запросы к сервисам в обход CORS, только белый список
  src/media_server.rs   HTTP-сервер на 127.0.0.1 с секретом в адресе: звук для <audio> с Range
  src/library.rs        локальные файлы: теги (lofty), обложки, слежение за папками (notify)
  src/offline.rs        скачивание треков
  src/discord.rs        статус в Discord (RPC), «Попросить присоединиться»
  src/overlay.rs, tray.rs, updates.rs   мини-плеер, трей, автообновление (только ПК)
  src/memory.rs, diagnostics.rs         память и процессы WebView2
  src/mobile/mod.rs     заглушки для телефона с теми же именами, что у ПК-модулей
  tauri.conf.json       окно, CSP, схема evplayer://, ключ обновлений
  capabilities/         что страницам разрешено из API Tauri
  gen/android/          Android-проект; свой код — app/src/main/java/.../evplayer/*.kt

public/worklets/        AudioWorklet для сдвига тона
pages/                  сайт на GitHub Pages (приглашения «Слушать вместе»)
packaging/arch/         PKGBUILD
.github/workflows/      check.yml (каждый PR и пуш в tauri), release.yml (тег v*), android.yml
```

## Как это работает

**Страница делает почти всё сама.** Поиск, разбор ответов сервисов, очередь, эквалайзер — в TypeScript. Rust нужен для того, чего страница не может: обойти CORS, отдать звук и файлы, трей, глобальные клавиши, Discord, обновления.

**Связь страницы и Rust — только через `window.evplayer`.** Его тип описан в `src/platform/api.ts`, реализация — в `src/platform/tauri.ts`. Остальной код не импортирует `@tauri-apps/api` напрямую. В обычном браузере (`pnpm web`, тесты) `window.evplayer` нет, и приложение работает без нативных функций — код обязан это переживать (`window.evplayer?.…`).

**Сеть.** Запросы к сервисам идут через `evproxy://` (`proxy.rs`). Разрешённые хосты перечислены **в трёх местах**, и их надо менять вместе:
1. `OPEN_HOSTS` в `src-tauri/src/proxy.rs`;
2. `OPEN_HOSTS` в `src/platform/net.ts`;
3. CSP в `src-tauri/tauri.conf.json` (если нужен прямой доступ: релеи, картинки, медиа).

**Звук** идёт через `media_server.rs` (`http://127.0.0.1:PORT/SECRET/...`): WebKitGTK не умеет играть из своих схем, а WebView2 капризничает с Range.

**Состояние** — сторы Zustand (`useX`), сохраняемые через `persist` в localStorage под ключом `evplayer.<что>.v<N>`. Меняется форма данных — поднимите версию и напишите `migrate`. Вне React к стору обращаются через `useX.getState()`.

**Плеер.** Все действия пользователя проходят через `player` из `src/player/engine.ts` и описаны типом `PlayerAction`. Во время сессии «Слушать вместе» `jam` перехватывает эти действия: у гостя они уходят хосту запросом. Новое действие с очередью или позицией нужно провести и через Jam.

**Телефон.** Тот же React-код; `isPhone` включает вёрстку из `phone.css` и отключает кэши. Нативное на Android — Kotlin-классы, которые страница зовёт через `window.EvPlayerAndroid` (`MediaBridge.kt`). Rust-модули только для ПК закрываются `#[cfg(desktop)]`, а для телефона в `mobile/mod.rs` есть заглушки с теми же именами.

### Как добавить команду в Rust

1. `#[tauri::command] fn my_thing(...)` в `src-tauri/src/lib.rs` (или в своём модуле) и её имя в `generate_handler![...]`.
2. Метод в `EvPlayerApi` (`src/platform/api.ts`) с комментарием, что он делает.
3. Реализация в `src/platform/tauri.ts`: `invoke("my_thing", {...})`.
4. Если команда только для ПК — заглушка для телефона.

## Как пишется код

Пишите так же, как уже написано вокруг. Главное:

- **Комментарии объясняют «зачем», простыми словами и с точки зрения пользователя.** Не «вызывает X», а «трек, который не включается, сам сменяется следующим». Перед модулем — короткий абзац о том, что он делает и почему так. У констант — комментарий `/** ... */` с единицами измерения в имени: `START_TIMEOUT_MS`, `BEAT_MS`.
- **TypeScript строгий** (`strict`, `noUnusedLocals`, `noUnusedParameters`). Без `any`; внешние данные описываются типом и проверяются.
- **Импорты через `@/`** (`@/player/store`), внутри папки — относительные (`./protocol`).
- **Именование:** компоненты и их файлы — `PascalCase.tsx`; остальное — `camelCase.ts`; сторы — `useSomething`; одиночки-движки — объект с маленькой буквы (`player`, `jam`).
- **Чистая логика отдельно от побочных эффектов.** Разбор ответов сервисов (`mappers.ts`), сравнения (`match.ts`), правила протоколов (`jam/protocol.ts`) — чистые функции, их легко тестировать. Сеть, таймеры и DOM — в движках и компонентах.
- **Тяжёлое грузится лениво:** страницы кроме Главной и «Сейчас играет» — через `lazy()`, сеть Jam — динамическим `import()`.
- **Помните о памяти и батарее.** Приложение живёт в трее часами: снимайте таймеры и подписки (`useEffect` возвращает отписку, как `startOffline()` и `startInvites()` в `App.tsx`), не держите лишнего, когда окно скрыто (`useVisible`).
- **Тексты интерфейса — по-русски**, коротко и по-человечески, как в остальном приложении.
- **Rust:** `rustfmt` по умолчанию; ошибки команд — `Result<T, String>`; ничего не паникует в обработчиках; длинная работа — `async` или `spawn_blocking`.
- **Kotlin:** стиль из `.editorconfig` в `src-tauri/gen/android`.
- Отдельного линтера и форматтера для TS нет: сохраняйте отступ в 2 пробела, двойные кавычки, точки с запятой и длинные строки, как в соседних файлах.

## Как запускать и тестировать

Нужны Node.js 22+, pnpm, Rust; на Linux — системные библиотеки из README.

| Команда | Что делает |
|---|---|
| `pnpm install` | зависимости |
| `pnpm dev` | приложение целиком (Rust + страница с горячей перезагрузкой) |
| `pnpm web` | только страница в браузере на http://localhost:5173 — быстро проверить вёрстку; нативных функций нет |
| `pnpm typecheck` | проверка типов |
| `pnpm test` | тесты Vitest (`src/**/*.test.ts`, окружение jsdom) |
| `cd src-tauri && cargo test` | тесты Rust (`#[cfg(test)] mod tests` внизу модулей) |
| `pnpm build` | typecheck + сборка страницы |
| `pnpm tauri build` | установщик (Windows: NSIS; Linux: `--bundles deb,rpm,appimage`) |
| `pnpm tauri android build --apk` | APK (нужны Android SDK и NDK) |

**Перед pull request'ом обязательно:** `pnpm typecheck`, `pnpm test`, а если трогали Rust — `cargo test`. CI (`check.yml`) запускает то же самое и собирает тестовые версии для всех систем — их можно скачать внизу запуска во вкладке Actions.

**Тесты:**
- Лежат рядом с кодом: `foo.ts` → `foo.test.ts`. Пишутся на `describe`/`it`/`expect` из `vitest`. Названия `it(...)` — фразы о поведении: `it("reads the host's pass from a Discord invite")`.
- Ответы сервисов — сохранёнными JSON-фикстурами (`src/services/youtube/*.test.json`), без настоящей сети.
- Новой чистой логике — тест обязателен. Для UI и движков с сетью и `<audio>` автотестов нет: проверяйте руками в `pnpm dev` и опишите в PR, что проверили.
- Отладка: в `pnpm dev` — `Ctrl+Shift+I`; вывод Rust — в терминале, где запущен `pnpm dev`. Настройки → Диагностика показывает память и процессы.

## Коммиты

- Сообщение на английском, первая строка описывает **что изменилось для пользователя**, а не что сделано в коде: `Listen together: guests in step with the host; seeking that works`, `Android: the player stays in the notifications, paused too`. Часто с префиксом области: `Jam:`, `Android:`, `Discord:`, `CI:`.
- Если нужно — пустая строка и пара предложений о причине.
- Один коммит — одна законченная мысль. Версия меняется отдельным коммитом (`0.7.0`) в `package.json` и `src-tauri/Cargo.toml` вместе.

## Что нельзя ломать

- Сохранённые данные пользователей: ключи `evplayer.*.v*` в localStorage и файлы в `%APPDATA%\EvPlayer` / `~/.config/EvPlayer`. Меняете формат — пишите миграцию.
- Ключ обновлений (`pubkey` в `tauri.conf.json`) и идентификатор `io.github.evire1337.evplayer`.
- Белый список хостов и CSP: не открывайте всё подряд.
- Работу без `window.evplayer` (браузер, тесты) и на телефоне.
