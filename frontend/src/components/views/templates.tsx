import { useCallback, useEffect, useState } from "react"
import { Layers, Rocket, RefreshCw } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from "@/components/ui/dialog"
import { v2 } from "@/lib/api"
import type { ModelTemplate } from "@/lib/types"

interface TemplatesViewProps {
  token: string
}

const EMPTY_DEPLOY_FORM = {
  model_uid: "",
  replicas: "",
  node_id: "",
  gpu_indices: "",
}

export function TemplatesView({ token }: TemplatesViewProps) {
  const [templates, setTemplates] = useState<ModelTemplate[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [deployDialogOpen, setDeployDialogOpen] = useState(false)
  const [selectedTemplate, setSelectedTemplate] = useState<ModelTemplate | null>(null)
  const [deployForm, setDeployForm] = useState(EMPTY_DEPLOY_FORM)

  const refresh = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const data = await v2.listTemplates(token)
      setTemplates(data)
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load templates")
    } finally {
      setLoading(false)
    }
  }, [token])

  useEffect(() => {
    refresh()
  }, [refresh])

  useEffect(() => {
    const id = setInterval(refresh, 30000)
    return () => clearInterval(id)
  }, [refresh])

  const openDeploy = (tpl: ModelTemplate) => {
    setSelectedTemplate(tpl)
    setDeployForm({ ...EMPTY_DEPLOY_FORM })
    setDeployDialogOpen(true)
  }

  const handleDeploy = async () => {
    if (!selectedTemplate) return
    setError(null)
    try {
      const body: Record<string, unknown> = {}
      if (deployForm.model_uid) body.model_uid = deployForm.model_uid
      if (deployForm.replicas) body.replicas = parseInt(deployForm.replicas, 10)
      if (deployForm.node_id) body.node_id = deployForm.node_id
      if (deployForm.gpu_indices) {
        body.gpu_indices = deployForm.gpu_indices
          .split(",")
          .map((s) => parseInt(s.trim(), 10))
          .filter((n) => !isNaN(n))
      }
      await v2.deployTemplate(selectedTemplate.template_id, body, token)
      setDeployDialogOpen(false)
      await refresh()
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to deploy template")
    }
  }

  return (
    <div className="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-700">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold text-foreground">Model Templates</h2>
          <p className="text-sm text-muted-foreground mt-1">
            Pre-configured model templates for quick deployment
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
        </div>
      </div>

      {error && (
        <div className="bg-destructive/10 border border-destructive/20 rounded-xl px-4 py-3 text-sm text-destructive">
          {error}
        </div>
      )}

      {/* Template List */}
      <div className="bg-card border border-border rounded-2xl shadow-sm overflow-hidden">
        <div className="px-6 py-5 border-b border-border bg-accent/30 flex items-center justify-between">
          <div>
            <h3 className="text-lg font-bold text-foreground tracking-tight">
              Available Templates
            </h3>
          </div>
          <Badge
            variant="outline"
            className="font-bold border-primary/20 text-primary uppercase h-6"
          >
            {templates.length} Total
          </Badge>
        </div>

        <Table>
          <TableHeader>
            <TableRow className="bg-muted hover:bg-muted border-b border-border">
              <TableHead className="text-[11px] font-bold text-muted-foreground uppercase pl-6 pr-4 py-4">
                Name
              </TableHead>
              <TableHead className="text-[11px] font-bold text-muted-foreground uppercase px-4 py-4">
                Model
              </TableHead>
              <TableHead className="text-[11px] font-bold text-muted-foreground uppercase px-4 py-4">
                Engine
              </TableHead>
              <TableHead className="text-[11px] font-bold text-muted-foreground uppercase px-4 py-4">
                Category
              </TableHead>
              <TableHead className="text-[11px] font-bold text-muted-foreground uppercase px-4 py-4">
                Source
              </TableHead>
              <TableHead className="text-[11px] font-bold text-muted-foreground uppercase px-4 py-4">
                Replicas
              </TableHead>
              <TableHead className="text-right text-[11px] font-bold text-muted-foreground uppercase pl-4 pr-6 py-4">
                Actions
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {templates.length === 0 ? (
              <TableRow>
                <TableCell colSpan={7} className="h-48 text-center">
                  <div className="flex flex-col items-center justify-center opacity-40">
                    <Layers className="h-12 w-12 mb-2" />
                    <p className="text-sm font-bold text-muted-foreground">
                      No templates available yet.
                    </p>
                  </div>
                </TableCell>
              </TableRow>
            ) : (
              templates.map((tpl) => (
                <TableRow key={tpl.template_id} className="group">
                  <TableCell className="pl-6 py-4">
                    <div className="font-bold text-sm tracking-tight">
                      {tpl.name}
                    </div>
                    {tpl.description && (
                      <div className="text-[10px] text-muted-foreground mt-0.5 max-w-[180px] truncate">
                        {tpl.description}
                      </div>
                    )}
                  </TableCell>
                  <TableCell>
                    <div className="font-mono text-xs bg-accent/40 inline-block px-2 py-0.5 rounded max-w-[280px] truncate">
                      {tpl.model_name}
                    </div>
                  </TableCell>
                  <TableCell>
                    {tpl.engine_type ? (
                      <Badge variant="outline" className="font-bold text-[11px] uppercase">
                        {tpl.engine_type}
                      </Badge>
                    ) : (
                      <span className="text-xs text-muted-foreground">—</span>
                    )}
                  </TableCell>
                  <TableCell>
                    {tpl.category ? (
                      <Badge variant="secondary" className="font-bold text-[10px] uppercase">
                        {tpl.category}
                      </Badge>
                    ) : (
                      <span className="text-xs text-muted-foreground">—</span>
                    )}
                  </TableCell>
                  <TableCell>
                    <Badge
                      variant={tpl.source === "system" ? "default" : "secondary"}
                      className="text-[10px] font-bold uppercase"
                    >
                      {tpl.source}
                    </Badge>
                  </TableCell>
                  <TableCell>
                    <span className="text-sm font-bold">{tpl.default_replicas}</span>
                  </TableCell>
                  <TableCell className="text-right pr-6">
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => openDeploy(tpl)}
                      className="text-xs font-bold rounded-xl h-8"
                    >
                      <Rocket className="h-3.5 w-3.5 mr-1" />
                      Deploy
                    </Button>
                  </TableCell>
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </div>

      {/* Deploy Dialog */}
      <Dialog open={deployDialogOpen} onOpenChange={setDeployDialogOpen}>
        <DialogContent className="sm:max-w-[480px]">
          <DialogHeader>
            <DialogTitle>
              Deploy Template: {selectedTemplate?.name}
            </DialogTitle>
          </DialogHeader>

          <div className="space-y-4 py-2">
            <div className="space-y-2">
              <Label htmlFor="deploy-uid">
                Model UID{" "}
                <span className="text-muted-foreground font-normal">(optional override)</span>
              </Label>
              <Input
                id="deploy-uid"
                placeholder="Leave empty to auto-generate"
                value={deployForm.model_uid}
                onChange={(e) => setDeployForm({ ...deployForm, model_uid: e.target.value })}
              />
            </div>

            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-2">
                <Label htmlFor="deploy-replicas">Replicas</Label>
                <Input
                  id="deploy-replicas"
                  type="number"
                  placeholder={String(selectedTemplate?.default_replicas ?? 1)}
                  value={deployForm.replicas}
                  onChange={(e) => setDeployForm({ ...deployForm, replicas: e.target.value })}
                />
              </div>

              <div className="space-y-2">
                <Label htmlFor="deploy-node">Node ID</Label>
                <Input
                  id="deploy-node"
                  placeholder="Optional"
                  value={deployForm.node_id}
                  onChange={(e) => setDeployForm({ ...deployForm, node_id: e.target.value })}
                />
              </div>
            </div>

            <div className="space-y-2">
              <Label htmlFor="deploy-gpus">
                GPU Indices{" "}
                <span className="text-muted-foreground font-normal">(comma-separated)</span>
              </Label>
              <Input
                id="deploy-gpus"
                placeholder="e.g. 0, 1"
                value={deployForm.gpu_indices}
                onChange={(e) => setDeployForm({ ...deployForm, gpu_indices: e.target.value })}
              />
            </div>
          </div>

          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setDeployDialogOpen(false)}
              className="rounded-xl"
            >
              Cancel
            </Button>
            <Button
              onClick={handleDeploy}
              className="rounded-xl"
            >
              <Rocket className="h-4 w-4 mr-2" />
              Deploy
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}

