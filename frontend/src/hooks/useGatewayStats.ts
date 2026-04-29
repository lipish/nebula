import { useQuery } from '@tanstack/react-query'
import { v2 } from '@/lib/api'
import { useAuthStore } from '@/store/useAuthStore'

export function useGatewayStats(windowValue: string) {
  const { token } = useAuthStore()
  
  return useQuery({
    queryKey: ['gateway-stats', windowValue],
    queryFn: async () => {
      const [overview, traffic, reliability, protection, latency] = await Promise.all([
        v2.gatewayOverview(windowValue, token || ''),
        v2.gatewayTraffic(windowValue, token || ''),
        v2.gatewayReliability(windowValue, token || ''),
        v2.gatewayProtection(windowValue, token || ''),
        v2.gatewayLatency(windowValue, token || ''),
      ])
      return { overview, traffic, reliability, protection, latency }
    },
    enabled: !!token,
    refetchInterval: 30000,
  })
}
