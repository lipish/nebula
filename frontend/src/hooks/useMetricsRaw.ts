import { useQuery } from '@tanstack/react-query'
import { apiGet } from '@/lib/api'
import { useAuthStore } from '@/store/useAuthStore'

export function useMetricsRaw() {
  const { token } = useAuthStore()
  
  return useQuery({
    queryKey: ['metrics-raw'],
    queryFn: () => apiGet<string>('/observe/metrics', token || ''),
    enabled: !!token,
    refetchInterval: 15000,
  })
}
