import { useCallback, useEffect, useState } from "react"
import { Plus, Trash2, Container, RefreshCw, CheckCircle2, XCircle, Loader2, Clock } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from "@/components/ui/dialog"
import { apiGet, apiPut, apiDelete } from "@/lib/api"
import type { EngineImage, NodeImageStatus } from "@/lib/types"

interface ImagesViewProps {
  token: string
}

const statusIcon = (status: string) => {
  switch (status) {
    case "ready":
      return <CheckCircle2 className="h-4 w-4 text-emerald-500" />
    case "pulling":
      return <Loader2 className="h-4 w-4 text-blue-500 animate-spin" />
    case "failed":
      return <XCircle className="h-4 w-4 text-red-500" />
    default:
      return <Clock className="h-4 w-4 text-muted-foreground" />
  }
}

const EMPTY_FORM: Omit<EngineImage, "created_at_ms" | "updated_at_ms"> = {
  id: "",
  engine_type: "vllm",
  image: "",
  platforms: [],
  version_policy: "pin",
  pre_pull: true,
  description: "",
}

export function ImagesView({ token }: ImagesViewProps) {
  const [images, setImages] = useState<EngineImage[]>([])
  const [statuses, setStatuses] = useState<NodeImageStatus[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [dialogOpen, setDialogOpen] = useState(false)
  const [editingId, setEditingId] = useState<string | null>(null)
  const [form, setForm] = useState(EMPTY_FORM)
  const [platformInput, setPlatformInput] = useState("")
  const [expandedImage, setExpandedImage] = useState<string | null>(null)

  const refresh = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const [imgs, sts] = await Promise.all([
        apiGet<EngineImage[]>("/images", token),
        apiGet<NodeImageStatus[]>("/images/status", token),
      ])
      setImages(imgs)
      setStatuses(sts)
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load images")
    } finally {
      setLoading(false)
    }
  }, [token])

  useEffect(() => {
    refresh()
  }, [refresh])

  useEffect(() => {
    const id = setInterval(refresh, 15000)
    return () => clearInterval(id)
  }, [refresh])

  const openCreate = () => {
    setEditingId(null)
    setForm({ ...EMPTY_FORM })
    setPlatformInput("")
    setDialogOpen(true)
  }

  const openEdit = (img: EngineImage) => {
    setEditingId(img.id)
    setForm({
      id: img.id,
      engine_type: img.engine_type,
      image: img.image,
      platforms: img.platforms,
      version_policy: img.version_policy,
      pre_pull: img.pre_pull,
      description: img.description || "",
    })
    setPlatformInput(img.platforms.join(", "))
    setDialogOpen(true)
  }

  const handleSave = async () => {
    if (!form.id || !form.image) return
    setError(null)
    try {
      const payload = {
        ...form,
        platforms: platformInput
          .split(",")
          .map((s) => s.trim())
          .filter(Boolean),
        created_at_ms: 0,
        updated_at_ms: 0,
      }
      await apiPut(`/images/${form.id}`, payload, token)
      setDialogOpen(false)
      await refresh()
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to save image")
    }
  }

  const handleDelete = async (id: string) => {
    setError(null)
    try {
      await apiDelete(`/images/${id}`, token)
      await refresh()
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to delete image")
    }
  }

  const nodeStatuses = (imageId: string) =>
    statuses.filter((s) => s.image_id === imageId)

  return (
    <div className="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-700">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold text-foreground">Image Registry</h2>
          <p className="text-sm text-muted-foreground mt-1">
            Manage engine Docker images across the cluster
          </p>
        </div>
        <div className="flex gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={refresh}
            disabled={loading}
            className="rounded-xl"
          >
            <RefreshCw className={`h-4 w-4 mr-1.5 ${loading ? "animate-spin" : ""}`} />
            Refresh
          </Button>
          <Button
            onClick={openCreate}
            className="bg-primary hover:bg-primary/90 rounded-xl shadow-sm px-5"
          >
            <Plus className="mr-2 h-4 w-4" />
            Register Image
          </Button>
        </div>
      </div>

      {error && (
        <div className="bg-destructive/10 border border-destructive/20 rounded-xl px-4 py-3 text-sm text-destructive">
          {error}
        </div>
      )}

      {/* Image List */}
      <div className="bg-card border border-border rounded-2xl shadow-sm overflow-hidden">
        <div className="px-6 py-5 border-b border-border bg-accent/30 flex items-center justify-between">
          <div>
            <h3 className="text-lg font-bold text-foreground tracking-tight">
              Registered Images
            </h3>
          </div>
          <Badge
            variant="outline"
            className="font-bold border-primary/20 text-primary uppercase h-6"
          >
            {images.length} Total
          </Badge>
        </div>

        <Table>
          <TableHeader>
            <TableRow className="bg-muted hover:bg-muted border-b border-border">
              <TableHead className="text-[11px] font-bold text-muted-foreground uppercase pl-6 pr-4 py-4">
                ID
              </TableHead>
              <TableHead className="text-[11px] font-bold text-muted-foreground uppercase px-4 py-4">
                Engine
              </TableHead>
              <TableHead className="text-[11px] font-bold text-muted-foreground uppercase px-4 py-4">
                Image
              </TableHead>
              <TableHead className="text-[11px] font-bold text-muted-foreground uppercase px-4 py-4">
                Policy
              </TableHead>
              <TableHead className="text-[11px] font-bold text-muted-foreground uppercase px-4 py-4">
                Node Status
              </TableHead>
              <TableHead className="text-right text-[11px] font-bold text-muted-foreground uppercase pl-4 pr-6 py-4">
                Actions
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {images.length === 0 ? (
              <TableRow>
                <TableCell colSpan={6} className="h-48 text-center">
                  <div className="flex flex-col items-center justify-center opacity-40">
                    <Container className="h-12 w-12 mb-2" />
                    <p className="text-sm font-bold text-muted-foreground">
                      No images registered yet.
                    </p>
                  </div>
                </TableCell>
              </TableRow>
            ) : (
              images.map((img) => {
                const ns = nodeStatuses(img.id)
                const readyCount = ns.filter((s) => s.status === "ready").length
                const totalCount = ns.length
                const isExpanded = expandedImage === img.id

                return (
                  <TableRow key={img.id} className="group">
                    <TableCell className="pl-6 py-4">
                      <button
                        onClick={() =>
                          setExpandedImage(isExpanded ? null : img.id)
                        }
                        className="text-left"
                      >
                        <div className="font-bold text-sm tracking-tight">
                          {img.id}
                        </div>
                        {img.description && (
                          <div className="text-[10px] text-muted-foreground mt-0.5 max-w-[180px] truncate">
                            {img.description}
                          </div>
                        )}
                      </button>
                    </TableCell>
                    <TableCell>
                      <Badge variant="outline" className="font-bold text-[11px] uppercase">
                        {img.engine_type}
                      </Badge>
                    </TableCell>
                    <TableCell>
                      <div className="font-mono text-xs bg-accent/40 inline-block px-2 py-0.5 rounded max-w-[280px] truncate">
                        {img.image}
                      </div>
                      {img.platforms.length > 0 && (
                        <div className="flex gap-1 mt-1">
                          {img.platforms.map((p) => (
                            <span
                              key={p}
                              className="text-[9px] bg-accent/60 px-1.5 py-0.5 rounded text-muted-foreground"
                            >
                              {p}
                            </span>
                          ))}
                        </div>
                      )}
                    </TableCell>
                    <TableCell>
                      <div className="flex items-center gap-2">
                        <Badge
                          variant={img.version_policy === "rolling" ? "default" : "secondary"}
                          className="text-[10px] font-bold uppercase"
                        >
                          {img.version_policy}
                        </Badge>
                        {img.pre_pull && (
                          <span className="text-[9px] text-muted-foreground">
                            auto-pull
                          </span>
                        )}
                      </div>
                    </TableCell>
                    <TableCell>
                      <button
                        onClick={() =>
                          setExpandedImage(isExpanded ? null : img.id)
                        }
                        className="text-left"
                      >
                        {totalCount > 0 ? (
                          <span className="text-xs font-bold">
                            <span className="text-emerald-500">{readyCount}</span>
                            <span className="text-muted-foreground">/{totalCount} ready</span>
                          </span>
                        ) : (
                          <span className="text-xs text-muted-foreground">—</span>
                        )}
                      </button>

                      {/* Expanded node status */}
                      {isExpanded && ns.length > 0 && (
                        <div className="mt-2 space-y-1">
                          {ns.map((s) => (
                            <div
                              key={`${s.node_id}-${s.image_id}`}
                              className="flex items-center gap-2 text-xs"
                            >
                              {statusIcon(s.status)}
                              <span className="font-medium">{s.node_id}</span>
                              <span className="text-muted-foreground">
                                {s.status}
                              </span>
                              {s.error && (
                                <span className="text-destructive text-[10px] truncate max-w-[200px]">
                                  {s.error}
                                </span>
                              )}
                            </div>
                          ))}
                        </div>
                      )}
                    </TableCell>
                    <TableCell className="text-right pr-6">
                      <div className="flex justify-end gap-1">
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => openEdit(img)}
                          className="text-xs font-bold rounded-xl h-8"
                        >
                          Edit
                        </Button>
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => handleDelete(img.id)}
                          className="text-destructive font-bold text-xs hover:text-white hover:bg-destructive rounded-xl h-8"
                        >
                          <Trash2 className="h-3.5 w-3.5" />
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

      {/* Create / Edit Dialog */}
      <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
        <DialogContent className="sm:max-w-[480px]">
          <DialogHeader>
            <DialogTitle>
              {editingId ? "Edit Image" : "Register New Image"}
            </DialogTitle>
          </DialogHeader>

          <div className="space-y-4 py-2">
            <div className="space-y-2">
              <Label htmlFor="img-id">Image ID</Label>
              <Input
                id="img-id"
                placeholder="e.g. vllm-cuda124"
                value={form.id}
                onChange={(e) => setForm({ ...form, id: e.target.value })}
                disabled={!!editingId}
              />
            </div>

            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-2">
                <Label htmlFor="img-engine">Engine Type</Label>
                <select
                  id="img-engine"
                  value={form.engine_type}
                  onChange={(e) =>
                    setForm({ ...form, engine_type: e.target.value })
                  }
                  className="flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
                >
                  <option value="vllm">vLLM</option>
                  <option value="sglang">SGLang</option>
                  <option value="xllm">xLLM</option>
                </select>
              </div>

              <div className="space-y-2">
                <Label htmlFor="img-policy">Version Policy</Label>
                <select
                  id="img-policy"
                  value={form.version_policy}
                  onChange={(e) =>
                    setForm({
                      ...form,
                      version_policy: e.target.value as "pin" | "rolling",
                    })
                  }
                  className="flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
                >
                  <option value="pin">Pin (stable)</option>
                  <option value="rolling">Rolling (latest)</option>
                </select>
              </div>
            </div>

            <div className="space-y-2">
              <Label htmlFor="img-ref">Docker Image</Label>
              <Input
                id="img-ref"
                placeholder="e.g. vllm/vllm-openai:v0.8.3"
                value={form.image}
                onChange={(e) => setForm({ ...form, image: e.target.value })}
              />
            </div>

            <div className="space-y-2">
              <Label htmlFor="img-platforms">
                Platforms{" "}
                <span className="text-muted-foreground font-normal">
                  (comma-separated, empty = all)
                </span>
              </Label>
              <Input
                id="img-platforms"
                placeholder="e.g. nvidia-cuda, ascend-cann8"
                value={platformInput}
                onChange={(e) => setPlatformInput(e.target.value)}
              />
            </div>

            <div className="space-y-2">
              <Label htmlFor="img-desc">Description</Label>
              <Input
                id="img-desc"
                placeholder="Optional description"
                value={form.description || ""}
                onChange={(e) =>
                  setForm({ ...form, description: e.target.value })
                }
              />
            </div>

            <div className="flex items-center gap-2">
              <input
                type="checkbox"
                id="img-prepull"
                checked={form.pre_pull}
                onChange={(e) =>
                  setForm({ ...form, pre_pull: e.target.checked })
                }
                className="rounded border-input"
              />
              <Label htmlFor="img-prepull" className="font-normal">
                Auto pre-pull on matching nodes
              </Label>
            </div>
          </div>

          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setDialogOpen(false)}
              className="rounded-xl"
            >
              Cancel
            </Button>
            <Button
              onClick={handleSave}
              disabled={!form.id || !form.image}
              className="rounded-xl"
            >
              {editingId ? "Update" : "Register"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
