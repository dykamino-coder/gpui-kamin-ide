# Bridge на Linux: ручное развёртывание и обновление

Этот runbook — шаблон для уполномоченного оператора **одного** Bridge-контейнера
во внутреннем контуре. Он не задаёт адрес корпоративного registry, TLS/proxy,
доступ к Docker host или значения credentials. Их владелец хранит в закрытой
deployment-конфигурации. Публикация новой версии происходит через
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
| `/home/bridge/bridge-sync`  | Возможные legacy snapshots                                                                           | Если путь смонтирован в действующем контейнере, сохранять mount и backup до подтверждённой миграции по [BR-13](../RUNTIME_RELIABILITY.md).                                                                 |

Содержимое `/app/installer` поставляется с versioned image. Не подменяйте его
старым bind mount из локального compose: это может показать клиенту не тот
installer, который проверен release pipeline.
Удаление контейнера само по себе не удаляет **named** volumes, но уничтожает
данные вне mounts и прерывает все процессы. `docker volume rm/prune` к рабочим
volumes и backup не применять. Имена volumes из `docker run` и Compose могут
различаться: перед обновлением источником истины является `docker inspect`
действующего контейнера.

## Перед первым запуском или заменой контейнера

1. Уточните у владельца deployment разрешённые registry/image digest, способ
   доступа клиентов (TLS, reverse proxy, VPN), расположение secret file и
   место шифрованных backup. В публичный репозиторий эти сведения не записывайте.
2. Если контейнер уже существует, сверьте его image, **имена и назначения**
   mounts, restart policy и legacy sync mount. Не переносите данные в новые
   volume только потому, что они так названы в примере ниже. Не выводите
   `Config.Env` в ticket или общий лог: там могут быть credentials.

   ```bash
   sudo docker inspect open-claude-bridge \
     --format 'image={{.Config.Image}} mounts={{range .Mounts}}{{.Name}}:{{.Destination}} {{end}} restart={{.HostConfig.RestartPolicy.Name}}'
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
   его пустым поверх уже действующего файла при обновлении.
   `--env-file` убирает пароль из shell history, но **не скрывает** его от
   администраторов Docker и `docker inspect`. Если это не соответствует политике
   контура, нужен утверждённый secret manager и отдельная поддержка file-based
   secret в Bridge; текущий env file не является Docker secret.
   Dashboard credentials защищают dashboard, но не заменяют сетевую изоляцию
   всего Bridge API и TLS. Для первой установки (не при обновлении) volumes
   создаются так:

   ```bash
   sudo docker volume create open-claude-bridge-data
   sudo docker volume create open-claude-bridge-claude
   ```

Если существующий `/home/bridge/.claude.json` содержит настройки, которые
нельзя потерять, остановитесь и согласуйте его перенос/отдельное persistence
до замены контейнера: текущие два mounts этот файл не сохраняют.

## Backup перед обновлением

Остановите контейнер штатно, пока он ещё существует, и снимите backup каждого
фактически смонтированного volume. Для DuckDB нельзя считать простое копирование
живого файла согласованным backup. Пример использует `BACKUP_IMAGE` — заранее
одобренный в контуре utility image с `tar`; подставьте реальный защищённый путь
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
и манифест `image digest + mount names + время + checksums` в закрытом контуре.
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
  "$BRIDGE_IMAGE"
```

`BRIDGE_IMAGE` — утверждённый versioned image/digest из registry, а не
копия команды для предыдущей версии. Если старый контейнер использует
legacy sync mount, добавьте его **с тем же volume** и сохраните до закрытия
BR-13. Для первой установки создайте оба named volumes до `docker run` и
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
