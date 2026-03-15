# GHub - Game Library Launcher

Кроссплатформенный лаунчер для управления игровой библиотекой с поддержкой пространств, учётом времени игры и интеграцией с itch.io.

## Требования

### Для разработки

1. **Node.js** >= 18.x
2. **Rust** >= 1.70 (https://rustup.rs/)
3. **Visual Studio Build Tools** (Windows) с компонентом "C++ build tools"

### Установка Rust (Windows)

```powershell
# Скачайте и запустите rustup-init.exe с https://rustup.rs/
# Или через winget:
winget install Rustlang.Rustup
```

После установки перезапустите терминал.

## Структура проекта

```
ghub/
├── src/                    # React frontend
│   ├── components/         # UI компоненты
│   ├── hooks/              # React hooks (useGames, useSpaces)
│   ├── types/              # TypeScript типы
│   ├── locales/            # Локализация (ru, en)
│   └── lib/                # Утилиты
├── src-tauri/              # Rust backend
│   └── src/
│       ├── lib.rs          # Точка входа Tauri
│       ├── database.rs     # SQLite операции
│       ├── commands.rs     # Tauri команды
│       └── models.rs       # Структуры данных
├── package.json
└── README.md
```

## Запуск

```bash
# Установка зависимостей (уже выполнено)
npm install

# Запуск в режиме разработки
npm run tauri dev

# Сборка для production
npm run tauri build
```

## Функциональность

### Реализовано (Phase 1)

- ✅ Базовый UI с сайдбаром пространств
- ✅ Сетка игр (grid/list view)
- ✅ SQLite база данных с WAL mode
- ✅ CRUD для игр и пространств
- ✅ Сканирование папок для поиска игр
- ✅ Запуск exe файлов
- ✅ Локализация (RU/EN)

### В разработке (Phase 2)

- [ ] Heartbeat система для учёта времени
- [ ] Полнотекстовый поиск (FTS5)
- [ ] Steam library scanning
- [ ] Метаданные из IGDB/SteamGridDB
- [ ] Кеширование обложек

### Планируется (Phase 3-4)

- [ ] itch.io OAuth интеграция
- [ ] Butler для загрузок
- [ ] Wishlist/отложенные ссылки
- [ ] System tray
- [ ] Auto-updater

## Технологии

- **Frontend**: React 18, TypeScript, Tailwind CSS, TanStack Query
- **Backend**: Tauri 2.0, Rust
- **Database**: SQLite (rusqlite)
- **Локализация**: i18next

## Лицензия

MIT
