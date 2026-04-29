import { useQuery } from '@tanstack/react-query'
import { apiGet } from '@/lib/api'
import type { EndpointStats } from '@/lib/types'
import { useAuthStore } from '@/store/useAuthStore'

export function useEngineStats() {
  const { token } = useAuthStore()
  
  return useQuery({
    queryKey: ['engine-stats'],
    queryFn: () => apiGet<EndpointStats[]>('/engine-stats', token || undefined),
    enabled: !!token,
    refetchInterval: 10000,
  })
}
