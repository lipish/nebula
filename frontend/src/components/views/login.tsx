import { useState } from 'react'
import { Eye, EyeOff, KeyRound, UserRound } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { authApi } from '@/lib/api'
import type { AuthUser } from '@/lib/types'

interface LoginViewProps {
  onLoginSuccess: (token: string, user: AuthUser) => void
}

export function LoginView({ onLoginSuccess }: LoginViewProps) {
  const [username, setUsername] = useState('admin')
  const [password, setPassword] = useState('admin123')
  const [remember, setRemember] = useState(true)
  const [showPassword, setShowPassword] = useState(false)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const submit = async () => {
    setLoading(true)
    setError(null)
    try {
      const result = await authApi.login(username.trim(), password)
      onLoginSuccess(result.token, result.user)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Login failed')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="min-h-screen w-full bg-background">
      <div className="min-h-screen w-full overflow-hidden grid grid-cols-1 lg:grid-cols-2">
        <div className="bg-gradient-to-br from-slate-950 via-slate-900 to-slate-950 text-white px-10 py-10 flex flex-col">
          <div className="flex items-center gap-3">
            <div className="h-8 w-8 rounded-lg border border-white/30 flex items-center justify-center">◈</div>
            <p className="text-3xl font-semibold tracking-tight">Nebula</p>
          </div>

          <div className="mt-auto mb-auto max-w-md">
            <h1 className="text-5xl font-bold leading-tight">Enterprise AI Model Management Platform</h1>
            <p className="mt-5 text-lg text-white/70 leading-8">
              Deploy, manage, and scale your AI models with confidence. Built for teams that demand reliability.
            </p>
          </div>

          <p className="text-sm text-white/40">© 2026 Nebula. All rights reserved.</p>
        </div>

        <div className="bg-background px-8 lg:px-16 py-10 flex items-center justify-center">
          <div className="w-full max-w-sm space-y-5">
            <div>
              <h2 className="text-4xl font-semibold text-foreground">Sign in</h2>
              <p className="text-sm text-muted-foreground mt-2">Enter your credentials to access the platform</p>
            </div>

            <div className="space-y-2">
              <Label htmlFor="login-username" className="text-sm">Email</Label>
              <div className="relative">
                <UserRound className="h-4 w-4 text-muted-foreground absolute left-3 top-1/2 -translate-y-1/2" />
                <Input
                  id="login-username"
                  className="pl-9 h-11 rounded-lg"
                  placeholder="name@company.com"
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                />
              </div>
            </div>

            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <Label htmlFor="login-password" className="text-sm">Password</Label>
                <button type="button" className="text-xs text-muted-foreground hover:text-foreground">Forgot password?</button>
              </div>
              <div className="relative">
                <KeyRound className="h-4 w-4 text-muted-foreground absolute left-3 top-1/2 -translate-y-1/2" />
                <Input
                  id="login-password"
                  type={showPassword ? 'text' : 'password'}
                  className="pl-9 pr-10 h-11 rounded-lg"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') submit()
                  }}
                />
                <button
                  type="button"
                  onClick={() => setShowPassword((v) => !v)}
                  className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                >
                  {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                </button>
              </div>
            </div>

            <label className="flex items-center gap-2 text-sm text-muted-foreground cursor-pointer">
              <input
                type="checkbox"
                className="h-4 w-4"
                checked={remember}
                onChange={(e) => setRemember(e.target.checked)}
              />
              Remember me
            </label>

            {error && <p className="text-sm text-destructive">{error}</p>}

            <Button className="w-full rounded-lg h-11" onClick={submit} disabled={loading}>
              {loading ? 'Signing in...' : 'Sign in'}
            </Button>

            <p className="text-sm text-muted-foreground text-center">
              Don't have an account? <span className="font-medium text-foreground">Contact admin</span>
            </p>

            <p className="text-xs text-muted-foreground text-center">Demo: admin / admin123</p>
          </div>
        </div>
      </div>
    </div>
  )
}
