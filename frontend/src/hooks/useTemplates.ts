import { useQuery } from '@tanstack/react-query'
import { v2 } from '@/lib/api'
import { useAuthStore } from '@/store/useAuthStore'

export function useTemplates() {
  const { token } = useAuthStore()
  
  return useQuery({
    queryKey: ['model-templates'],
    queryFn: () => v2.listTemplates(token || ''),
    enabled: !!token,
    refetchInterval: 30000,
  })
}
