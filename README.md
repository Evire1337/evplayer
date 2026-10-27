<div align="center">

<a href="https://github.com/Evire1337/evplayer/releases/latest">
  <img src="https://raw.githubusercontent.com/Evire1337/evplayer/desktop/public/icon.png" alt="EvPlayer" width="120" />
</a>

# EvPlayer

**SoundCloud, YouTube Music и ваши файлы в одном плеере для компьютера**<br />
Живой фон из обложки, текст песен строка за строкой, эквалайзер, статистика и мини-плеер поверх окон.<br />
Без рекламы, без регистрации, бесплатно и с открытым кодом.

[![Version](https://img.shields.io/github/v/release/Evire1337/evplayer?style=for-the-badge&label=версия&color=6ff0c4&labelColor=06201d)](https://github.com/Evire1337/evplayer/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/Evire1337/evplayer/total?style=for-the-badge&label=скачиваний&color=6ff0c4&labelColor=06201d)](https://github.com/Evire1337/evplayer/releases)
[![Stars](https://img.shields.io/github/stars/Evire1337/evplayer?style=for-the-badge&label=звёзды&color=6ff0c4&labelColor=06201d)](https://github.com/Evire1337/evplayer/stargazers)
[![License](https://img.shields.io/github/license/Evire1337/evplayer?style=for-the-badge&label=лицензия&color=6ff0c4&labelColor=06201d)](LICENSE)
[![Check](https://img.shields.io/github/actions/workflow/status/Evire1337/evplayer/check.yml?branch=desktop&style=for-the-badge&label=сборка&labelColor=06201d)](https://github.com/Evire1337/evplayer/actions)

![Windows](https://img.shields.io/badge/Windows_10_%7C_11-0078D4?style=flat-square&logo=windows&logoColor=white)
![Electron](https://img.shields.io/badge/Electron-47848F?style=flat-square&logo=electron&logoColor=white)
![React](https://img.shields.io/badge/React_19-149ECA?style=flat-square&logo=react&logoColor=white)
![TypeScript](https://img.shields.io/badge/TypeScript-3178C6?style=flat-square&logo=typescript&logoColor=white)
![Vite](https://img.shields.io/badge/Vite-646CFF?style=flat-square&logo=vite&logoColor=white)

<a href="https://github.com/Evire1337/evplayer/releases/latest">
  <img src="https://img.shields.io/badge/Скачать_для_Windows-6ff0c4?style=for-the-badge&logo=windows&logoColor=06201d&labelColor=6ff0c4" alt="Скачать для Windows" height="42" />
</a>

<br /><br />

<img src="https://raw.githubusercontent.com/Evire1337/evplayer/desktop/docs/screenshots/now-playing.jpg" alt="Экран «Сейчас играет»: обложка, текст песни и очередь" width="900" />

</div>

---

## Что это

EvPlayer — музыкальный плеер для Windows. Ищет сразу по **SoundCloud** и **YouTube Music**, играет **ваши файлы** (MP3, FLAC, M4A, OGG, WAV) и складывает всё в одну библиотеку: в одном плейлисте могут лежать треки из всех источников вперемешку. Плейлисты из **Spotify**, **Яндекс Музыки** и **Deezer** переносятся по ссылке — песни находятся автоматически.

Всё хранится на вашем компьютере: никаких аккаунтов и серверов EvPlayer. Обновления приходят сами.

## Возможности

<table>
<tr>
<td width="50%" valign="top">

**🎧 Воспроизведение**
- Поиск по SoundCloud и YouTube Music: треки, исполнители, альбомы, плейлисты
- Очередь с перетаскиванием, перемешивание, повтор, «Радио по треку»
- Автовоспроизведение похожих, когда очередь закончилась
- Продолжение с того же места после перезапуска
- Скорость, тон (отдельно или как у пластинки) и реверберация

**🎚 Звук**
- 6-полосный параметрический эквалайзер с живым спектром
- Пресеты и свои пресеты, которыми можно поделиться кодом
- Выравнивание громкости между треками

**📝 Тексты**
- Синхронный текст из LRCLIB: строки подсвечиваются в такт, по клику — перемотка
- Обычный текст с Genius, если синхронного нет

</td>
<td width="50%" valign="top">

**📚 Библиотека**
- Понравившиеся и свои плейлисты с обложками и перестановкой треков
- Умные плейлисты: «На повторе», «Новые открытия», «Забытые любимые»
- Свои файлы: папки обновляются сами, теги и обложки можно править
- Импорт по ссылке: Spotify, Яндекс Музыка, YouTube, SoundCloud, Deezer
- Импорт архива данных Spotify, подключение профиля SoundCloud
- Обмен плейлистами одной строкой-кодом, резервная копия

**📊 Статистика**
- Топ-10 треков и исполнителей за неделю, месяц и всё время
- График прослушиваний любого трека, «сегодня прослушано»

**🪟 Система**
- Мини-плеер поверх всех окон, трей, медиаклавиши
- Свои горячие клавиши в окне и глобально
- Статус «Слушает» в Discord с обложкой и временем
- Автообновление через GitHub Releases

</td>
</tr>
</table>

## Скриншоты

<table>
<tr>
<td><img src="https://raw.githubusercontent.com/Evire1337/evplayer/desktop/docs/screenshots/home.jpg" alt="Главная" /><p align="center"><sub>Главная: недавнее, умные плейлисты, топы</sub></p></td>
<td><img src="https://raw.githubusercontent.com/Evire1337/evplayer/desktop/docs/screenshots/search.jpg" alt="Поиск" /><p align="center"><sub>Поиск по SoundCloud и YouTube Music</sub></p></td>
</tr>
<tr>
<td><img src="https://raw.githubusercontent.com/Evire1337/evplayer/desktop/docs/screenshots/library.jpg" alt="Библиотека" /><p align="center"><sub>Библиотека</sub></p></td>
<td><img src="https://raw.githubusercontent.com/Evire1337/evplayer/desktop/docs/screenshots/stats.jpg" alt="Статистика трека" /><p align="center"><sub>Статистика трека</sub></p></td>
</tr>
<tr>
<td><img src="https://raw.githubusercontent.com/Evire1337/evplayer/desktop/docs/screenshots/equalizer.jpg" alt="Эквалайзер" /><p align="center"><sub>Эквалайзер</sub></p></td>
<td><img src="https://raw.githubusercontent.com/Evire1337/evplayer/desktop/docs/screenshots/sound.jpg" alt="Скорость, тон и реверберация" /><p align="center"><sub>Скорость, тон и реверберация</sub></p></td>
</tr>
<tr>
<td><img src="https://raw.githubusercontent.com/Evire1337/evplayer/desktop/docs/screenshots/discord.jpg" alt="Discord" /><p align="center"><sub>Статус в Discord</sub></p></td>
<td align="center" valign="middle"><img src="https://raw.githubusercontent.com/Evire1337/evplayer/desktop/docs/screenshots/mini-player.png" alt="Мини-плеер" /><p align="center"><sub>Мини-плеер поверх окон</sub></p></td>
</tr>
</table>

## Скачать

| Система | Файл | |
|---|---|---|
| Windows 10 / 11 (64 бит) | `EvPlayer-Setup-x.y.z.exe` | [Последняя версия](https://github.com/Evire1337/evplayer/releases/latest) |

> [!NOTE]
> Установщик пока не подписан сертификатом, поэтому Windows SmartScreen может показать «Windows защитила ваш компьютер». Нажмите **Подробнее → Выполнить в любом случае**.

## Обновления

EvPlayer сам проверяет новые версии при запуске и каждые 4 часа. Обновление скачивается в фоне и ставится при следующем перезапуске — переустанавливать ничего не нужно. Проверить вручную: **Настройки → О программе**.

## Импорт из других сервисов

| Откуда | Как | Что переносится |
|---|---|---|
| Spotify | ссылка на плейлист, альбом или трек | первые 100 треков; после «Войти через Spotify» — весь плейлист (приложению Spotify нужен владелец с Premium) |
| Spotify | «Войти через Spotify» в «Сервисах» | «Любимые треки» и все ваши плейлисты целиком |
| Spotify | архив «Скачать свои данные» | любимые, все плейлисты, самое слушаемое |
| YouTube / YouTube Music | ссылка на плейлист или видео | весь плейлист |
| SoundCloud | ссылка или профиль в «Сервисах» | треки, плейлисты, ваши публичные лайки |
| Deezer | ссылка на плейлист, альбом или трек | весь плейлист |
| Яндекс Музыка | ссылка или профиль в «Сервисах» | плейлисты, альбомы, публичное «Мне нравится» |
| EvPlayer | код `EVP2.…` от друга | плейлист целиком с обложкой |

Песни из Spotify и Deezer ищутся на YouTube Music и SoundCloud; если уверенного совпадения нет, трек пропускается.

## Горячие клавиши

Все клавиши меняются в **Настройки → Клавиши**. По умолчанию:

| В окне | | Везде в системе | |
|---|---|---|---|
| `Пробел` | играть / пауза | `Ctrl+Alt+Пробел` | играть / пауза |
| `Ctrl+→` / `Ctrl+←` | следующий / предыдущий | `Ctrl+Alt+→` / `←` | следующий / предыдущий |
| `→` / `←` | перемотка на 5 с | `Ctrl+Alt+L` | нравится |
| `↑` / `↓` | громкость | `Ctrl+Alt+M` | мини-плеер |
| `L` / `M` / `S` / `R` | нравится / без звука / перемешать / повтор | `Ctrl+Alt+E` | показать окно |
| `Ctrl+F` / `Ctrl+,` | поиск / настройки | медиаклавиши | работают всегда |

## Где хранятся данные

| Что | Где |
|---|---|
| Библиотека, статистика, настройки | `%APPDATA%\EvPlayer` |
| Список ваших файлов и обложки к ним | `%APPDATA%\EvPlayer\local-tracks.json`, `local-covers\` |
| Резервная копия | файл `.json`, который вы сохраняете в **Настройки → Данные и копия** |

## Сборка из исходников

Нужны [Node.js 22+](https://nodejs.org) и [pnpm](https://pnpm.io) (`npm i -g pnpm`).

```powershell
git clone -b desktop https://github.com/Evire1337/evplayer.git
cd evplayer
pnpm install
pnpm dev          # запуск с горячей перезагрузкой
```

| Команда | Что делает |
|---|---|
| `pnpm dev` | приложение в режиме разработки: правки в `src/` применяются сразу |
| `pnpm test` | тесты (Vitest) |
| `pnpm typecheck` | проверка типов |
| `pnpm build` | сборка без установщика |
| `pnpm dist:win` | установщик в папке `release/` |

**Отладка.** В любом режиме `F12` или `Ctrl+Shift+I` открывает инструменты разработчика (консоль, сеть, производительность). Сообщения главного процесса (трей, файлы, Discord, обновления) печатаются в терминал, где запущен `pnpm dev`.

### Как выпустить новую версию

1. Поменяйте `version` в `package.json` и закоммитьте.
2. `git tag v0.3.0` → `git push origin v0.3.0`
3. GitHub Actions соберёт установщик и опубликует релиз — у всех пользователей обновление придёт само.

## Стек

| Часть | Технологии |
|---|---|
| Оболочка | Electron, electron-builder, electron-updater |
| Интерфейс | React 19, TypeScript, Vite |
| Состояние | Zustand |
| Звук | Web Audio API: свой AudioWorklet для тона, свёрточная реверберация, BiquadFilter-эквалайзер, лимитер |
| Потоки | hls.js; песни с YouTube играют из копии на SoundCloud |
| Файлы | music-metadata, свой протокол `evplayer-media://` с поддержкой перемотки |
| Фон | свой WebGL-шейдер из цветов обложки |
| Тесты и CI | Vitest, GitHub Actions |

## Ветки

- `desktop` — приложение для Windows (этот код)
- `mobile` — будущая версия для телефонов

## Звёзды

<a href="https://star-history.com/#Evire1337/evplayer&Date">
  <img src="https://api.star-history.com/svg?repos=Evire1337/evplayer&type=Date&theme=dark" alt="Star History" width="600" />
</a>

## Лицензия

[MIT](LICENSE) © Evire1337

EvPlayer — независимый проект и не связан с SoundCloud, YouTube, Google, Spotify, Deezer, Apple или Discord. Названия сервисов принадлежат их владельцам. Музыка воспроизводится через публичные сервисы этих платформ; тексты песен — LRCLIB и Genius.

<div align="center">
<br />
<a href="https://github.com/Evire1337/evplayer/releases/latest">
  <img src="https://img.shields.io/badge/Скачать_EvPlayer-6ff0c4?style=for-the-badge&logo=windows&logoColor=06201d" alt="Скачать EvPlayer" height="36" />
</a>
</div>
