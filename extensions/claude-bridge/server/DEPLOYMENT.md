# Bridge на Linux: ручное развёртывание и обновление

Этот runbook — пример для оператора **одного** Bridge-контейнера на Linux-хосте
с Docker Engine. Он не задаёт registry, сетевой адрес, TLS/proxy, доступ к
Docker host или значения credentials: эти параметры зависят от окружения и
хранятся вне публичного репозитория. Имена контейнера и volumes, а также host
port в командах ниже служат примерами; сверяйте их с действующим deployment.
Публикация новой версии происходит через
[release flow](../../../CONTRIBUTING.md#8-автоматическая-публикация-после-merge);
на Linux-хосте оператор разворачивает уже опубликованный versioned image, а не
собирает или перезаписывает релиз вручную. Тег `latest` для воспроизводимого
обновления не подходит: зафиксируйте версию, registry и resolved digest.

Файл `docker-compose.yml` рядом с этим runbook предназначен для локальной
сборки: в нём есть `build`, bind mount `./installer` и legacy sync volume. Его
нельзя запускать как production-манифест без проверки фактического deployment.

## Что сохранять

| Путь в контейнере           | Данные                                                                                               | Правило                                                                                                                                                                                                    |
| --------------------------- | ---------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `/app/data`                 | DuckDB (`proxy.duckdb`), настройки, токены и новые sync snapshots (`bridge-sync/`)                   | Сохранять существующий named volume и делать согласованный backup при остановленном Bridge.                                                                                                                |
| `/home/bridge/.claude`      | Claude credentials/config, JSONL истории в `projects/`, настройки Bridge-сессий в `bridge-sessions/` | Сохранять существующий named volume; считать его и backup секретными.                                                                                                                                      |
| `/home/bridge/.claude.json` | Claude onboarding/account metadata                                                                   | **Не входит** в два volume выше. Entrypoint создаёт базовый файл заново; перед заменой сохраните его отдельно и проверьте auth после запуска. Полную сохранность этого файла текущая схема не гарантирует. |
| `/app/logs/lifecycle`       | Allowlisted lifecycle journal, максимум 5 MiB                                                        | Отдельный named volume; не монтировать весь `/app/logs`, содержащий raw payloads.                                                                                                                          |
| `/home/bridge/bridge-sync`  | Возможные legacy snapshots                                                                           | Если путь смонтирован в действующем контейнере, сохранять mount и backup до подтверждённой миграции по [BR-13](../RUNTIME_RELIABILITY.md).                                                                 |

Содержимое `/app/installer` поставляется с versioned image. Не подменяйте его
старым bind mount из локального compose: это может показать клиенту не тот
installer, который проверен release pipeline.
Удаление контейнера само по себе не удаляет **named** volumes, но уничтожает
данные вне mounts и прерывает все процессы. `docker volume rm/prune` к рабочим
volumes и backup не применять. Имена volumes из `docker run` и Compose могут
различаться: перед обновлением источником истины является `docker inspect`
действующего контейнера.

`VOLUME` в Dockerfile объявляет только путь внутри image. Если не указать
источник при запуске, Docker создаст **анонимный** volume: данные переживут
удаление контейнера без `-v`, но следующий `docker run` не подключит этот volume
автоматически. Поэтому data/auth и lifecycle-log paths всегда монтируйте явно, а перед заменой
сверяйте `Type`, `Name`/`Source` и `Destination` в `docker inspect`. Volume —
место хранения, а не бессрочный архив: очистка старых JSONL самим Claude Code
или Bridge действует и внутри него. Для сохранения истории сверх срока очистки
нужны отдельная настройка retention и проверенный backup; одно лишь повторное
подключение того же volume удалённые файлы не вернёт.

## Перед первым запуском или заменой контейнера

1. Уточните у владельца deployment нужные registry/image digest, способ
   доступа клиентов (TLS, reverse proxy, VPN), расположение secret file и
   место шифрованных backup. В публичный репозиторий эти сведения не записывайте.
2. Если контейнер уже существует, сверьте его image, **имена и назначения**
   mounts, restart policy и legacy sync mount. Не переносите данные в новые
   volume только потому, что они так названы в примере ниже. Не выводите
   `Config.Env` в ticket или общий лог: там могут быть credentials.

   ```bash
   sudo docker inspect open-claude-bridge \
     --format 'image={{.Config.Image}} mounts={{range .Mounts}}{{.Type}}:{{.Name}}:{{.Source}}:{{.Destination}} {{end}} restart={{.HostConfig.RestartPolicy.Name}}'
   sudo docker volume inspect open-claude-bridge-data open-claude-bridge-claude
   ```

3. Перед остановкой назначьте окно обслуживания: `docker stop` завершает
   работающие PTY-процессы, а новый контейнер может восстановить историю из
   JSONL, но **не продолжает выполнявшийся процесс с того же места**. Сверьте
   число активных сессий и предупредите пользователей. Сначала `docker pull`
   целевой versioned image, затем остановка и backup; не используйте
   `docker rm -f` как шаг обновления.
4. Для нового deployment создайте named volumes явно. Для существующего
   сначала убедитесь через `docker volume inspect`, что используете именно его
   старые volumes; Docker может автоматически создать пустой volume при опечатке
   в имени. Secret file держите вне Git, с доступом только операторам:
   `DASHBOARD_USER` и `DASHBOARD_PASSWORD` сейчас читаются из environment.
   Для локального env file проверьте владельца и режим `0600`; не создавайте
   его пустым поверх уже действующего файла при обновлении. `--env-file` ниже —
   способ передачи credentials в этом примере, а не обязательный файл Bridge.
   Если deployment уже передаёт environment variables иначе, сохраните его
   способ и не используйте `BRIDGE_ENV_FILE`/`--env-file` из примера.
   `--env-file` убирает пароль из shell history, но **не скрывает** его от
   администраторов Docker и `docker inspect`. Если это не соответствует требованиям
   deployment, нужен подходящий secret manager и отдельная поддержка file-based
   secret в Bridge; текущий env file не является Docker secret.
   Dashboard credentials защищают dashboard, но не заменяют сетевую изоляцию
   всего Bridge API и TLS. Для первой установки (не при обновлении) volumes
   создаются так:

   ```bash
   sudo docker volume create open-claude-bridge-data
   sudo docker volume create open-claude-bridge-claude
   sudo docker volume create open-claude-bridge-lifecycle-logs
   ```

Если существующий `/home/bridge/.claude.json` содержит настройки, которые
нельзя потерять, остановитесь и согласуйте его перенос/отдельное persistence
до замены контейнера: текущие два mounts этот файл не сохраняют.

## Backup перед обновлением

Остановите контейнер штатно, пока он ещё существует, и снимите backup каждого
фактически смонтированного volume. Для DuckDB нельзя считать простое копирование
живого файла согласованным backup. Пример использует `BACKUP_IMAGE` — выбранный
оператором utility image с `tar`; подставьте реальный защищённый путь
и **проверенные** имена volumes из предыдущего шага:

Перед выполнением задайте `BRIDGE_IMAGE` (целевой image/digest), `BACKUP_IMAGE`,
`BRIDGE_BACKUP_DIR`, `BRIDGE_ENV_FILE` и уникальное имя
`BRIDGE_PREVIOUS_CONTAINER`. Ни один из этих параметров не должен содержать
секрет в командной строке. Проверьте, что обязательные значения заданы:

```bash
: "${BRIDGE_IMAGE:?set approved versioned image}"
: "${BACKUP_IMAGE:?set approved tar-capable image}"
: "${BRIDGE_BACKUP_DIR:?set restricted backup directory}"
: "${BRIDGE_ENV_FILE:?set restricted env-file path}"
: "${BRIDGE_PREVIOUS_CONTAINER:?set unique previous-container name}"
sudo docker pull "$BRIDGE_IMAGE"
```

```bash
sudo docker stop --time 30 open-claude-bridge
sudo docker run --rm \
  --mount type=volume,source=open-claude-bridge-data,target=/source,readonly \
  --mount type=bind,source="$BRIDGE_BACKUP_DIR",target=/backup \
  --entrypoint tar "$BACKUP_IMAGE" -C /source -cf /backup/bridge-data.tar .
sudo docker run --rm \
  --mount type=volume,source=open-claude-bridge-claude,target=/source,readonly \
  --mount type=bind,source="$BRIDGE_BACKUP_DIR",target=/backup \
  --entrypoint tar "$BACKUP_IMAGE" -C /source -cf /backup/claude-home.tar .
sudo docker cp open-claude-bridge:/home/bridge/.claude.json \
  "$BRIDGE_BACKUP_DIR/claude.json"
```

Backup directory должен уже существовать с режимом `0700`, а готовые файлы
должны быть доступны только операторам (`0600`). Если есть
legacy или дополнительные mounts, сохраните их отдельно. Проверьте ненулевой
размер, читаемость архивов, checksums и возможность восстановить копию в **новые
тестовые volumes**; не тестируйте restore на действующих данных. Храните backup
и манифест `image digest + mount names + время + checksums` в защищённом хранилище.
Не отправляйте архивы, `docker inspect` целиком или логи в public repo.

## Запуск обновлённого контейнера

После подтверждённого backup переименуйте остановленный контейнер для
диагностики и запуска прежнего image при необходимости. Пустой новый volume
создавать не нужно: укажите **те же** имена. Пример слушает только loopback
Docker host; внешний доступ требует отдельно настроенного TLS reverse proxy/VPN
с поддержкой WebSocket и нужных длительных соединений.
При первой установке (без старого контейнера) пропустите `docker rename`:

```bash
sudo docker rename open-claude-bridge "$BRIDGE_PREVIOUS_CONTAINER"
```

```bash
sudo docker run -d --name open-claude-bridge \
  --restart unless-stopped --stop-timeout 30 \
  --publish 127.0.0.1:3456:3456 \
  --health-cmd "node -e 'fetch(\"http://127.0.0.1:3456/health\").then(r=>process.exit(r.ok?0:1)).catch(()=>process.exit(1))'" \
  --health-interval 15s --health-timeout 5s --health-retries 5 --health-start-period 30s \
  --env-file "$BRIDGE_ENV_FILE" \
  --mount type=volume,source=open-claude-bridge-data,target=/app/data \
  --mount type=volume,source=open-claude-bridge-claude,target=/home/bridge/.claude \
  --mount type=volume,source=open-claude-bridge-lifecycle-logs,target=/app/logs/lifecycle \
  "$BRIDGE_IMAGE"
```

`BRIDGE_IMAGE` — утверждённый versioned image/digest из registry, а не
копия команды для предыдущей версии. Если старый контейнер использует
legacy sync mount, добавьте его **с тем же volume** и сохраните до закрытия
BR-13. Для первой установки создайте все три named volumes до `docker run` и
авторизуйте Claude поддерживаемым способом внутри контейнера; результаты
`claude auth status` и dashboard-login проверяйте локально, не публикуя их.

После запуска проверьте версию в `GET /health`, `claude auth status`, доступ
авторизованного dashboard/клиента, прежние токены/настройки и историю сессий,
а также выдачу нужного installer. `/health` сообщает состояние процесса и
версию, **но не доказывает** работоспособность DuckDB, Claude auth или sync.
`unhealthy` помогает обнаружить сбой, но Docker restart policy перезапускает
контейнер при завершении процесса, а не только из-за `unhealthy`.
Проверьте `sudo docker logs --tail 200 open-claude-bridge` на ошибки запуска;
логи также считайте приватной диагностикой.

Если новая версия не прошла проверку, остановите её и исследуйте причину.
Простой запуск старого image на уже изменённых новым image volumes не является
гарантированным rollback: миграции данных могли изменить формат. Для отката
восстановите проверенный backup в отдельные volumes и только затем поднимайте
прежний image. Старый контейнер, image и backup удаляйте после приёмки и
закреплённого срока хранения. `docker image prune -a` в процессе обновления
не запускайте: команда удаляет неиспользуемые images, включая возможный
rollback image после удаления старого контейнера; volumes она не заменяет и
не копирует.

Основа процедур backup/restart/prune: [Docker volumes](https://docs.docker.com/engine/storage/volumes/),
[restart policy](https://docs.docker.com/engine/containers/start-containers-automatically/),
[image prune](https://docs.docker.com/engine/manage-resources/pruning/).

<a id="lifecycle-journal-br-17"></a>

## Lifecycle journal (BR-17)

Постоянный журнал находится в `/app/logs/lifecycle/lifecycle.jsonl`, backups —
`.1`–`.4` (новейший — `.1`). Каждый файл ограничен 1 MiB, всего не более 5 MiB.
Один server process владеет этим каталогом; несколько реплик используют разные
volumes. Новый image создаёт каталог с владельцем `bridge`; новый named volume
получает эти права из image. Для существующего bind mount оператор заранее
обеспечивает права runtime UID, не запуская Bridge от root.

Журнал работает независимо от raw debug level и принимает только фиксированные
события/причины и числовые/boolean поля. `session` — boot-local HMAC-псевдоним,
`boot` меняется при запуске процесса; `sequence` задаёт порядок внутри boot.
Псевдоним связывает события внутри журнала, но не позволяет восстановить raw ID
или сопоставить старый client log без отдельной временной корреляции. Token,
user name, conversation path, prompt/tool payload, stdout/stderr и произвольный
error text в journal не копируются. На рестарте `server_start` добавляется к
существующим файлам. Отсутствие `server_shutdown` совместимо с аварийным exit,
но само по себе не доказывает его причину. Утраченные старые server logs журнал
не восстанавливает. Границы startup/shutdown отмечаются при запуске/остановке
server reaper после HTTP listen/в начале shutdown.

Записи низкой частоты синхронные: нет неограниченной очереди или потерянного
буфера при обычном exit. Ошибка файловой системы не прерывает session lifecycle;
фиксированное предупреждение выводится не чаще раза в минуту. В этом случае
событие может отсутствовать — отсутствие записи не доказывает отсутствие
операции. `BRIDGE_LIFECYCLE_LOG_DIR` меняет каталог для локальных/изолированных
проверок; в compose оставьте default, соответствующий mount.

Оператор может читать или копировать **только** этот каталог без остановки PTY:

```bash
docker exec open-claude-bridge cat /app/logs/lifecycle/lifecycle.jsonl
docker cp open-claude-bridge:/app/logs/lifecycle "$PRIVATE_CAPTURE_DIR/lifecycle"
```

`PRIVATE_CAPTURE_DIR` — новая внешняя private inbox по diagnostic intake, не
checkout проекта. Запишите UTC interval, image digest, container start time,
размеры и SHA-256 всех поколений. Для backup используйте ту же read-only копию
или filesystem snapshot volume. Copy во время rotation является best effort:
проверьте поколения/sequence и повторите capture при разрыве; это не атомарный
snapshot. Не копируйте соседние `errors`, `sessions`, `captured` или `prompts` как
privacy-safe journal. Для Docker замену на Podman выполняйте последовательно
только в своём контуре.

Если оператору нужно удалить journal, сначала сохраните разрешённый backup,
затем удалите только пять имён через `docker exec`, без удаления volume или
остановки активных PTY:

```bash
docker exec open-claude-bridge node -e 'const fs=require("fs"); const base="/app/logs/lifecycle/lifecycle.jsonl"; for(let n=0;n<=4;n++) fs.rmSync(n?base+"."+n:base,{force:true})'
```

Следующее lifecycle-событие создаст новый файл; конкурентная запись при очистке
может быть потеряна. Проверка durability/rotation проводится на disposable
контейнере с отдельным named volume и случайным loopback port, без production
credentials. Только deployment owner подключает новый volume при обычной
выкладке после merge и отдельно подтверждает запись/retention; implementation
agent не пересоздаёт действующий service ради этой задачи.
