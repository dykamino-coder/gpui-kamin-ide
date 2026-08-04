import { signal } from '@preact/signals'
import { useEffect, useCallback } from 'preact/hooks'
import type { RequestLogEntry } from '../../types'
import type { StatFilter } from '../dashboard/StatsGrid'
import { api } from '../../services/api-client'
import { RequestTable } from './RequestTable'
import styles from './RequestListModal.module.css'

const PAGE_SIZE = 50

const requests = signal<RequestLogEntry[]>([])
const loading = signal(false)
const loadingMore = signal(false)
const hasMore = signal(false)
const totalLoaded = signal(0)

function buildFilter(filter: StatFilter): Record<string, string> {
  const f: Record<string, string> = {}
  if (filter.endpoint) f.endpoint = filter.endpoint
  if (filter.userName) f.userName = filter.userName
  if (filter.status) f.status = filter.status
  return f
}

interface RequestListModalProps {
  filter: StatFilter
}

export function RequestListModal({ filter }: RequestListModalProps) {
  const fetchInitial = () => {
    loading.value = true
    requests.value = []
    totalLoaded.value = 0
    api.getFilteredRequests(buildFilter(filter), PAGE_SIZE, 0)
      .then((data) => {
        requests.value = data
        totalLoaded.value = data.length
        hasMore.value = data.length === PAGE_SIZE
      })
      .catch(() => { requests.value = [] })
      .finally(() => { loading.value = false })
  }

  const fetchMore = useCallback(() => {
    if (loadingMore.value || !hasMore.value) return
    loadingMore.value = true
    const offset = totalLoaded.value
    api.getFilteredRequests(buildFilter(filter), PAGE_SIZE, offset)
      .then((data) => {
        requests.value = [...requests.value, ...data]
        totalLoaded.value = totalLoaded.value + data.length
        hasMore.value = data.length === PAGE_SIZE
      })
      .catch(() => {})
      .finally(() => { loadingMore.value = false })
  }, [filter])

  useEffect(() => {
    fetchInitial()
  }, [filter.label, filter.endpoint, filter.pluginId, filter.userName, filter.status])

  const list = requests.value
  const isLoading = loading.value
  const isLoadingMore = loadingMore.value

  return (
    <div class={styles.container}>
      <div class={styles.headerRow}>
        <span class={styles.filterLabel}>
          <i class="fa-solid fa-filter" />
          {filter.label}
        </span>
        <span class={styles.count}>
          {isLoading ? 'Loading...' : `${list.length} results${hasMore.value ? '+' : ''}`}
        </span>
      </div>

      <RequestTable
        entries={list}
        onLoadMore={fetchMore}
        loading={isLoading}
        loadingMore={isLoadingMore}
        maxHeight={400}
      />
    </div>
  )
}
