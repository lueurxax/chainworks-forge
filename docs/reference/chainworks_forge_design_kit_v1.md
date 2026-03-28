# Chainworks Forge — Design Kit v1

## 1. Зачем это нужно

Этот документ фиксирует базовую визуальную систему Chainworks Forge: знак, цвет, типографику, иконки, правила интерфейса и готовые промпты для дальнейшей генерации и полировки. Цель простая: чтобы продукт выглядел как инструмент с характером, а не как ещё один AI-чат с красивой обложкой.

Главная метафора бренда:

> **Согласованное движение нескольких специализированных агентов под управлением лидера.**

Отсюда и знак:

- **клин из трёх гусей** — workflow и командное движение;
- **лидирующий гусь** — orchestrator / run;
- **плавная траектория** — flow, chain, execution path;
- **отсутствие буквальной цепи** — меньше шума, больше продукта.

---

## 2. Бренд-ядро

### Название

**Chainworks Forge**

### Характер

- инженерный;
- собранный;
- живой, но не шумный;
- строгий без корпоративной стерильности;
- не «магический AI», а понятный control plane.

### Что бренд должен передавать

- orchestration;
- движение вперёд;
- согласованную работу ролей;
- контроль над сложным процессом;
- превращение сырой идеи в результат.

### Чего бренд не должен передавать

- болтовню про AI;
- ощущение чат-бота;
- fantasy ради fantasy;
- тяжёлую “металлическую” буквальность;
- стартапную глянцевость с бессмысленными градиентами.

---

## 3. Логотип

## 3.1 Основная идея

Основной знак — **три гуся в клине**, летящие по восходящей траектории слева снизу направо вверх.

### Семантика

- первый гусь: лидер, orchestrator, run control;
- два следующих: специализированные агенты;
- общая форма: workflow / chain of execution;
- линия под птицами: направленность и траектория выполнения.

## 3.2 Базовые правила

### Обязательные свойства знака

- 3 гуся, не 4 и не 5;
- форма клина должна читаться даже в маленьком размере;
- лидер должен быть немного впереди;
- линия движения должна быть мягкой и вторичной, не главнее птиц;
- силуэты должны быть чистыми, без мелкой иллюстративной детализации.

### Что нельзя делать

- добавлять буквальные звенья цепи;
- превращать знак в сложную иллюстрацию;
- делать из птиц реалистичный орнитологический рисунок;
- перегружать крылья мелкими перьями;
- использовать несколько акцентных цветов одновременно.

## 3.3 Версии логотипа

### Основная горизонтальная версия

Использование:
- README;
- сайт;
- splash / launch screen;
- документация;
- презентации.

Состав:
- знак слева;
- wordmark справа;
- интервал между знаком и названием свободный и «дышащий».

### App icon version

Использование:
- `Assets.xcassets`;
- macOS app icon;
- internal build previews.

Состав:
- квадратная композиция;
- знак центрирован;
- фон тёмный;
- без текста.

### Monochrome version

Использование:
- toolbar;
- sidebar;
- small-size UI;
- печать / single-color use cases.

Состав:
- один цвет;
- без внутренних бликов;
- максимально чистый силуэт.

## 3.4 Минимальные размеры

- **до 24 px** — использовать упрощённый силуэт без линии траектории;
- **24–64 px** — знак с тремя гусями, но без мелких внутренних деталей;
- **64 px и больше** — можно использовать полную версию с линией движения.

## 3.5 Safe area

Вокруг знака должен быть свободный отступ не меньше высоты головы среднего гуся. Ничто не должно прижиматься к клюву лидера или к внешней дуге траектории.

---

## 4. Цветовая система

## 4.1 Основная палитра

### Primary

| Token | Hex | Назначение |
|---|---|---|
| `ForgeBlue` | `#0B1F2A` | основной тёмный фон, ключевые акценты интерфейса |
| `ForgeBlueSoft` | `#132F3F` | вторичный тёмный тон, hover / panels / depth |

### Accent

| Token | Hex | Назначение |
|---|---|---|
| `ForgeAccent` | `#FF8A00` | клювы в логотипе, CTA, approval / attention highlights |
| `ForgeAccentSoft` | `#FFB347` | мягкие акценты, выделения, secondary emphasis |

### Neutrals

| Token | Hex | Назначение |
|---|---|---|
| `ForgeBackgroundLight` | `#F5F7FA` | светлый фон |
| `ForgeBackgroundDark` | `#0A0F14` | тёмный фон |
| `ForgeSurfaceLight` | `#FFFFFF` | карточки и поверхности на светлом фоне |
| `ForgeSurfaceDark` | `#111821` | карточки и поверхности на тёмном фоне |
| `ForgeTextPrimary` | `#1A1A1A` | основной текст на светлом фоне |
| `ForgeTextSecondary` | `#6B7280` | вторичный текст |
| `ForgeTextOnDark` | `#E8EDF2` | основной текст на тёмном фоне |

## 4.2 Статусы

| Status | Token | Hex |
|---|---|---|
| running | `RunBlue` | `#2563EB` |
| waiting approval | `RunAmber` | `#F59E0B` |
| blocked | `RunRed` | `#DC2626` |
| failed | `RunCrimson` | `#B91C1C` |
| completed | `RunGreen` | `#16A34A` |
| pending / idle | `RunGray` | `#9CA3AF` |

## 4.3 Правила цвета

- оранжевый использовать скупо;
- оранжевый не должен быть базовым цветом больших поверхностей;
- логотип может жить без оранжевого в monochrome-версии;
- интерфейс не должен быть похож на «чёрный терминал с неоновыми кнопками»;
- status color важнее декоративного цвета.

---

## 5. Типографика

## 5.1 Основной шрифт

Использовать системный стек Apple:

- **SF Pro Display** — заголовки;
- **SF Pro Text** — основной интерфейс и контент.

## 5.2 Размеры

| Назначение | Размер |
|---|---|
| App Title / hero | 24–28 |
| Section title | 18–20 |
| Standard label | 14–16 |
| Body text | 13–14 |
| Meta / helper | 11–12 |

## 5.3 Веса

| Назначение | Weight |
|---|---|
| Hero / big section titles | Semibold / Bold |
| Primary UI labels | Medium |
| Body copy | Regular |
| Secondary info | Regular / Medium |

## 5.4 Типографические правила

- не использовать жирность как замену иерархии;
- не ставить весь интерфейс в uppercase;
- uppercase допустим только в служебных малых лейблах и status chips;
- spacing важнее «красивых шрифтовых эффектов».

---

## 6. Иконография

Иконки должны продолжать язык логотипа, а не спорить с ним.

## 6.1 Базовый атом

**Гусь** — атомарный знак движения и исполнения.

## 6.2 Иконки системы

| Иконка | Значение | Идея |
|---|---|---|
| `run` | run execution | один гусь, направленный вперёд |
| `workflow` | workflow | три гуся клином |
| `stage` | stage | гусь + точка впереди |
| `approval` | gate / approval | гусь + check |
| `blocked` | blocked state | гусь + разрыв траектории |
| `failed` | failure | гусь + cross |
| `completed` | success | гусь + круг / finish mark |
| `artifact` | artifact | лист / слой + мягкая траектория |

## 6.3 Правила для small icons

- до 16 px — не рисовать трёх птиц, только абстрагированный силуэт;
- до 20 px — не использовать тонкие хвостовые линии;
- small icons должны быть монохромными.

---

## 7. UI-принципы

## 7.1 Главная иерархия продукта

В интерфейсе всегда должна читаться такая цепочка:

```text
Run → Stage → Agent → Artifact
```

### Что это значит визуально

- **Run** — главный объект, крупный и самый заметный;
- **Stage** — основной контекст внутри run;
- **Agent** — исполнитель внутри stage;
- **Artifact** — результат, доступный для инспекции.

## 7.2 Чего не делать

- не строить интерфейс как чат;
- не смешивать progress, logs, approvals и artifacts в один слой;
- не прятать важные действия за декоративной сеткой карточек;
- не делать интерфейс «технологичным» ценой ясности.

## 7.3 Правило главного экрана

На любом ключевом экране пользователь должен быстро понимать:

1. где сейчас находится run;
2. что требует внимания;
3. что уже произошло;
4. что можно открыть и проверить.

---

## 8. Motion

Motion должен помогать ориентироваться, а не развлекать.

## 8.1 Разрешённые эффекты

- мягкий fade / slide для перехода между stage;
- subtle pulse для running agent;
- краткий pop-in для approval gate;
- мягкий status transition для run chips.

## 8.2 Запреты

- никакого «летящего логотипа» на каждом экране;
- никакой бесконечной декоративной анимации;
- никакой тяжёлой spring-анимации для служебного UI.

---

## 9. Ассеты и структура

Рекомендуемая структура:

```text
Design/
  Brand/
    chainworks_forge_logo_main.png
    chainworks_forge_logo_dark.png
    chainworks_forge_logo_light.png
    chainworks_forge_logo_monochrome.png
  Icons/
    run.svg
    workflow.svg
    stage.svg
    approval.svg
    blocked.svg
    failed.svg
    completed.svg
    artifact.svg
  AppIcon/
    appicon_1024.png
    appicon_dark.png
    appicon_light.png
  Tokens/
    Colors.swift
    Typography.swift
    Theme.swift
```

---

## 10. SwiftUI design tokens

Ниже — базовая стартовая схема для кодовой дизайн-системы.

```swift
import SwiftUI

enum ForgeColor {
    static let blue = Color(hex: 0x0B1F2A)
    static let blueSoft = Color(hex: 0x132F3F)
    static let accent = Color(hex: 0xFF8A00)
    static let accentSoft = Color(hex: 0xFFB347)

    static let backgroundLight = Color(hex: 0xF5F7FA)
    static let backgroundDark = Color(hex: 0x0A0F14)
    static let surfaceLight = Color.white
    static let surfaceDark = Color(hex: 0x111821)

    static let textPrimary = Color(hex: 0x1A1A1A)
    static let textSecondary = Color(hex: 0x6B7280)
    static let textOnDark = Color(hex: 0xE8EDF2)

    static let runBlue = Color(hex: 0x2563EB)
    static let runAmber = Color(hex: 0xF59E0B)
    static let runRed = Color(hex: 0xDC2626)
    static let runCrimson = Color(hex: 0xB91C1C)
    static let runGreen = Color(hex: 0x16A34A)
    static let runGray = Color(hex: 0x9CA3AF)
}

enum ForgeTypography {
    static let hero = Font.system(size: 26, weight: .semibold, design: .default)
    static let section = Font.system(size: 18, weight: .semibold, design: .default)
    static let label = Font.system(size: 14, weight: .medium, design: .default)
    static let body = Font.system(size: 13, weight: .regular, design: .default)
    static let meta = Font.system(size: 11, weight: .regular, design: .default)
}
```

---

## 11. Правила для app icon

## 11.1 Композиция

- знак центрируется;
- гуси не должны упираться в края;
- линия траектории не должна касаться скруглений иконки;
- background лучше тёмный, чтобы силуэт жил контрастно.

## 11.2 Что важно проверить руками

- читается ли знак на 32 px;
- не сливается ли линия траектории с фоном;
- не становится ли средний гусь визуальным шумом;
- не выглядит ли знак как авиакомпания вместо control plane.

---

## 12. Prompt pack

Ниже — набор готовых промптов, если понадобится ещё раз генерировать или полировать логотип через image model или передавать задачу дизайнеру.

## 12.1 Главный prompt: production logo refinement

```text
Create a production-ready logo for a macOS developer tool called “Chainworks Forge”.

Core metaphor:
- three geese flying in a clean V-formation
- the lead goose is slightly ahead and represents orchestration
- the other two geese represent specialized agents
- a subtle curved trajectory line underneath suggests workflow / execution flow
- do NOT use literal chain links

Style:
- modern product logo, not an illustration
- minimal, crisp, vector-like shapes
- clean silhouette that works at small sizes
- elegant but restrained
- engineering product, not playful mascot branding
- not an AI cliché, not fantasy art, not corporate stock style

Visual rules:
- exactly 3 geese
- reduce feather detail
- strong silhouette readability
- one lead bird slightly emphasized
- thin secondary trajectory line
- balanced negative space
- suitable for app icon and horizontal product branding

Color palette:
- dark navy / graphite body tones
- soft light gray for contrast shapes
- small orange accent on beaks only
- optional monochrome version

Deliver:
1. primary horizontal logo
2. square app icon version
3. monochrome version
4. dark and light background variants

Avoid:
- chain links
- realism
- too many gradients
- mascot/cartoon look
- too much detail in wings
- generic AI / neural / brain imagery
```

## 12.2 Prompt: ultra-minimal icon version

```text
Design an ultra-minimal app icon for “Chainworks Forge” based on three geese flying in V-formation.

Requirements:
- square composition
- dark background
- simplified geometric bird shapes
- high contrast
- readable at 32px and 64px
- no text
- subtle curved line for motion if it survives small-size clarity
- elegant, premium, engineering-tool aesthetic

Avoid:
- realistic feathers
- decorative textures
- excessive highlights
- cartoon bird styling
```

## 12.3 Prompt: monochrome UI symbol system

```text
Create a monochrome icon family for a macOS workflow orchestration tool.

Base metaphor: stylized goose / flight formation.

Icons needed:
- run
- workflow
- stage
- approval
- blocked
- failed
- completed
- artifact

Style:
- monochrome
- minimal
- consistent stroke logic
- suitable for toolbar / sidebar / 16–20px sizes
- derived from the same visual language as a three-geese logo
```

## 12.4 Prompt: brand board / presentation sheet

```text
Create a clean brand presentation board for “Chainworks Forge”.

Show:
- primary logo
- app icon
- dark version
- light version
- monochrome version
- color palette chips
- logo usage examples on a macOS product context

Style:
- premium product design board
- calm grid layout
- minimal labels
- no fake file icons
- no overdesigned presentation elements
- modern, technical, elegant
```

---

## 13. Практический next step

Если превращать это в рабочий пакет, порядок я бы держал такой:

1. зафиксировать **основной знак**;
2. сделать **app icon 1024×1024**;
3. сделать **monochrome SVG / vector version**;
4. собрать `Colors.swift`, `Typography.swift`, `Theme.swift`;
5. ввести эту систему в 2–3 ключевых SwiftUI-экрана;
6. только потом допиливать мелкие декоративные различия.

Иначе очень легко наделать много красивых мелочей раньше, чем появится основной костяк.

---

## 14. Короткая формула

> **Chainworks Forge выглядит не как AI-игрушка, а как собранный рабочий инструмент.**
>
> Визуально это выражается через три вещи:
> - согласованное движение,
> - лидерство внутри системы,
> - и чистую инженерную форму без лишней магии.
