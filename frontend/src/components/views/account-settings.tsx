import { useEffect, useState } from 'react'
import { ShieldCheck } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { authApi } from '@/lib/api'
import type { AuthUser, ManagedUser } from '@/lib/types'

interface AccountSettingsViewProps {
  token: string
  user: AuthUser | null
  onOpenSecuritySettings: () => void
}

export function AccountSettingsView({ token, user, onOpenSecuritySettings }: AccountSettingsViewProps) {
  const [emailAlerts, setEmailAlerts] = useState(false)
  const [inAppAlerts, setInAppAlerts] = useState(true)
  const [saved, setSaved] = useState(false)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const isAdmin = user?.role === 'admin'
  const [users, setUsers] = useState<ManagedUser[]>([])
  const [newUsername, setNewUsername] = useState('')
  const [newPassword, setNewPassword] = useState('')
  const [newRole, setNewRole] = useState<'admin' | 'operator' | 'viewer'>('viewer')

  const load = async () => {
    setLoading(true)
    setError(null)
    try {
      const settings = await authApi.getSettings(token)
      setInAppAlerts(settings.in_app_alerts)
      setEmailAlerts(settings.email_alerts)
      if (isAdmin) {
        setUsers(await authApi.listUsers(token))
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load account settings')
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void load()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [token, isAdmin])

  const savePreferences = async () => {
    setError(null)
    try {
      await authApi.updateSettings(
        {
          in_app_alerts: inAppAlerts,
          email_alerts: emailAlerts,
        },
        token,
      )
      setSaved(true)
      setTimeout(() => setSaved(false), 1200)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save preferences')
    }
  }

  const createUser = async () => {
    if (!newUsername.trim() || !newPassword.trim()) return
    setError(null)
    try {
      await authApi.createUser(
        {
          username: newUsername.trim(),
          password: newPassword,
          role: newRole,
        },
        token,
      )
      setNewUsername('')
      setNewPassword('')
      setNewRole('viewer')
      setUsers(await authApi.listUsers(token))
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create user')
    }
  }

  const toggleUserActive = async (u: ManagedUser) => {
    setError(null)
    try {
      await authApi.updateUser(u.id, { is_active: !u.is_active }, token)
      setUsers(await authApi.listUsers(token))
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update user')
    }
  }

  const removeUser = async (u: ManagedUser) => {
    setError(null)
    try {
      await authApi.deleteUser(u.id, token)
      setUsers(await authApi.listUsers(token))
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to delete user')
    }
  }

  return (
    <div className="max-w-2xl space-y-6 animate-in fade-in slide-in-from-bottom-4 duration-700">
      <div>
        <h2 className="text-2xl font-bold text-foreground">Account Settings</h2>
        <p className="text-sm text-muted-foreground mt-1">Manage account preferences and security options.</p>
      </div>

      <div className="bg-card border border-border rounded-2xl p-6 space-y-5">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2.5">
            <div className="h-9 w-9 rounded-xl bg-primary/10 text-primary flex items-center justify-center">
              <ShieldCheck className="h-4.5 w-4.5" />
            </div>
            <p className="font-semibold text-foreground">Preferences</p>
          </div>
          <Badge variant="outline" className="text-[10px] uppercase">PostgreSQL</Badge>
        </div>

        {loading && <p className="text-sm text-muted-foreground">Loading settings...</p>}
        {error && <p className="text-sm text-destructive">{error}</p>}

        <label className="flex items-center justify-between rounded-xl border border-border px-4 py-3 cursor-pointer">
          <div>
            <p className="text-sm font-medium text-foreground">In-App Alerts</p>
            <p className="text-xs text-muted-foreground">Show alert indicators in the top-right bell.</p>
          </div>
          <input type="checkbox" checked={inAppAlerts} onChange={(e) => setInAppAlerts(e.target.checked)} className="h-4 w-4" disabled={loading} />
        </label>

        <label className="flex items-center justify-between rounded-xl border border-border px-4 py-3 cursor-pointer">
          <div>
            <p className="text-sm font-medium text-foreground">Email Alerts</p>
            <p className="text-xs text-muted-foreground">Receive critical notices by email.</p>
          </div>
          <input type="checkbox" checked={emailAlerts} onChange={(e) => setEmailAlerts(e.target.checked)} className="h-4 w-4" disabled={loading} />
        </label>

        <div className="rounded-xl bg-accent/40 px-4 py-3 text-xs text-muted-foreground">
          User profile/account settings are persisted in PostgreSQL and shared across sessions.
        </div>

        <div className="flex items-center justify-between pt-1">
          <Button variant="outline" onClick={onOpenSecuritySettings} className="rounded-xl">
            Open Security Settings
          </Button>
          <Button onClick={savePreferences} className="rounded-xl">
            {saved ? 'Saved' : 'Save Preferences'}
          </Button>
        </div>
      </div>

      {isAdmin && (
        <div className="bg-card border border-border rounded-2xl p-6 space-y-4">
          <div className="flex items-center justify-between">
            <h3 className="text-lg font-bold text-foreground">User Management</h3>
            <Badge variant="outline" className="text-[10px] uppercase">Admin</Badge>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-4 gap-2">
            <Input placeholder="Username" value={newUsername} onChange={(e) => setNewUsername(e.target.value)} />
            <Input placeholder="Password" type="password" value={newPassword} onChange={(e) => setNewPassword(e.target.value)} />
            <select
              value={newRole}
              onChange={(e) => setNewRole(e.target.value as 'admin' | 'operator' | 'viewer')}
              className="h-10 rounded-md border border-input bg-background px-3 text-sm"
            >
              <option value="viewer">viewer</option>
              <option value="operator">operator</option>
              <option value="admin">admin</option>
            </select>
            <Button onClick={createUser} className="rounded-xl">Create User</Button>
          </div>

          <div className="space-y-2">
            {users.map((u) => (
              <div key={u.id} className="rounded-xl border border-border px-4 py-2.5 flex items-center justify-between">
                <div>
                  <p className="text-sm font-medium text-foreground">{u.username} <span className="text-xs text-muted-foreground">({u.role})</span></p>
                  <p className="text-xs text-muted-foreground">{u.display_name || '—'} · {u.email || '—'} · {u.is_active ? 'active' : 'disabled'}</p>
                </div>
                <div className="flex items-center gap-2">
                  {u.username !== 'admin' && (
                    <>
                      <Button variant="outline" size="sm" className="rounded-xl" onClick={() => toggleUserActive(u)}>
                        {u.is_active ? 'Disable' : 'Enable'}
                      </Button>
                      <Button variant="destructive" size="sm" className="rounded-xl" onClick={() => removeUser(u)}>
                        Delete
                      </Button>
                    </>
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      <div className="bg-card border border-border rounded-2xl p-5 space-y-2">
        <Label className="text-xs uppercase text-muted-foreground">Backend Status</Label>
        <p className="text-sm text-foreground">User auth/profile/settings are now served by backend PostgreSQL APIs.</p>
      </div>
    </div>
  )
}
