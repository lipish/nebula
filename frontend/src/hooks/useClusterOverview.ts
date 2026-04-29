import { useQuery } from '@tanstack/react-query'
import { apiGet } from '@/lib/api'
import type { ClusterStatus } from '@/lib/types'
import { useAuthStore } from '@/store/useAuthStore'

export function useClusterOverview() {
  const { token } = useAuthStore()
  
  return useQuery({
    queryKey: ['cluster-overview'],
    queryFn: () => apiGet<ClusterStatus>('/overview', token || undefined),
    enabled: !!token,
    refetchInterval: 10000,
  })
}
