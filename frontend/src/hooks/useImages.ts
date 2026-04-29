import { useQuery } from '@tanstack/react-query'
import { apiGet } from '@/lib/api'
import type { EngineImage, NodeImageStatus } from '@/lib/types'
import { useAuthStore } from '@/store/useAuthStore'

export function useImages() {
  const { token } = useAuthStore()
  
  return useQuery({
    queryKey: ['engine-images'],
    queryFn: async () => {
      const [images, statuses] = await Promise.all([
        apiGet<EngineImage[]>('/images', token || ''),
        apiGet<NodeImageStatus[]>('/images/status', token || ''),
      ])
      return { images, statuses }
    },
    enabled: !!token,
    refetchInterval: 15000,
  })
}
