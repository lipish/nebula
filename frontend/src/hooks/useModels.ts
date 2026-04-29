import { useQuery } from '@tanstack/react-query'
import { v2 } from '@/lib/api'
import { useAuthStore } from '@/store/useAuthStore'

export function useModels() {
  const { token } = useAuthStore()
  
  return useQuery({
    queryKey: ['models'],
    queryFn: () => v2.listModels(token || ''),
    enabled: !!token,
    refetchInterval: 10000,
  })
}
