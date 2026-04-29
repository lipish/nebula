import { useQuery } from '@tanstack/react-query'
import { apiGet } from '@/lib/api'
import { useAuthStore } from '@/store/useAuthStore'

export function useCacheSummary() {
  const { token } = useAuthStore()
  
  return useQuery({
    queryKey: ['cache-summary'],
    queryFn: () => apiGet<any>('/v2/cache/summary', token || ''),
    enabled: !!token,
    refetchInterval: 10000,
  })
}
