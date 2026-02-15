import { useCallback, useEffect, useState } from 'react'
import { FolderInput, Loader2, RefreshCw, Trash2 } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { apiGet } from '@/lib/api'
import { v2 } from '@/lib/api'
import type { ModelView } from '@/lib/types'

interface ModelLibraryViewProps {
  token: string
  onOpenService: (uid: string) => void
}

interface CacheSummary {
  caches: Array<{
    model_name: string
    node_id: string
    size_bytes: number
    matched_model_uids?: string[]
  }>
}

export function ModelLibraryView({ token, onOpenService }: ModelLibraryViewProps) {
  const [models, setModels] = useState<ModelView[]>([])
  const [cacheSummary, setCacheSummary] = useState<CacheSummary | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [actingUid, setActingUid] = useState<string | null>(null)

  const [moveDialogOpen, setMoveDialogOpen] = useState(false)
  const [selectedModel, setSelectedModel] = useState<ModelView | null>(null)
  const [newPath, setNewPath] = useState('')

  const refresh = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const [list, cache] = await Promise.all([
        v2.listModels(token),
        apiGet<CacheSummary>('/v2/cache/summary', token),
      ])
      setModels(list)
      setCacheSummary(cache)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load model library')
    } finally {
      setLoading(false)
    }
  }, [token])

  useEffect(() => {
    refresh()
  }, [refresh])

  useEffect(() => {
    const id = setInterval(refresh, 10000)
    return () => clearInterval(id)
  }, [refresh])

  const handleDelete = async (uid: string) => {
    setActingUid(uid)
    setError(null)
    try {
      await v2.deleteModel(uid, token)
      await refresh()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to delete model')
    } finally {
      setActingUid(null)
    }
  }

  const cacheMap = useCallback((modelUid: string, modelName: string) => {
    const matched = (cacheSummary?.caches || []).filter((item) => {
      if ((item.matched_model_uids || []).includes(modelUid)) {
        return true
      }
      return item.model_name === modelName
    })
    return {
      nodes: new Set(matched.map((item) => item.node_id)).size,
      bytes: matched.reduce((sum, item) => sum + item.size_bytes, 0),
    }
  }, [cacheSummary])

  const openMoveDialog = (model: ModelView) => {
    setSelectedModel(model)
    setNewPath('')
    setMoveDialogOpen(true)
  }

  const handleMove = async () => {
    if (!selectedModel || !newPath.trim()) return
    setActingUid(selectedModel.model_uid)
    setError(null)
    try {
      await v2.updateModel(selectedModel.model_uid, { model_path: newPath.trim() }, token)
      setMoveDialogOpen(false)
      setSelectedModel(null)
      setNewPath('')
      await refresh()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update model path')
    } finally {
      setActingUid(null)
    }
  }

  return (
    <div className="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-700">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold text-foreground">Model Library</h2>
          <p className="text-sm text-muted-foreground mt-1">Manage downloaded model assets and storage path</p>
        </div>
        <Button variant="outline" size="sm" onClick={refresh} disabled={loading} className="rounded-xl">
          <RefreshCw className={`h-4 w-4 mr-1.5 ${loading ? 'animate-spin' : ''}`} />
          Refresh
        </Button>
      </div>

      {error && (
        <div className="bg-destructive/10 border border-destructive/20 rounded-xl px-4 py-3 text-sm text-destructive">
          {error}
        </div>
      )}

      <div className="bg-card border border-border rounded-2xl shadow-sm overflow-hidden">
        <div className="px-6 py-5 border-b border-border bg-accent/30 flex items-center justify-between">
          <div>
            <h3 className="text-lg font-bold text-foreground tracking-tight">Downloaded Assets</h3>
          </div>
          <Badge variant="outline" className="font-bold border-primary/20 text-primary uppercase h-6">
            {models.length} Models
          </Badge>
        </div>

        <Table>
          <TableHeader>
            <TableRow className="bg-muted hover:bg-muted border-b border-border">
              <TableHead className="text-[11px] font-bold text-muted-foreground uppercase py-4">Model</TableHead>
              <TableHead className="text-[11px] font-bold text-muted-foreground uppercase py-4">Asset Status</TableHead>
              <TableHead className="text-[11px] font-bold text-muted-foreground uppercase py-4">Service Status</TableHead>
              <TableHead className="text-[11px] font-bold text-muted-foreground uppercase py-4">Replicas</TableHead>
              <TableHead className="text-right text-[11px] font-bold text-muted-foreground uppercase py-4">Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {loading ? (
              <TableRow>
                <TableCell colSpan={5} className="h-40 text-center">
                  <Loader2 className="h-5 w-5 animate-spin mx-auto mb-2 text-muted-foreground" />
                  <p className="text-sm text-muted-foreground">Loading model library…</p>
                </TableCell>
              </TableRow>
            ) : models.length === 0 ? (
              <TableRow>
                <TableCell colSpan={5} className="h-40 text-center text-sm text-muted-foreground">
                  No downloaded models found.
                </TableCell>
              </TableRow>
            ) : (
              models.map((model) => {
                const acting = actingUid === model.model_uid
                const cache = cacheMap(model.model_uid, model.model_name)
                return (
                  <TableRow key={model.model_uid}>
                    <TableCell className="py-4">
                      <div className="font-bold text-sm tracking-tight">{model.model_uid}</div>
                      <div className="text-[10px] text-muted-foreground mt-0.5 max-w-[300px] truncate">{model.model_name}</div>
                    </TableCell>
                    <TableCell>
                      {cache.bytes > 0 ? (
                        <Badge variant="secondary" className="text-[11px] font-bold uppercase">Downloaded</Badge>
                      ) : (
                        <Badge variant="outline" className="text-[11px] font-bold uppercase">Pending</Badge>
                      )}
                      <div className="text-[10px] text-muted-foreground mt-1">
                        {cache.nodes} node(s) cached
                      </div>
                    </TableCell>
                    <TableCell>
                      <Badge variant="outline" className="text-[11px] font-bold uppercase">{model.state}</Badge>
                    </TableCell>
                    <TableCell>
                      <span className="text-sm font-bold">{model.replicas.ready}/{model.replicas.desired}</span>
                    </TableCell>
                    <TableCell className="text-right">
                      <div className="inline-flex items-center gap-2">
                        <Button size="sm" variant="outline" className="rounded-xl h-8" onClick={() => onOpenService(model.model_uid)}>
                          Open Service
                        </Button>
                        <Button
                          size="sm"
                          variant="outline"
                          className="rounded-xl h-8"
                          onClick={() => openMoveDialog(model)}
                          disabled={acting}
                        >
                          <FolderInput className="h-3.5 w-3.5 mr-1" /> Move
                        </Button>
                        <Button
                          size="sm"
                          variant="destructive"
                          className="rounded-xl h-8"
                          onClick={() => handleDelete(model.model_uid)}
                          disabled={acting}
                        >
                          <Trash2 className="h-3.5 w-3.5 mr-1" /> Delete
                        </Button>
                      </div>
                    </TableCell>
                  </TableRow>
                )
              })
            )}
          </TableBody>
        </Table>
      </div>

      <Dialog open={moveDialogOpen} onOpenChange={setMoveDialogOpen}>
        <DialogContent className="sm:max-w-[520px]">
          <DialogHeader>
            <DialogTitle>Move Model Storage Path</DialogTitle>
          </DialogHeader>

          <div className="space-y-3 py-2">
            <p className="text-sm text-muted-foreground break-all">Model: {selectedModel?.model_uid || '—'}</p>
            <div className="space-y-2">
              <Label htmlFor="library-move-path">New Path</Label>
              <Input
                id="library-move-path"
                placeholder="e.g. /DATA/Model or /mnt/models"
                value={newPath}
                onChange={(e) => setNewPath(e.target.value)}
              />
              <p className="text-xs text-muted-foreground">This updates model download/storage path configuration for subsequent operations.</p>
            </div>
          </div>

          <DialogFooter>
            <Button variant="outline" onClick={() => setMoveDialogOpen(false)}>Cancel</Button>
            <Button onClick={handleMove} disabled={!newPath.trim() || !selectedModel}>Save</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
