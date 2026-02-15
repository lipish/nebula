import { useEffect, useState } from 'react'
import { UserRound } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { authApi } from '@/lib/api'
import type { AuthUser } from '@/lib/types'

interface UserProfileViewProps {
  token: string
  user: AuthUser | null
  onProfileUpdated: () => Promise<void>
}

export function UserProfileView({ token, user, onProfileUpdated }: UserProfileViewProps) {
  const [name, setName] = useState('')
  const [email, setEmail] = useState('')
  const [saved, setSaved] = useState(false)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    setName(user?.display_name || user?.username || '')
    setEmail(user?.email || '')
  }, [user])

  const onSave = async () => {
    setSaving(true)
    setError(null)
    try {
      await authApi.updateProfile(
        {
          display_name: name.trim(),
          email: email.trim(),
        },
        token,
      )
      await onProfileUpdated()
      setSaved(true)
      setTimeout(() => setSaved(false), 1200)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save profile')
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="max-w-2xl space-y-6 animate-in fade-in slide-in-from-bottom-4 duration-700">
      <div>
        <h2 className="text-2xl font-bold text-foreground">Edit Profile</h2>
        <p className="text-sm text-muted-foreground mt-1">Update your basic profile information.</p>
      </div>

      <div className="bg-card border border-border rounded-2xl p-6 space-y-5">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2.5">
            <div className="h-9 w-9 rounded-xl bg-primary/10 text-primary flex items-center justify-center">
              <UserRound className="h-4.5 w-4.5" />
            </div>
            <p className="font-semibold text-foreground">Profile Details</p>
          </div>
          <Badge variant="outline" className="text-[10px] uppercase">PostgreSQL</Badge>
        </div>

        <div className="space-y-2">
          <Label htmlFor="profile-name">Display Name</Label>
          <Input id="profile-name" value={name} onChange={(e) => setName(e.target.value)} placeholder="Your name" />
        </div>

        <div className="space-y-2">
          <Label htmlFor="profile-email">Email</Label>
          <Input id="profile-email" type="email" value={email} onChange={(e) => setEmail(e.target.value)} placeholder="name@company.com" />
        </div>

        {error && <p className="text-sm text-destructive">{error}</p>}

        <div className="flex items-center justify-between pt-2">
          <p className="text-xs text-muted-foreground">Profile data is persisted in backend PostgreSQL.</p>
          <Button onClick={onSave} className="rounded-xl" disabled={saving}>
            {saving ? 'Saving...' : saved ? 'Saved' : 'Save Changes'}
          </Button>
        </div>
      </div>
    </div>
  )
}
