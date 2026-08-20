#!/usr/bin/env bash
# Шардированный свод WPT: делит список пар на N частей и гонит N wptrun-процессов
# параллельно, затем склеивает отчёты. Использование:
#   bash scripts/wpt-shard.sh target/all-nojs.txt 6
# Результат: target/wpt-report-merged.txt + счёт зелёных.
# ВАЖНО: бинарь должен быть уже собран (cargo build --example wptrun -p kamin-html);
# скрипт НЕ вызывает cargo, чтобы шарды не дрались за target-lock.
#
# ВОТЧДОГ: страница, зацикливающая рендер (vars-font-shorthand-001 и родня),
# вешает процесс навсегда — отчёт перестаёт расти. Вотчдог убивает такой
# процесс, помечает ПЕРВУЮ невыполненную пару вердиктом HUNG и продолжает
# шард с хвоста списка.
set -o pipefail
LIST="${1:?список пар}"
N="${2:-6}"
STALL="${WPT_STALL_SECS:-90}"
BIN="target/debug/examples/wptrun.exe"
[ -x "$BIN" ] || { echo "нет бинаря $BIN — сначала cargo build --example wptrun -p kamin-html"; exit 1; }
total=$(grep -c '|' "$LIST")
per=$(( (total + N - 1) / N ))
rm -f target/wpt-shard-*.txt target/wpt-shard-*.list
split -l "$per" -d "$LIST" target/wpt-shard- --additional-suffix=.list

run_shard() { # $1 = list file, $2 = report file
  local list="$1" report="$2" tries=0
  : > "$report"
  local work="$list.work"
  cp "$list" "$work"
  while [ -s "$work" ] && [ "$tries" -lt 50 ]; do
    tries=$((tries+1))
    WPT_REPORT="$report.part" "$BIN" "$work" >/dev/null 2>&1 &
    local pid=$!
    local last=0 quiet=0
    while kill -0 "$pid" 2>/dev/null; do
      sleep 5
      local now
      now=$(grep -c '|' "$report.part" 2>/dev/null || echo 0)
      if [ "$now" -gt "$last" ]; then last=$now; quiet=0; else quiet=$((quiet+5)); fi
      if [ "$quiet" -ge "$STALL" ]; then
        kill -9 "$pid" 2>/dev/null
        wait "$pid" 2>/dev/null
        break
      fi
    done
    wait "$pid" 2>/dev/null
    cat "$report.part" >> "$report" 2>/dev/null
    # осталось = пары work без вердикта; первая из них — висючая (если процесс убит)
    awk -F'|' '{print $1}' "$report" | sort -u > "$report.done"
    grep -vFf "$report.done" "$work" > "$work.rest" || true
    if [ -s "$work.rest" ]; then
      if [ "$last" -ge 0 ] && ! kill -0 "$pid" 2>/dev/null; then
        # процесс мёртв, а пары остались: первую считаем висючей
        head -1 "$work.rest" | awk -F'|' '{print $1"|"$2"|HUNG"}' >> "$report"
        tail -n +2 "$work.rest" > "$work"
      else
        cp "$work.rest" "$work"
      fi
    else
      : > "$work"
    fi
    : > "$report.part"
  done
  rm -f "$work" "$work.rest" "$report.part" "$report.done"
}

pids=()
i=0
for part in target/wpt-shard-*.list; do
  run_shard "$part" "target/wpt-shard-$i.txt" &
  pids+=($!)
  i=$((i+1))
done
fail=0
for p in "${pids[@]}"; do wait "$p" || fail=1; done
cat target/wpt-shard-*.txt > target/wpt-report-merged.txt
got=$(grep -c '|' target/wpt-report-merged.txt)
green=$(awk -F'|' '$3~/^[0-9.]+$/ && $3+0<=0.5' target/wpt-report-merged.txt | wc -l)
hung=$(grep -c '|HUNG' target/wpt-report-merged.txt)
echo "pairs: $got/$total, green: $green, hung: $hung, fail_flag: $fail"
