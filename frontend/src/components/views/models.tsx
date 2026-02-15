import { useCallback, useEffect, useState } from "react"
import { Plus, Trash2, Box, Play, Square, Loader2, ExternalLink, Copy, Check } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Progress } from "@/components/ui/progress"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import type { AggregatedModelState, ModelView } from "@/lib/types"
import { v2 } from "@/lib/api"
import { cn } from "@/lib/utils"
import { useI18n } from "@/lib/i18n"

const STATE_BADGE: Record<AggregatedModelState, { key: string; cls: string }> = {
    running: { key: "state.running", cls: "bg-success/10 text-success border-success/20" },
    stopped: { key: "state.stopped", cls: "bg-muted text-muted-foreground border-border" },
    downloading: { key: "state.downloading", cls: "bg-blue-500/10 text-blue-600 border-blue-500/20 animate-pulse" },
    starting: { key: "state.starting", cls: "bg-yellow-500/10 text-yellow-600 border-yellow-500/20 animate-pulse" },
    degraded: { key: "state.degraded", cls: "bg-orange-500/10 text-orange-600 border-orange-500/20" },
    failed: { key: "state.failed", cls: "bg-destructive/10 text-destructive border-destructive/20" },
    stopping: { key: "state.stopping", cls: "bg-muted text-muted-foreground border-border animate-pulse" },
}

interface ModelsProps {
    token: string
    onOpenLoadDialog: () => void
    onNavigate?: (page: string) => void
    onSelectModel?: (uid: string) => void
}

export function ModelsView({
    token,
    onOpenLoadDialog,
    onNavigate: _onNavigate,
    onSelectModel,
}: ModelsProps) {
    const { t } = useI18n()
    const [models, setModels] = useState<ModelView[]>([])
    const [initialLoading, setInitialLoading] = useState(true)
    const [error, setError] = useState<string | null>(null)
    const [acting, setActing] = useState<string | null>(null)
    const [filter, setFilter] = useState<AggregatedModelState | "all">("all")
    const [copiedModelUid, setCopiedModelUid] = useState<string | null>(null)

    const refresh = useCallback(async () => {
        try {
            const data = await v2.listModels(token)
            setModels(data)
            setError(null)
        } catch (err) {
            setError(err instanceof Error ? err.message : t('models.failedLoad'))
        } finally {
            setInitialLoading(false)
        }
    }, [token, t])

    useEffect(() => { refresh() }, [refresh])
    useEffect(() => {
        const id = setInterval(refresh, 8000)
        return () => clearInterval(id)
    }, [refresh])

    const act = async (uid: string, fn: () => Promise<unknown>) => {
        setActing(uid)
        try { await fn(); await refresh() } catch (err) {
            setError(err instanceof Error ? err.message : t('models.actionFailed'))
        } finally { setActing(null) }
    }

    const copyModelName = async (uid: string, modelName: string) => {
        const markCopied = () => {
            setCopiedModelUid(uid)
            setError(null)
            setTimeout(() => setCopiedModelUid((current) => (current === uid ? null : current)), 1200)
        }

        try {
            if (navigator.clipboard && window.isSecureContext) {
                await navigator.clipboard.writeText(modelName)
                markCopied()
                return
            }
        } catch {
            // fallback below
        }

        try {
            const textArea = document.createElement("textarea")
            textArea.value = modelName
            textArea.setAttribute("readonly", "")
            textArea.style.position = "fixed"
            textArea.style.opacity = "0"
            textArea.style.pointerEvents = "none"
            document.body.appendChild(textArea)
            textArea.select()
            textArea.setSelectionRange(0, textArea.value.length)
            const copied = document.execCommand("copy")
            document.body.removeChild(textArea)

            if (!copied) throw new Error("execCommand copy failed")
            markCopied()
        } catch {
            setError(t('models.copyFail'))
        }
    }

    const filtered = filter === "all" ? models : models.filter((m) => m.state === filter)

    return (
        <div className="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-700">
            <div className="flex items-center justify-between">
                <div>
                    <h2 className="text-2xl font-bold text-foreground">{t('models.title')}</h2>
                    <p className="text-sm text-muted-foreground mt-1">{t('models.subtitle')}</p>
                </div>
                <Button
                    onClick={onOpenLoadDialog}
                    className="bg-primary hover:bg-primary/90 rounded-xl shadow-sm px-5"
                >
                    <Plus className="mr-2 h-4 w-4" />
                    {t('models.loadModel')}
                </Button>
            </div>

            {/* State filter */}
            <div className="flex items-center gap-2 flex-wrap">
                {(["all", "running", "stopped", "downloading", "starting", "degraded", "failed", "stopping"] as const).map((s) => {
                    const count = s === "all" ? models.length : models.filter((m) => m.state === s).length
                    if (s !== "all" && count === 0) return null
                    return (
                        <button
                            key={s}
                            onClick={() => setFilter(s)}
                            className={cn(
                                "px-3 py-1.5 rounded-xl text-xs font-bold transition-colors border",
                                filter === s
                                    ? "bg-primary text-primary-foreground border-primary"
                                    : "bg-transparent text-muted-foreground border-border hover:bg-accent"
                            )}
                        >
                            {s === "all" ? t('common.all') : t(`state.${s}`)} ({count})
                        </button>
                    )
                })}
            </div>

            {error && <p className="text-destructive text-sm bg-destructive/5 rounded-xl px-4 py-2">{error}</p>}

            <div className="bg-card border border-border rounded-2xl shadow-sm overflow-hidden">
                <div className="px-6 py-5 border-b border-border bg-accent/30 flex items-center justify-between">
                    <div>
                        <h3 className="text-lg font-bold text-foreground tracking-tight">{t('models.tableTitle')}</h3>
                        <p className="text-xs font-medium text-muted-foreground">{t('models.tableDesc')}</p>
                    </div>
                    <Badge variant="outline" className="font-bold border-primary/20 text-primary uppercase h-6">
                        {filtered.length} {t('common.total')}
                    </Badge>
                </div>

                <Table>
                    <TableHeader>
                        <TableRow className="bg-muted hover:bg-muted border-b border-border">
                            <TableHead className="text-[11px] font-bold text-muted-foreground uppercase py-4">{t('models.model')}</TableHead>
                            <TableHead className="text-[11px] font-bold text-muted-foreground uppercase py-4">{t('models.state')}</TableHead>
                            <TableHead className="text-[11px] font-bold text-muted-foreground uppercase py-4">{t('models.replicas')}</TableHead>
                            <TableHead className="text-[11px] font-bold text-muted-foreground uppercase py-4">{t('models.engine')}</TableHead>
                            <TableHead className="text-right text-[11px] font-bold text-muted-foreground uppercase py-4">{t('common.actions')}</TableHead>
                        </TableRow>
                    </TableHeader>
                    <TableBody>
                        {initialLoading ? (
                            <TableRow>
                                <TableCell colSpan={5} className="h-48 text-center">
                                    <Loader2 className="h-5 w-5 animate-spin mx-auto mb-2 text-muted-foreground" />
                                    <p className="text-sm text-muted-foreground">{t('models.loading')}</p>
                                </TableCell>
                            </TableRow>
                        ) : filtered.length === 0 ? (
                            <TableRow>
                                <TableCell colSpan={5} className="h-48 text-center">
                                    <div className="flex flex-col items-center justify-center opacity-40">
                                        <Box className="h-12 w-12 mb-2" />
                                        <p className="text-sm font-bold text-muted-foreground">{t('models.empty')}</p>
                                    </div>
                                </TableCell>
                            </TableRow>
                        ) : (
                            filtered.map((model) => {
                                const sb = STATE_BADGE[model.state]
                                const isActing = acting === model.model_uid
                                return (
                                    <TableRow
                                        key={model.model_uid}
                                        className="group hover:bg-accent/20 transition-colors"
                                    >
                                        <TableCell className="py-5">
                                            <div className="font-bold text-sm tracking-tight inline-flex items-center gap-1">
                                                {model.model_uid}
                                            </div>
                                            <div className="mt-1.5 flex items-center gap-1.5">
                                                <span className="text-[10px] font-mono text-muted-foreground truncate max-w-[220px] bg-accent/40 inline-block px-1.5 rounded select-text" title={model.model_name}>
                                                    {model.model_name}
                                                </span>
                                                <Button
                                                    type="button"
                                                    variant="ghost"
                                                    size="icon"
                                                    className="h-6 w-6 rounded-md text-muted-foreground hover:text-foreground"
                                                    onClick={() => copyModelName(model.model_uid, model.model_name)}
                                                    aria-label={`Copy model name ${model.model_name}`}
                                                >
                                                    {copiedModelUid === model.model_uid ? <Check className="h-3.5 w-3.5 text-success" /> : <Copy className="h-3.5 w-3.5" />}
                                                </Button>
                                            </div>
                                        </TableCell>
                                        <TableCell>
                                            <Badge className={cn("text-[11px] font-bold", sb.cls)}>{t(sb.key)}</Badge>
                                            {model.state === "downloading" && (
                                                <Progress value={30} className="h-1 mt-1.5 w-24" />
                                            )}
                                        </TableCell>
                                        <TableCell>
                                            <span className="text-sm font-bold">{model.replicas.ready}</span>
                                            <span className="text-xs text-muted-foreground"> / {model.replicas.desired}</span>
                                            {model.replicas.unhealthy > 0 && (
                                                <span className="text-[10px] text-destructive ml-1">({model.replicas.unhealthy} {t('models.unhealthy')})</span>
                                            )}
                                        </TableCell>
                                        <TableCell>
                                            <span className="text-xs font-medium">{model.engine_type ?? "default"}</span>
                                        </TableCell>
                                        <TableCell className="text-right" onClick={(e) => e.stopPropagation()}>
                                            <div className="flex items-center justify-end gap-1">
                                                {(model.state === "stopped" || model.state === "failed") && (
                                                    <Button
                                                        variant="ghost" size="sm"
                                                        className="text-success font-bold text-xs rounded-xl h-8"
                                                        onClick={() => act(model.model_uid, () => v2.startModel(model.model_uid, {}, token))}
                                                        disabled={isActing}
                                                    >
                                                        <Play className="h-3.5 w-3.5 mr-1" /> {t('models.start')}
                                                    </Button>
                                                )}
                                                {model.state === "running" && (
                                                    <Button
                                                        variant="outline" size="sm"
                                                        className="text-destructive border-destructive/30 hover:bg-destructive hover:text-destructive-foreground font-bold text-xs rounded-xl h-8"
                                                        onClick={() => act(model.model_uid, () => v2.stopModel(model.model_uid, token))}
                                                        disabled={isActing}
                                                    >
                                                        <Square className="h-3.5 w-3.5 mr-1" /> {t('models.stopService')}
                                                    </Button>
                                                )}
                                                <Button
                                                    variant="ghost" size="sm"
                                                    className="font-bold text-xs rounded-xl h-8"
                                                    onClick={() => onSelectModel?.(model.model_uid)}
                                                >
                                                    <ExternalLink className="h-3.5 w-3.5 mr-1" /> {t('common.details')}
                                                </Button>
                                                {(model.state === "stopped" || model.state === "failed") && (
                                                    <Button
                                                        variant="ghost" size="sm"
                                                        className="text-destructive font-bold text-xs hover:text-white hover:bg-destructive rounded-xl h-8"
                                                        onClick={() => act(model.model_uid, () => v2.deleteModel(model.model_uid, token))}
                                                        disabled={isActing}
                                                    >
                                                        <Trash2 className="h-3.5 w-3.5 mr-1" /> {t('models.delete')}
                                                    </Button>
                                                )}
                                                {isActing && <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />}
                                            </div>
                                        </TableCell>
                                    </TableRow>
                                )
                            })
                        )}
                    </TableBody>
                </Table>
            </div>
        </div>
    )
}
