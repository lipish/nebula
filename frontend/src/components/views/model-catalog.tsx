import { useCallback, useEffect, useMemo, useState } from 'react'
import { Download, ExternalLink, RefreshCw, Search } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { apiGetWithParams, v2 } from '@/lib/api'
import type { ModelView } from '@/lib/types'
import { useI18n } from '@/lib/i18n'

interface ModelCatalogViewProps {
  token: string
  onSelectModel: (uid: string) => void
  onOpenModels: () => void
}

interface ModelSearchResult {
  id: string
  name: string
  author?: string | null
  downloads: number
  likes: number
  tags: string[]
  pipeline_tag?: string | null
  source: 'huggingface' | 'modelscope'
}

interface CreatedModelResponse {
  model_uid: string
  model_name: string
}

type SourceOption = (typeof SOURCE_OPTIONS)[number]['value']

const SOURCE_OPTIONS = [
  { value: 'hugging_face', label: 'Hugging Face' },
  { value: 'model_scope', label: 'ModelScope' },
] as const

export function ModelCatalogView({ token, onSelectModel, onOpenModels }: ModelCatalogViewProps) {
  const { t } = useI18n()
  const [models, setModels] = useState<ModelView[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [importing, setImporting] = useState(false)
  const [searching, setSearching] = useState(false)
  const [source, setSource] = useState<SourceOption>('hugging_face')
  const [taskFilter, setTaskFilter] = useState<string>('all')
  const [searchQuery, setSearchQuery] = useState('')
  const [searchResults, setSearchResults] = useState<ModelSearchResult[]>([])
  const [downloadDialogOpen, setDownloadDialogOpen] = useState(false)
  const [selectedResult, setSelectedResult] = useState<ModelSearchResult | null>(null)
  const [downloadModelUid, setDownloadModelUid] = useState('')
  const [downloadPath, setDownloadPath] = useState('')
  const [activeDownloadUid, setActiveDownloadUid] = useState<string | null>(null)

  const compactNumber = useMemo(
    () => new Intl.NumberFormat('en', { notation: 'compact', maximumFractionDigits: 1 }),
    []
  )

  const refresh = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const list = await v2.listModels(token)
      setModels(list)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load model catalog')
    } finally {
      setLoading(false)
    }
  }, [token])

  const runSearch = useCallback(async (query: string, src: SourceOption) => {
    setSearching(true)
    setError(null)
    try {
      const results = await apiGetWithParams<ModelSearchResult[]>(
        '/models/search',
        {
          q: query.trim(),
          source: src === 'hugging_face' ? 'huggingface' : 'modelscope',
          limit: '20',
        },
        token
      )
      setSearchResults(results)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to search models')
    } finally {
      setSearching(false)
    }
  }, [token])

  useEffect(() => {
    refresh()
  }, [refresh])

  useEffect(() => {
    const id = setTimeout(() => {
      runSearch(searchQuery, source)
    }, 250)
    return () => clearTimeout(id)
  }, [searchQuery, source, runSearch])

  useEffect(() => {
    if (!activeDownloadUid) return
    const id = setInterval(refresh, 3000)
    return () => clearInterval(id)
  }, [activeDownloadUid, refresh])

  const downloadedByName = useMemo(() => {
    const index = new Map<string, ModelView[]>()
    for (const model of models) {
      const list = index.get(model.model_name) || []
      list.push(model)
      index.set(model.model_name, list)
    }
    return index
  }, [models])

  const activeDownloadModel = useMemo(
    () => (activeDownloadUid ? models.find((m) => m.model_uid === activeDownloadUid) || null : null),
    [activeDownloadUid, models]
  )

  const taskLabel = (pipelineTag?: string | null) => {
    if (!pipelineTag) return 'Other'
    return pipelineTag
      .split('-')
      .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
      .join(' ')
  }

  const taskOptions = useMemo(() => {
    const set = new Set<string>()
    for (const result of searchResults) {
      set.add(taskLabel(result.pipeline_tag))
    }
    return ['all', ...Array.from(set).sort()]
  }, [searchResults])

  const visibleResults = useMemo(() => {
    if (taskFilter === 'all') return searchResults
    return searchResults.filter((result) => taskLabel(result.pipeline_tag) === taskFilter)
  }, [searchResults, taskFilter])

  const modelSourceUrl = (result: ModelSearchResult) =>
    result.source === 'huggingface'
      ? `https://huggingface.co/${result.id}`
      : `https://modelscope.cn/models/${result.id}`

  const openDownloadDialog = (result: ModelSearchResult) => {
    setSelectedResult(result)
    setDownloadDialogOpen(true)
  }

  const handleDownloadConfirm = async () => {
    if (!selectedResult) return

    setImporting(true)
    setError(null)
    try {
      const created = (await v2.createModel(
        {
          model_name: selectedResult.id,
          model_uid: downloadModelUid.trim() || undefined,
          model_path: downloadPath.trim() || undefined,
          model_source: selectedResult.source === 'huggingface' ? 'hugging_face' : 'model_scope',
          auto_start: true,
          replicas: 1,
        },
        token
      )) as CreatedModelResponse

      setActiveDownloadUid(created.model_uid)
      setDownloadDialogOpen(false)
      setDownloadModelUid('')
      setDownloadPath('')
      setSelectedResult(null)
      await refresh()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to download model')
    } finally {
      setImporting(false)
    }
  }

  return (
    <div className="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-700">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold text-foreground">{t('catalog.title')}</h2>
          <p className="text-sm text-muted-foreground mt-1">
            {t('catalog.subtitle')}
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm" onClick={onOpenModels} className="rounded-xl">
            {t('catalog.downloadedModels')}
          </Button>
          <Button variant="outline" size="sm" onClick={refresh} disabled={loading} className="rounded-xl">
            <RefreshCw className={`h-4 w-4 mr-1.5 ${loading ? 'animate-spin' : ''}`} />
            {t('common.refresh')}
          </Button>
        </div>
      </div>

      {error && (
        <div className="bg-destructive/10 border border-destructive/20 rounded-xl px-4 py-3 text-sm text-destructive">
          {error}
        </div>
      )}

      {activeDownloadUid && (
        <div className="bg-primary/10 border border-primary/20 rounded-xl px-4 py-3 flex items-center justify-between gap-3">
          <div>
            <p className="text-sm font-semibold text-foreground">{t('catalog.downloadTask')}</p>
            <p className="text-xs text-muted-foreground">
              {activeDownloadUid} · {activeDownloadModel ? activeDownloadModel.state : t('catalog.submitted')}
            </p>
          </div>
          <div className="flex items-center gap-2">
            {activeDownloadModel && (
              <Button
                size="sm"
                variant="outline"
                className="rounded-xl"
                onClick={() => onSelectModel(activeDownloadUid)}
              >
                {t('catalog.openStatus')}
              </Button>
            )}
            <Button
              size="sm"
              variant="ghost"
              className="rounded-xl"
              onClick={() => setActiveDownloadUid(null)}
            >
              {t('catalog.dismiss')}
            </Button>
          </div>
        </div>
      )}

      <div className="bg-card border border-border rounded-2xl shadow-sm overflow-hidden">
        <div className="px-6 py-5 border-b border-border bg-accent/30">
          <h3 className="text-lg font-bold text-foreground tracking-tight">{t('catalog.browse')}</h3>
        </div>
        <div className="p-6 grid grid-cols-1 md:grid-cols-12 gap-3">
          <div className="space-y-2">
            <Label htmlFor="catalog-source">{t('catalog.source')}</Label>
            <select
              id="catalog-source"
              value={source}
              onChange={(e) => setSource(e.target.value as SourceOption)}
              className="h-10 w-full rounded-md border border-input bg-background px-3 text-sm"
            >
              {SOURCE_OPTIONS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </div>
          <div className="space-y-2 md:col-span-8">
            <Label htmlFor="catalog-search">{t('catalog.searchModel')}</Label>
            <div className="relative">
              <Search className="h-4 w-4 text-muted-foreground absolute left-3 top-1/2 -translate-y-1/2" />
              <Input
                id="catalog-search"
                placeholder={t('catalog.searchPlaceholder')}
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="pl-9"
              />
            </div>
          </div>
          <div className="space-y-2 md:col-span-3">
            <Label htmlFor="catalog-task-filter">{t('catalog.task')}</Label>
            <select
              id="catalog-task-filter"
              value={taskFilter}
              onChange={(e) => setTaskFilter(e.target.value)}
              className="h-10 w-full rounded-md border border-input bg-background px-3 text-sm"
            >
              <option value="all">{t('catalog.allTasks')}</option>
              {taskOptions
                .filter((option) => option !== 'all')
                .map((option) => (
                  <option key={option} value={option}>{option}</option>
                ))}
            </select>
          </div>
        </div>

        <div className="border-t border-border">
          <Table>
            <TableHeader>
              <TableRow className="bg-muted hover:bg-muted border-b border-border">
                <TableHead className="text-[11px] font-bold text-muted-foreground uppercase pl-6 pr-4 py-4">{t('models.model')}</TableHead>
                <TableHead className="text-[11px] font-bold text-muted-foreground uppercase px-4 py-4">{t('catalog.task')}</TableHead>
                <TableHead className="text-[11px] font-bold text-muted-foreground uppercase px-4 py-4">{t('catalog.downloads')}</TableHead>
                <TableHead className="text-[11px] font-bold text-muted-foreground uppercase px-4 py-4">{t('catalog.likes')}</TableHead>
                <TableHead className="text-right text-[11px] font-bold text-muted-foreground uppercase pl-4 pr-6 py-4">{t('common.actions')}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {visibleResults.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={5} className="h-28 text-center text-sm text-muted-foreground">
                    {searching ? t('catalog.searching') : t('catalog.empty')}
                  </TableCell>
                </TableRow>
              ) : (
                visibleResults.map((result) => {
                  const downloaded = downloadedByName.get(result.id) || []
                  const firstDownloaded = downloaded[0]

                  return (
                    <TableRow key={`${result.source}-${result.id}`}>
                      <TableCell className="pl-6 py-4">
                        <div className="flex items-center gap-2">
                          <div className="font-bold text-sm tracking-tight">{result.id}</div>
                          {firstDownloaded && (
                            <Badge variant="secondary" className="text-[10px] font-bold uppercase">
                              {t('catalog.downloaded')}
                            </Badge>
                          )}
                        </div>
                        {result.author && (
                          <div className="text-[10px] text-muted-foreground mt-0.5">by {result.author}</div>
                        )}
                      </TableCell>
                      <TableCell>
                        <Badge variant="outline" className="text-[10px] font-medium">
                          {taskLabel(result.pipeline_tag)}
                        </Badge>
                      </TableCell>
                      <TableCell>
                        <span className="text-sm">{compactNumber.format(result.downloads)}</span>
                      </TableCell>
                      <TableCell>
                        <span className="text-sm">{compactNumber.format(result.likes)}</span>
                      </TableCell>
                      <TableCell className="text-right pr-6">
                        <div className="inline-flex items-center gap-2">
                          <Button
                            size="icon"
                            variant="ghost"
                            className="h-8 w-8"
                            onClick={() => window.open(modelSourceUrl(result), '_blank', 'noopener,noreferrer')}
                          >
                            <ExternalLink className="h-4 w-4" />
                          </Button>
                          {firstDownloaded ? (
                            <Button
                              size="sm"
                              variant="secondary"
                              className="rounded-xl h-8"
                              onClick={() => onSelectModel(firstDownloaded.model_uid)}
                            >
                              {t('catalog.imported')}
                            </Button>
                          ) : (
                            <Button
                              size="sm"
                              className="rounded-xl h-8"
                              disabled={importing}
                              onClick={() => openDownloadDialog(result)}
                            >
                              <Download className="mr-1.5 h-3.5 w-3.5" />
                              {t('catalog.import')}
                            </Button>
                          )}
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

      <Dialog
        open={downloadDialogOpen}
        onOpenChange={(open) => {
          setDownloadDialogOpen(open)
          if (!open) {
            setSelectedResult(null)
            setDownloadModelUid('')
            setDownloadPath('')
          }
        }}
      >
        <DialogContent className="sm:max-w-[520px]">
          <DialogHeader>
            <DialogTitle>{t('catalog.downloadModel')}</DialogTitle>
          </DialogHeader>

          <div className="space-y-4 py-2">
            <div className="space-y-1">
              <p className="text-sm font-medium text-foreground">{t('models.model')}</p>
              <p className="text-sm text-muted-foreground break-all">
                {selectedResult?.id || '—'}
              </p>
            </div>

            <div className="space-y-2">
              <Label htmlFor="download-model-uid">{t('catalog.modelUidOptional')}</Label>
              <Input
                id="download-model-uid"
                placeholder={t('catalog.autoGenerated')}
                value={downloadModelUid}
                onChange={(e) => setDownloadModelUid(e.target.value)}
              />
            </div>

            <div className="space-y-2">
              <Label htmlFor="download-model-path">{t('catalog.downloadPathOptional')}</Label>
              <Input
                id="download-model-path"
                placeholder={t('library.newPathPlaceholder')}
                value={downloadPath}
                onChange={(e) => setDownloadPath(e.target.value)}
              />
              <p className="text-xs text-muted-foreground">
                {t('catalog.pathHint')}
              </p>
            </div>
          </div>

          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setDownloadDialogOpen(false)}
              disabled={importing}
            >
              {t('common.cancel')}
            </Button>
            <Button onClick={handleDownloadConfirm} disabled={!selectedResult || importing}>
              <Download className="mr-1.5 h-4 w-4" />
              {importing ? t('catalog.downloading') : t('catalog.startDownload')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
