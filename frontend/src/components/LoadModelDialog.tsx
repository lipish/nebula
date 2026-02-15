import { useCallback, useEffect, useRef, useState } from "react"
import { Search, ArrowLeft, ArrowRight, Download, Heart, Tag, Cpu, Server, Loader2, Sparkles, AlertTriangle, Check, LayoutTemplate } from "lucide-react"
import {
    Dialog,
    DialogContent,
    DialogHeader,
    DialogTitle,
    DialogDescription,
    DialogFooter,
} from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Badge } from "@/components/ui/badge"
import { cn } from "@/lib/utils"
import { apiGet, apiGetWithParams, v2 } from "@/lib/api"
import type { ClusterStatus, EngineImage, ModelLoadRequest, ModelSearchResult, ModelTemplate } from "@/lib/types"
import { useI18n } from "@/lib/i18n"

type Step = "search" | "configure" | "templates"
type Source = "huggingface" | "modelscope"

interface PopularModel {
    id: string
    description: string
    tags: string[]
}

const POPULAR_MODELS: Record<Source, PopularModel[]> = {
    huggingface: [
        { id: "Qwen/Qwen2.5-7B-Instruct", description: "Qwen 2.5 7B chat model", tags: ["7B", "chat"] },
        { id: "Qwen/Qwen2.5-14B-Instruct", description: "Qwen 2.5 14B chat model", tags: ["14B", "chat"] },
        { id: "Qwen/Qwen2.5-72B-Instruct", description: "Qwen 2.5 72B chat model", tags: ["72B", "chat"] },
        { id: "Qwen/Qwen3-8B", description: "Qwen 3 8B model", tags: ["8B", "new"] },
        { id: "Qwen/Qwen3-32B", description: "Qwen 3 32B model", tags: ["32B", "new"] },
        { id: "deepseek-ai/DeepSeek-R1-Distill-Qwen-7B", description: "DeepSeek R1 distilled 7B", tags: ["7B", "reasoning"] },
        { id: "deepseek-ai/DeepSeek-R1-Distill-Qwen-32B", description: "DeepSeek R1 distilled 32B", tags: ["32B", "reasoning"] },
        { id: "meta-llama/Llama-3.1-8B-Instruct", description: "Meta Llama 3.1 8B", tags: ["8B", "chat"] },
        { id: "meta-llama/Llama-3.1-70B-Instruct", description: "Meta Llama 3.1 70B", tags: ["70B", "chat"] },
        { id: "mistralai/Mistral-7B-Instruct-v0.3", description: "Mistral 7B v0.3", tags: ["7B", "chat"] },
    ],
    modelscope: [
        { id: "Qwen/Qwen2.5-7B-Instruct", description: "Qwen 2.5 7B 对话模型", tags: ["7B", "chat"] },
        { id: "Qwen/Qwen2.5-14B-Instruct", description: "Qwen 2.5 14B 对话模型", tags: ["14B", "chat"] },
        { id: "Qwen/Qwen2.5-72B-Instruct", description: "Qwen 2.5 72B 对话模型", tags: ["72B", "chat"] },
        { id: "Qwen/Qwen3-8B", description: "Qwen 3 8B 模型", tags: ["8B", "new"] },
        { id: "Qwen/Qwen3-32B", description: "Qwen 3 32B 模型", tags: ["32B", "new"] },
        { id: "deepseek-ai/DeepSeek-R1-Distill-Qwen-7B", description: "DeepSeek R1 蒸馏 7B", tags: ["7B", "reasoning"] },
        { id: "deepseek-ai/DeepSeek-R1-Distill-Qwen-32B", description: "DeepSeek R1 蒸馏 32B", tags: ["32B", "reasoning"] },
        { id: "LLM-Research/Meta-Llama-3.1-8B-Instruct", description: "Meta Llama 3.1 8B", tags: ["8B", "chat"] },
        { id: "AI-ModelScope/Mistral-7B-Instruct-v0.3", description: "Mistral 7B v0.3", tags: ["7B", "chat"] },
    ],
}

interface LoadModelDialogProps {
    open: boolean
    onOpenChange: (open: boolean) => void
    overview: ClusterStatus
    usedGpus: Map<string, Set<number>>
    pct: (used: number, total: number) => number
    token: string
    onSubmit: (form: ModelLoadRequest, gpu: { nodeId: string; gpuIndices: number[] }) => Promise<void>
    onUnloadRequestId: (requestId: string) => Promise<void>
}

function formatDownloads(n: number): string {
    if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
    if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
    return String(n)
}

function generateModelUid(modelId: string): string {
    const parts = modelId.split("/")
    const name = parts[parts.length - 1] || modelId
    return name.toLowerCase().replace(/[^a-z0-9-]/g, "-").replace(/-+/g, "-").replace(/^-|-$/g, "")
}

export function LoadModelDialog({
    open,
    onOpenChange,
    overview,
    usedGpus,
    pct,
    token,
    onSubmit,
    onUnloadRequestId,
}: LoadModelDialogProps) {
    const { t } = useI18n()
    const [step, setStep] = useState<Step>("search")
    const [source, setSource] = useState<Source>("huggingface")
    const [query, setQuery] = useState("")
    const [results, setResults] = useState<ModelSearchResult[]>([])
    const [searching, setSearching] = useState(false)
    const [searchError, setSearchError] = useState<string | null>(null)
    const [selectedModel, setSelectedModel] = useState<ModelSearchResult | null>(null)
    const [submitting, setSubmitting] = useState(false)

    const [form, setForm] = useState<ModelLoadRequest>({
        model_name: "",
        model_uid: "",
        replicas: 1,
        config: {},
        engine_type: "vllm",
    })
    const [selectedNode, setSelectedNode] = useState<string | null>(null)
    const [selectedGpuIndices, setSelectedGpuIndices] = useState<Set<number>>(new Set())
    const [engineImages, setEngineImages] = useState<EngineImage[]>([])
    const [templates, setTemplates] = useState<ModelTemplate[]>([])
    const [loadingTemplates, setLoadingTemplates] = useState(false)
    const [deployingTemplate, setDeployingTemplate] = useState<string | null>(null)

    const requestIdByModelUid = (() => {
        const m = new Map<string, string>()
        for (const r of (overview.model_requests ?? [])) {
            const rid = (r.id ?? "").toString()
            const uid = r.request?.model_uid
            if (!rid || !uid) continue
            const st = r.status
            if (typeof st === "string") {
                const s = st.toLowerCase()
                if (s.includes("fail") || s.includes("unload")) continue
            } else if (st && typeof st === "object") {
                if (Object.prototype.hasOwnProperty.call(st as object, "Failed")) continue
                if (Object.prototype.hasOwnProperty.call(st as object, "Unloaded")) continue
            }
            m.set(uid, rid)
        }
        return m
    })()

    const placementForGpu = (nodeId: string, gpuIndex: number) => {
        for (const p of overview.placements ?? []) {
            for (const a of p.assignments ?? []) {
                if (a.node_id !== nodeId) continue
                if (a.gpu_index != null && a.gpu_index === gpuIndex) return p
                const gis = (a.gpu_indices ?? []) as number[]
                if (Array.isArray(gis) && gis.includes(gpuIndex)) return p
            }
        }
        return null
    }

    const resolveRequestId = (modelUid: string, placementRequestId?: string | null) => {
        const rid = (placementRequestId ?? "").toString()
        if (rid.length > 0) return rid
        return requestIdByModelUid.get(modelUid) ?? null
    }

    const occupiedEntries = (() => {
        const fromPlacements = new Map<string, string>()
        for (const p of overview.placements ?? []) {
            const rid = (p.request_id ?? "").toString()
            if (rid.length > 0) fromPlacements.set(p.model_uid, rid)
        }
        if (fromPlacements.size > 0) return Array.from(fromPlacements.entries())

        const fromRequests = new Map<string, string>()
        for (const r of (overview.model_requests ?? [])) {
            const rid = (r.id ?? "").toString()
            if (!rid) continue
            const uid = r.request?.model_uid
            if (!uid) continue
            const st = r.status
            if (typeof st === "string") {
                const s = st.toLowerCase()
                if (s.includes("fail") || s.includes("unload")) continue
            } else if (st && typeof st === "object") {
                if (Object.prototype.hasOwnProperty.call(st as object, "Failed")) continue
                if (Object.prototype.hasOwnProperty.call(st as object, "Unloaded")) continue
            }
            fromRequests.set(uid, rid)
        }
        return Array.from(fromRequests.entries())
    })()

    const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null)

    const resetState = useCallback(() => {
        setStep("search")
        setQuery("")
        setResults([])
        setSearching(false)
        setSearchError(null)
        setSelectedModel(null)
        setSubmitting(false)
        setForm({ model_name: "", model_uid: "", replicas: 1, config: {}, engine_type: "vllm" })
        setSelectedNode(null)
        setSelectedGpuIndices(new Set())
    }, [])

    useEffect(() => {
        if (!open) resetState()
    }, [open, resetState])

    // Fetch registered engine images when dialog opens
    useEffect(() => {
        if (!open) return
        apiGet<EngineImage[]>("/admin/images", token)
            .then(setEngineImages)
            .catch(() => setEngineImages([]))
    }, [open, token])

    // Fetch templates when dialog opens
    useEffect(() => {
        if (!open) return
        setLoadingTemplates(true)
        v2.listTemplates(token)
            .then(setTemplates)
            .catch(() => setTemplates([]))
            .finally(() => setLoadingTemplates(false))
    }, [open, token])

    const imagesForEngine = engineImages.filter(img => img.engine_type === (form.engine_type ?? "vllm"))

    const doSearch = useCallback(async (q: string, src: Source) => {
        if (q.trim().length < 2) {
            setResults([])
            return
        }
        setSearching(true)
        setSearchError(null)
        try {
            const data = await apiGetWithParams<ModelSearchResult[]>(
                "/models/search",
                { q: q.trim(), source: src, limit: "20" },
                token
            )
            setResults(data)
        } catch (err) {
            setSearchError(err instanceof Error ? err.message : "Search failed")
            setResults([])
        } finally {
            setSearching(false)
        }
    }, [token])

    const handleQueryChange = useCallback((value: string) => {
        setQuery(value)
        if (debounceRef.current) clearTimeout(debounceRef.current)
        debounceRef.current = setTimeout(() => {
            doSearch(value, source)
        }, 400)
    }, [doSearch, source])

    const handleSourceChange = useCallback((src: Source) => {
        setSource(src)
        if (query.trim().length >= 2) {
            doSearch(query, src)
        }
    }, [doSearch, query])

    const handleSelectModel = useCallback((model: ModelSearchResult) => {
        setSelectedModel(model)
        setForm(prev => ({
            ...prev,
            model_name: model.id,
            model_uid: generateModelUid(model.id),
        }))
        setStep("configure")
    }, [])

    const handleEngineChange = useCallback((engineType: string) => {
        setForm(prev => ({ ...prev, engine_type: engineType, docker_image: null }))
    }, [])

    const handleBack = useCallback(() => {
        setStep("search")
    }, [])

    const toggleGpu = useCallback((nodeId: string, gpuIndex: number) => {
        setSelectedGpuIndices(prev => {
            // If switching node, reset selection
            if (selectedNode !== nodeId) {
                setSelectedNode(nodeId)
                return new Set([gpuIndex])
            }
            const next = new Set(prev)
            if (next.has(gpuIndex)) {
                next.delete(gpuIndex)
            } else {
                next.add(gpuIndex)
            }
            return next
        })
        if (selectedNode !== nodeId) {
            setSelectedNode(nodeId)
        }
    }, [selectedNode])

    const handleSubmit = useCallback(async () => {
        if (!selectedNode || selectedGpuIndices.size === 0 || !form.model_name) return
        const gpuIndices = Array.from(selectedGpuIndices).sort((a, b) => a - b)
        // Auto-set tensor_parallel_size if multi-GPU
        const finalForm = gpuIndices.length > 1
            ? { ...form, config: { ...form.config, tensor_parallel_size: gpuIndices.length } }
            : form
        setSubmitting(true)
        try {
            await onSubmit(finalForm, { nodeId: selectedNode, gpuIndices })
            onOpenChange(false)
        } catch {
            // error handled by parent
        } finally {
            setSubmitting(false)
        }
    }, [form, selectedNode, selectedGpuIndices, onSubmit, onOpenChange])

    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent className="max-w-2xl h-[70vh] flex flex-col overflow-hidden rounded-2xl">
                <DialogHeader>
                    <DialogTitle className="text-xl font-bold">
                        {step === "search" ? t('loadDialog.title.search') : step === "templates" ? t('loadDialog.title.templates') : t('loadDialog.title.configure')}
                    </DialogTitle>
                    <DialogDescription>
                        {step === "search"
                            ? t('loadDialog.desc.search')
                            : step === "templates"
                                ? t('loadDialog.desc.templates')
                                : t('loadDialog.desc.configure', { model: selectedModel?.id ?? form.model_name })}
                    </DialogDescription>
                </DialogHeader>

                {step === "search" && (
                    <div className="flex flex-col gap-4 flex-1 min-h-0">
                        {/* Source toggle + search */}
                        <div className="flex gap-2">
                            <div className="flex rounded-xl border border-border overflow-hidden">
                                <button
                                    onClick={() => handleSourceChange("huggingface")}
                                    className={cn(
                                        "px-3 py-1.5 text-xs font-bold transition-colors",
                                        source === "huggingface"
                                            ? "bg-primary text-primary-foreground"
                                            : "bg-transparent text-muted-foreground hover:bg-accent"
                                    )}
                                >
                                    🤗 HuggingFace
                                </button>
                                <button
                                    onClick={() => handleSourceChange("modelscope")}
                                    className={cn(
                                        "px-3 py-1.5 text-xs font-bold transition-colors",
                                        source === "modelscope"
                                            ? "bg-primary text-primary-foreground"
                                            : "bg-transparent text-muted-foreground hover:bg-accent"
                                    )}
                                >
                                    ModelScope
                                </button>
                            </div>
                            <div className="relative flex-1">
                                <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
                                <Input
                                    placeholder={t('loadDialog.searchPlaceholder')}
                                    className="pl-9 rounded-xl h-9"
                                    value={query}
                                    onChange={(e) => handleQueryChange(e.target.value)}
                                    autoFocus
                                />
                            </div>
                        </div>

                        {/* Results */}
                        <div className="flex-1 overflow-y-auto min-h-0 space-y-1.5 pr-1">
                            {searching && (
                                <div className="flex items-center justify-center py-12 text-muted-foreground">
                                    <Loader2 className="h-5 w-5 animate-spin mr-2" />
                                    <span className="text-sm">{t('loadDialog.searching')}</span>
                                </div>
                            )}
                            {searchError && (
                                <div className="text-center py-8 text-destructive text-sm">{searchError}</div>
                            )}
                            {!searching && !searchError && results.length === 0 && query.length >= 2 && (
                                <div className="text-center py-12 text-muted-foreground text-sm">
                                    {t('loadDialog.noModelsFound')}
                                </div>
                            )}
                            {!searching && !searchError && results.length === 0 && query.length < 2 && (
                                <div className="space-y-3">
                                    <div className="flex items-center gap-1.5 text-xs font-bold text-muted-foreground uppercase tracking-wider">
                                        <Sparkles className="h-3.5 w-3.5" />
                                        {t('loadDialog.popularModels')}
                                    </div>
                                    <div className="grid gap-1.5">
                                        {POPULAR_MODELS[source].map((model) => (
                                            <button
                                                key={model.id}
                                                onClick={() => handleSelectModel({
                                                    id: model.id,
                                                    name: model.id,
                                                    author: model.id.split("/")[0] || null,
                                                    downloads: 0,
                                                    likes: 0,
                                                    tags: model.tags,
                                                    pipeline_tag: "text-generation",
                                                    source,
                                                })}
                                                className="w-full text-left rounded-xl border border-border p-2.5 hover:border-primary/40 hover:bg-accent/30 transition-all group flex items-center justify-between"
                                            >
                                                <div className="flex-1 min-w-0">
                                                    <div className="flex items-center gap-2">
                                                        <p className="text-sm font-bold text-foreground truncate">{model.id}</p>
                                                        {model.tags.map((tag) => (
                                                            <Badge key={tag} variant="secondary" className="text-[9px] h-4 px-1.5 font-medium shrink-0">
                                                                {tag}
                                                            </Badge>
                                                        ))}
                                                    </div>
                                                    <p className="text-[11px] text-muted-foreground mt-0.5">{model.description}</p>
                                                </div>
                                                <ArrowRight className="h-4 w-4 text-muted-foreground opacity-0 group-hover:opacity-100 transition-opacity shrink-0 ml-2" />
                                            </button>
                                        ))}
                                    </div>
                                </div>
                            )}
                            {!searching && results.map((model) => (
                                <button
                                    key={`${model.source}-${model.id}`}
                                    onClick={() => handleSelectModel(model)}
                                    className="w-full text-left rounded-xl border border-border p-3 hover:border-primary/40 hover:bg-accent/30 transition-all group"
                                >
                                    <div className="flex items-start justify-between gap-2">
                                        <div className="flex-1 min-w-0">
                                            <p className="text-sm font-bold text-foreground truncate">{model.id}</p>
                                            {model.author && (
                                                <p className="text-[11px] text-muted-foreground mt-0.5">{model.author}</p>
                                            )}
                                        </div>
                                        <ArrowRight className="h-4 w-4 text-muted-foreground opacity-0 group-hover:opacity-100 transition-opacity mt-0.5 shrink-0" />
                                    </div>
                                    <div className="flex items-center gap-3 mt-2">
                                        <span className="flex items-center gap-1 text-[10px] text-muted-foreground">
                                            <Download className="h-3 w-3" />
                                            {formatDownloads(model.downloads)}
                                        </span>
                                        <span className="flex items-center gap-1 text-[10px] text-muted-foreground">
                                            <Heart className="h-3 w-3" />
                                            {formatDownloads(model.likes)}
                                        </span>
                                        {model.pipeline_tag && (
                                            <span className="flex items-center gap-1 text-[10px] text-muted-foreground">
                                                <Tag className="h-3 w-3" />
                                                {model.pipeline_tag}
                                            </span>
                                        )}
                                    </div>
                                    {model.tags.length > 0 && (
                                        <div className="flex flex-wrap gap-1 mt-2">
                                            {model.tags.slice(0, 5).map((tag) => (
                                                <Badge key={tag} variant="secondary" className="text-[9px] h-4 px-1.5 font-medium">
                                                    {tag}
                                                </Badge>
                                            ))}
                                            {model.tags.length > 5 && (
                                                <Badge variant="secondary" className="text-[9px] h-4 px-1.5 font-medium">
                                                    +{model.tags.length - 5}
                                                </Badge>
                                            )}
                                        </div>
                                    )}
                                </button>
                            ))}
                        </div>

                        {/* Manual entry + templates fallback */}
                        <div className="border-t border-border/50 pt-3 flex items-center justify-between">
                            <button
                                onClick={() => {
                                    setSelectedModel(null)
                                    setStep("configure")
                                }}
                                className="text-xs text-muted-foreground hover:text-foreground transition-colors"
                            >
                                {t('loadDialog.manualEntry')}
                            </button>
                            <button
                                onClick={() => setStep("templates")}
                                className="flex items-center gap-1.5 text-xs font-bold text-primary hover:text-primary/80 transition-colors"
                            >
                                <LayoutTemplate className="h-3.5 w-3.5" />
                                {t('loadDialog.fromTemplate')}
                            </button>
                        </div>
                    </div>
                )}

                {step === "templates" && (
                    <div className="flex flex-col gap-4 flex-1 min-h-0">
                        <button
                            onClick={() => setStep("search")}
                            className="flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors w-fit"
                        >
                            <ArrowLeft className="h-3.5 w-3.5" /> {t('loadDialog.backToSearch')}
                        </button>
                        <div className="flex-1 overflow-y-auto min-h-0 space-y-3 pr-1">
                            {loadingTemplates && (
                                <div className="flex items-center justify-center py-12 text-muted-foreground">
                                    <Loader2 className="h-5 w-5 animate-spin mr-2" />
                                    <span className="text-sm">{t('loadDialog.loadingTemplates')}</span>
                                </div>
                            )}
                            {!loadingTemplates && templates.length === 0 && (
                                <div className="text-center py-12 text-muted-foreground text-sm">
                                    {t('loadDialog.noTemplates')}
                                </div>
                            )}
                            {!loadingTemplates && (() => {
                                const grouped = new Map<string, ModelTemplate[]>()
                                for (const t of templates) {
                                    const cat = t.category ?? "other"
                                    if (!grouped.has(cat)) grouped.set(cat, [])
                                    grouped.get(cat)!.push(t)
                                }
                                return Array.from(grouped.entries()).map(([cat, items]) => (
                                    <div key={cat}>
                                        <h4 className="text-xs font-bold text-muted-foreground uppercase tracking-wider mb-2">
                                            {cat}
                                        </h4>
                                        <div className="space-y-1.5">
                                            {items.map((tmpl) => (
                                                <button
                                                    key={tmpl.template_id}
                                                    disabled={deployingTemplate !== null}
                                                    onClick={async () => {
                                                        setDeployingTemplate(tmpl.template_id)
                                                        try {
                                                            await v2.deployTemplate(tmpl.template_id, {}, token)
                                                            onOpenChange(false)
                                                        } catch (err) {
                                                            setSearchError(err instanceof Error ? err.message : t('loadDialog.deployFailed'))
                                                        } finally {
                                                            setDeployingTemplate(null)
                                                        }
                                                    }}
                                                    className="w-full text-left rounded-xl border border-border p-3 hover:border-primary/40 hover:bg-accent/30 transition-all group disabled:opacity-50"
                                                >
                                                    <div className="flex items-start justify-between gap-2">
                                                        <div className="flex-1 min-w-0">
                                                            <p className="text-sm font-bold text-foreground">{tmpl.name}</p>
                                                            <p className="text-[11px] text-muted-foreground mt-0.5 font-mono">{tmpl.model_name}</p>
                                                            {tmpl.description && (
                                                                <p className="text-[11px] text-muted-foreground mt-1">{tmpl.description}</p>
                                                            )}
                                                        </div>
                                                        <div className="flex items-center gap-2 shrink-0">
                                                            {tmpl.engine_type && (
                                                                <Badge variant="secondary" className="text-[9px]">{tmpl.engine_type}</Badge>
                                                            )}
                                                            {deployingTemplate === tmpl.template_id ? (
                                                                <Loader2 className="h-4 w-4 animate-spin text-primary" />
                                                            ) : (
                                                                <ArrowRight className="h-4 w-4 text-muted-foreground opacity-0 group-hover:opacity-100 transition-opacity" />
                                                            )}
                                                        </div>
                                                    </div>
                                                </button>
                                            ))}
                                        </div>
                                    </div>
                                ))
                            })()}
                        </div>
                        {searchError && (
                            <p className="text-destructive text-sm">{searchError}</p>
                        )}
                    </div>
                )}

                {step === "configure" && (
                    <div className="flex flex-col gap-6 flex-1 min-h-0 overflow-y-auto pr-1">
                        {/* Back button */}
                        <button
                            onClick={handleBack}
                            className="flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors w-fit"
                        >
                            <ArrowLeft className="h-3 w-3" />
                            {t('loadDialog.backToSearch')}
                        </button>

                        {/* GPU selection */}
                        <div className="space-y-3">
                            <div className="flex items-center justify-between">
                                <label className="text-xs font-bold text-muted-foreground uppercase tracking-wider">{t('loadDialog.targetHardware')}</label>
                                {selectedGpuIndices.size > 1 && (
                                    <Badge variant="secondary" className="text-[10px] font-bold">
                                        {t('loadDialog.gpuTensorParallel', { count: selectedGpuIndices.size })}
                                    </Badge>
                                )}
                            </div>
                            <div className="text-[10px] text-muted-foreground/70">
                                {t('loadDialog.vramHint')}
                            </div>

                            {(occupiedEntries.length > 0 || usedGpus.size > 0) && (
                                <div className="rounded-xl border border-border/60 bg-background p-3">
                                    <div className="flex items-center justify-between">
                                        <div className="text-[11px] font-bold text-muted-foreground/80">{t('loadDialog.occupiedModels')}</div>
                                        <div className="text-[10px] text-muted-foreground/70">{t('loadDialog.unloadToFreeGpus')}</div>
                                    </div>
                                    <div className="mt-2 space-y-2">
                                        {occupiedEntries.map(([modelUid, requestId]) => (
                                            <div key={modelUid} className="flex items-center justify-between gap-2">
                                                <div className="text-[11px] font-mono text-foreground truncate">{modelUid}</div>
                                                <Button
                                                    variant="secondary"
                                                    size="sm"
                                                    className="h-7 rounded-lg text-[10px] font-bold"
                                                    onClick={async () => {
                                                        await onUnloadRequestId(requestId)
                                                    }}
                                                    disabled={submitting}
                                                >
                                                    {t('loadDialog.unload')}
                                                </Button>
                                            </div>
                                        ))}
                                        {occupiedEntries.length === 0 && (
                                            <div className="text-[10px] text-muted-foreground/70">
                                                {t('loadDialog.noUnloadablePlacements')}
                                            </div>
                                        )}
                                    </div>
                                </div>
                            )}

                            <div className="grid gap-4 md:grid-cols-2">
                                {overview.nodes.map((node) => {
                                    const isThisNode = selectedNode === node.node_id
                                    return (
                                        <div key={node.node_id} className="space-y-2">
                                            <p className="text-[11px] font-bold text-muted-foreground/80 flex items-center gap-1.5">
                                                <Server className="h-3 w-3" />
                                                {node.node_id.toUpperCase()}
                                            </p>
                                            <div className="grid gap-2 p-0.5">
                                                {node.gpus.map((gpu) => {
                                                    const isUsed = usedGpus.get(node.node_id)?.has(gpu.index) ?? false
                                                    const isSel = isThisNode && selectedGpuIndices.has(gpu.index)
                                                    const usage = pct(gpu.memory_used_mb, gpu.memory_total_mb)
                                                    const freeMb = gpu.memory_total_mb - gpu.memory_used_mb
                                                    const freeGb = (freeMb / 1024).toFixed(1)
                                                    const totalGb = (gpu.memory_total_mb / 1024).toFixed(1)
                                                    const lowVram = freeMb < 4096
                                                    const usedPlacement = isUsed ? placementForGpu(node.node_id, gpu.index) : null
                                                    const usedModelUid = usedPlacement?.model_uid ?? null
                                                    const unloadRequestId = usedModelUid ? resolveRequestId(usedModelUid, usedPlacement?.request_id ?? null) : null
                                                    return (
                                                        <div key={gpu.index} className="flex items-center gap-2">
                                                            <button
                                                                onClick={() => {
                                                                    if (isUsed && !isSel) return
                                                                    toggleGpu(node.node_id, gpu.index)
                                                                }}
                                                                className={cn(
                                                                    "flex-1 flex items-center justify-between rounded-xl border p-3 text-left transition-all shadow-sm",
                                                                    isUsed && !isSel && "opacity-60 cursor-not-allowed",
                                                                    isSel
                                                                        ? "border-primary bg-primary/[0.03] ring-1 ring-primary"
                                                                        : "border-border hover:border-primary/40 hover:bg-accent/30"
                                                                )}
                                                            >
                                                                <div className="flex-1">
                                                                    <div className="flex items-center justify-between mb-1.5">
                                                                        <div className="flex items-center gap-1.5">
                                                                            <Cpu className={cn("h-3.5 w-3.5", isSel ? "text-primary" : "text-muted-foreground")} />
                                                                            <span className="text-xs font-bold">GPU {gpu.index}</span>
                                                                        </div>
                                                                        <div className="flex items-center gap-2">
                                                                            <span className={cn("text-[10px] font-bold", lowVram ? "text-destructive" : "text-muted-foreground")}>
                                                                                {t('loadDialog.freeTotal', { free: freeGb, total: totalGb })}
                                                                            </span>
                                                                            <span className="text-[10px] font-bold text-muted-foreground">{t('loadDialog.usedPct', { value: usage })}</span>
                                                                        </div>
                                                                    </div>
                                                                    <div className="h-1 w-full bg-accent rounded-full overflow-hidden">
                                                                        <div
                                                                            className={cn("h-full rounded-full transition-all",
                                                                                usage > 80 ? "bg-destructive" : (isSel ? "bg-primary" : "bg-primary/50")
                                                                            )}
                                                                            style={{ width: `${usage}%` }}
                                                                        />
                                                                    </div>
                                                                    {lowVram && isSel && (
                                                                        <div className="flex items-center gap-1 mt-1.5 text-[10px] text-amber-500">
                                                                            <AlertTriangle className="h-3 w-3" />
                                                                            {t('loadDialog.lowVram')}
                                                                        </div>
                                                                    )}
                                                                </div>
                                                                {isUsed && !isSel && (
                                                                    <Badge className="ml-2 text-[9px] font-bold bg-secondary text-secondary-foreground border-0">{t('loadDialog.inUse')}</Badge>
                                                                )}
                                                                {isSel && (
                                                                    <div className="ml-2 h-4 w-4 rounded-full bg-primary flex items-center justify-center">
                                                                        <Check className="h-2.5 w-2.5 text-primary-foreground" />
                                                                    </div>
                                                                )}
                                                            </button>
                                                            {isUsed && unloadRequestId && !isSel && (
                                                                <Button
                                                                    variant="secondary"
                                                                    size="sm"
                                                                    className="h-7 rounded-lg text-[10px] font-bold shrink-0"
                                                                    onClick={async () => {
                                                                        await onUnloadRequestId(unloadRequestId)
                                                                    }}
                                                                    disabled={submitting}
                                                                >
                                                                    {t('loadDialog.unload')}
                                                                </Button>
                                                            )}
                                                        </div>
                                                    )
                                                })}
                                            </div>
                                        </div>
                                    )
                                })}
                            </div>
                            {selectedGpuIndices.size > 1 && (
                                <p className="text-[11px] text-muted-foreground">
                                    {t('loadDialog.multiGpuAutoTensor', { count: selectedGpuIndices.size })}
                                </p>
                            )}
                        </div>

                        {/* Engine selection */}
                        <div className="space-y-3">
                            <label className="text-xs font-bold text-muted-foreground uppercase tracking-wider">{t('models.engine')}</label>
                            <div className="flex gap-2">
                                <div className="flex rounded-xl border border-border overflow-hidden">
                                    {["vllm", "sglang"].map((eng) => (
                                        <button
                                            key={eng}
                                            onClick={() => handleEngineChange(eng)}
                                            className={cn(
                                                "px-4 py-1.5 text-xs font-bold transition-colors",
                                                form.engine_type === eng
                                                    ? "bg-primary text-primary-foreground"
                                                    : "bg-transparent text-muted-foreground hover:bg-accent"
                                            )}
                                        >
                                            {eng === "vllm" ? "vLLM" : "SGLang"}
                                        </button>
                                    ))}
                                </div>
                                {imagesForEngine.length > 0 && (
                                    <select
                                        className="flex-1 rounded-xl border border-border bg-background px-3 py-1.5 text-xs font-mono focus:outline-none focus:ring-1 focus:ring-primary"
                                        value={form.docker_image ?? ""}
                                        onChange={(e) => setForm(prev => ({ ...prev, docker_image: e.target.value || null }))}
                                    >
                                        <option value="">{t('loadDialog.defaultNodeCli')}</option>
                                        {imagesForEngine.map((img) => (
                                            <option key={img.id} value={img.image}>
                                                {img.image}{img.description ? ` — ${img.description}` : ""}
                                            </option>
                                        ))}
                                    </select>
                                )}
                            </div>
                            {imagesForEngine.length === 0 && (
                                <p className="text-[10px] text-muted-foreground/70">
                                    {t('loadDialog.noRegisteredImages', { engine: form.engine_type === 'vllm' ? 'vLLM' : 'SGLang' })}
                                </p>
                            )}
                        </div>

                        {/* Model config fields */}
                        <div className="grid gap-4 sm:grid-cols-2">
                            <div className="space-y-1.5">
                                <label className="text-xs font-bold text-muted-foreground uppercase">{t('loadDialog.modelPath')}</label>
                                <Input
                                    placeholder={t('loadDialog.modelPathPlaceholder')}
                                    className="rounded-xl h-10"
                                    value={form.model_name}
                                    onChange={(e) => setForm({ ...form, model_name: e.target.value })}
                                />
                                <div className="text-[10px] text-muted-foreground/70">
                                    {t('loadDialog.ggufHint')}
                                </div>
                            </div>
                            <div className="space-y-1.5">
                                <label className="text-xs font-bold text-muted-foreground uppercase">{t('loadDialog.deploymentUid')}</label>
                                <Input
                                    placeholder={t('loadDialog.deploymentUidPlaceholder')}
                                    className="rounded-xl h-10 font-mono"
                                    value={form.model_uid}
                                    onChange={(e) => setForm({ ...form, model_uid: e.target.value })}
                                />
                            </div>
                            <div className="space-y-1.5">
                                <label className="text-xs font-bold text-muted-foreground uppercase">{t('models.replicas')}</label>
                                <Input
                                    type="number"
                                    className="rounded-xl h-10"
                                    value={form.replicas}
                                    onChange={(e) => setForm({ ...form, replicas: Number(e.target.value) })}
                                />
                            </div>
                            <div className="space-y-1.5">
                                <label className="text-xs font-bold text-muted-foreground uppercase">{t('loadDialog.contextWindow')}</label>
                                <Input
                                    type="number"
                                    placeholder="4096"
                                    className="rounded-xl h-10 font-mono"
                                    value={form.config?.max_model_len ?? ""}
                                    onChange={(e) => {
                                        const raw = e.target.value
                                        const next = raw === "" ? undefined : Number(raw)
                                        setForm({
                                            ...form,
                                            config: { ...form.config, max_model_len: next },
                                        })
                                    }}
                                />
                            </div>
                        </div>
                    </div>
                )}

                {step === "configure" && (
                    <DialogFooter className="border-t border-border/50 pt-4">
                        <Button variant="outline" className="rounded-xl" onClick={() => onOpenChange(false)}>
                            {t('common.cancel')}
                        </Button>
                        <Button
                            className="bg-primary font-bold rounded-xl px-6"
                            onClick={handleSubmit}
                            disabled={!selectedNode || selectedGpuIndices.size === 0 || !form.model_name || submitting}
                        >
                            {submitting ? (
                                <>
                                    <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                                    {t('loadDialog.deploying')}
                                </>
                            ) : (
                                t('loadDialog.launchModel')
                            )}
                        </Button>
                    </DialogFooter>
                )}
            </DialogContent>
        </Dialog>
    )
}
