# Нативный Wine + FEX для Vela

Это запись локальной сборки, а не готовый универсальный установщик движков.
Минимальная система движка — macOS 26.5 на Apple Silicon. Минимум интерфейса Vela
и минимум подключённого движка — разные требования.

## Закреплённые компоненты

Точные ревизии и SHA-256 архивов находятся в `native-runtime-sources.json`.
Wine 11.18: `6880117619afdf62b4ccc40a8c6268c86613131f`.
FEX: `0df84d3844bcdb87bb7d3f5b8fb0959cd009c038`.
Применены соответственно 38 и 5 патчей из Hadron v0.3.1. Их байты сверены с
коммитом `3f5f18e2c08e22eb539f1fbbee52d4ba3d9e05c4`, указанным в notices готового runtime.

[Wine](https://github.com/wine-mirror/wine), [FEX](https://github.com/FEX-Emu/FEX),
[патчи и сценарии поставщика](https://github.com/Broyojo/hadron).

## Инструменты и параметры

Локально использованы Apple clang 21 / SDK 26.5, Bison 3.8.2, CMake,
LLVM dlltool/ar/ranlib 23.1.4 и LLD 23.1.1 из Rust 1.99.1.
Windows-объектные файлы обязательно должны обрабатываться LLVM ar/ranlib:
системный Apple ar повреждал COFF-архивы при добавлении файлов.

Wine настроен с такими параметрами:

```sh
export MACOSX_DEPLOYMENT_TARGET=26.5
export CPPFLAGS="-I<private-headers> -DWINE_HIGH_USER_SHARED_DATA"
export CROSSCFLAGS="-O2 -DWINE_HIGH_USER_SHARED_DATA"
export LDFLAGS="-L<private-libraries>"

<patched-wine-source>/configure \
  --prefix=<new-runtime-directory> \
  --enable-archs=arm64ec,aarch64,i386 \
  --with-mingw=/usr/bin/clang \
  --without-x --disable-tests \
  CC=/usr/bin/clang CXX=/usr/bin/clang++ BISON=<bison-3.8.2>
make -j6
make install
```

Угловые скобки обозначают пути разработчика, их нужно заменить.
Переменные передаются в окружении configure и make. Для приватного Bison задан
`BISON_PKGDATADIR`, для FreeType/GnuTLS — `PKG_CONFIG_PATH`.

Из FEX отдельно пересобраны `Source/Windows/UnixLib` после тех же патчей:
CMake, Release, Apple clang++, `CMAKE_OSX_DEPLOYMENT_TARGET=26.5`.
`libarm64ecfex.so` и `libwow64fex.so` размещены в `lib/wine/aarch64-unix/`.
PE-библиотеки `libarm64ecfex.dll` и `libwow64fex.dll` взяты из проверенного архива
Hadron 0.3.1. Повторные копии называются `xtajit64.dll` и `xtajit.dll`.
Повторный `make install` Wine заменяет xtajit заглушками — копии FEX надо восстановить.
Основное приложение Hadron, Steam-интеграция и его графические пакеты не использованы.

Дополнительно применяется собственный патч из `runtime/patches/`: сборка с верхними
адресами не объявляет поддержку 32-битных архитектур. Это исключает запуск
неподдерживаемых 32-битных шагов при настройке нового окружения.

GnuTLS и зависимости, а также FreeType и необходимые ему библиотеки размещены
в `ext/lib`, ссылки на Homebrew заменены на пути относительно библиотеки.
Это позволяет работать без установленного у пользователя Homebrew. Скопированные
Mach-O библиотеки подписаны локально после изменения install names.
Валидация зависимостей включает загрузку GnuTLS и `gnutls_global_init() == 0`.
Это не заменяет проверку HTTPS конкретной Windows-программы.

Загрузчик подписан ad hoc с обычным разрешением
`com.apple.security.custom-x18-abi-toggle = true`.
Ограниченное cross-architecture entitlement не добавлялось; SIP и AMFI не отключались.

## Возможности этой конфигурации

`WINE_HIGH_USER_SHARED_DATA` переносит разделяемые данные выше 4 ГБ. Это позволяет
запускать 64-битные программы без ограниченного профиля Apple, но не даёт поддержки
32-битных программ. Наличие i386 DLL в сборке не снимает этого ограничения.
В `vela-runtime.json` обязательно стоит `supports_x86_32: false`.

Без профиля Apple нельзя просто поменять этот флаг на true: он описывает возможности
движка, а не включает отсутствующее разрешение ОС.
Программы, жёстко ожидающие традиционный низкий адрес KUSER_SHARED_DATA, также могут
быть несовместимы. Это предварительный движок для проверки конкретного ПО.
Vulkan/MoltenVK, GStreamer/FFmpeg, DXMT и готовый набор DirectX в эту сборку не входят.

## Проверка и обновление

`tests/windows/probe.c` компилируется в настоящие AMD64 и i386 PE без Windows SDK.
`scripts/build_windows_probes.py` использует clang и LLD. `tests/macos/translation_probe.c`
читает системный флаг `P_TRANSLATED`: контрольные процессы ARM64 и Rosetta дали
соответственно false и true.

`scripts/native_smoke_test.py` проверяет подключение ARM64 Wine/FEX, wineboot,
запуск именно внешнего AMD64 PE, отсутствие Rosetta по флагу ОС, отказ для i386
и остановку только тестового окружения. Его `--home` должен быть новым временным
каталогом; пользовательские префиксы тест не использует.

Обновление делается отдельной версией папки движка и новым тестовым окружением.
Старый движок и его префикс сохраняются. После успешной проверки нужных программ
можно переносить рабочие файлы вручную; автоматическая конвертация Intel-префиксов
в ARM64-префиксы не реализована. Vela 0.2 не скачивает новые движки автоматически.

Wine сохраняет реестр асинхронно. После успешного wineboot менеджер ждёт
появления system.reg до 20 секунд; отсутствие файла сразу после завершения
процесса не считается мгновенной ошибкой. Отсутствующий сервер при остановке
считается уже остановленным окружением.
