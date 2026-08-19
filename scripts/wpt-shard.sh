#!/usr/bin/env bash
# Шардированный свод WPT: делит список пар на N частей и гонит N wptrun-процессов
# параллельно, затем склеивает отчёты. Использование:
#   bash scripts/wpt-shard.sh target/all-nojs.txt 6
# Результат: target/wpt-report-merged.txt + счёт зелёных.
# ВАЖНО: бинарь должен быть уже собран (cargo build --example wptrun -p kamin-html);
# скрипт НЕ вызывает cargo, чтобы шарды не дрались за target-lock.
set -o pipefail
LIST="${1:?список пар}"
N="${2:-6}"
BIN="target/debug/examples/wptrun.exe"
[ -x "$BIN" ] || { echo "нет бинаря $BIN — сначала cargo build --example wptrun -p kamin-html"; exit 1; }
total=$(grep -c '|' "$LIST")
per=$(( (total + N - 1) / N ))
rm -f target/wpt-shard-*.txt target/wpt-shard-*.list
split -l "$per" -d "$LIST" target/wpt-shard- --additional-suffix=.list
pids=()
i=0
for part in target/wpt-shard-*.list; do
  # Каждому шарду — свой файл отчёта через env (wptrun должен уважать WPT_REPORT;
  # если не уважает — добавить в wptrun.rs чтение WPT_REPORT перед хардкодом).
  WPT_REPORT="target/wpt-shard-$i.txt" "$BIN" "$part" >/dev/null 2>&1 &
  pids+=($!)
  i=$((i+1))
done
fail=0
for p in "${pids[@]}"; do wait "$p" || fail=1; done
cat target/wpt-shard-*.txt > target/wpt-report-merged.txt
got=$(grep -c '|' target/wpt-report-merged.txt)
green=$(awk -F'|' '$3~/^[0-9.]+$/ && $3+0<=0.5' target/wpt-report-merged.txt | wc -l)
echo "pairs: $got/$total, green: $green, fail_flag: $fail"
