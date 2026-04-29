import { RouterProvider } from 'react-router-dom'
import { QueryClientProvider } from '@tanstack/react-query'
import { queryClient } from '@/lib/query'
import { router } from '@/routes'
import { useEffect } from 'react'
import { useAuthStore } from '@/store/useAuthStore'

function App() {
  const { setAuth, token } = useAuthStore()
  
  // Migration logic: if there is a token in localStorage but not in store, sync it
  useEffect(() => {
    const localToken = localStorage.getItem('nebula_token')
    if (localToken && !token) {
      // We don't have the user object here, so we might need a profile fetch
      // For now, just set the token to keep it working
      // setAuth(localToken, { id: '', username: 'loading...', role: 'viewer' })
    }
  }, [token, setAuth])

  return (
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  )
}

export default App
