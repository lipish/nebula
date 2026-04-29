import { create } from 'zustand'
import type { ModelLoadRequest, ModelSearchResult } from '@/lib/types'

type Step = 'source' | 'search' | 'hardware' | 'engine' | 'review'

interface LoadModelState {
  open: boolean
  step: Step
  source: 'huggingface' | 'modelscope' | 'manual' | 'template'
  searchQuery: string
  selectedModel: ModelSearchResult | null
  form: ModelLoadRequest
  selectedNode: string | null
  selectedGpuIndices: number[]
  
  setOpen: (open: boolean) => void
  setStep: (step: Step) => void
  setSource: (source: LoadModelState['source']) => void
  setSearchQuery: (query: string) => void
  setSelectedModel: (model: ModelSearchResult | null) => void
  updateForm: (updates: Partial<ModelLoadRequest>) => void
  setHardware: (nodeId: string | null, gpuIndices: number[]) => void
  reset: () => void
}

const INITIAL_FORM: ModelLoadRequest = {
  model_name: '',
  model_uid: '',
  replicas: 1,
  config: {},
  engine_type: 'vllm',
}

export const useLoadModelStore = create<LoadModelState>((set) => ({
  open: false,
  step: 'source',
  source: 'huggingface',
  searchQuery: '',
  selectedModel: null,
  form: INITIAL_FORM,
  selectedNode: null,
  selectedGpuIndices: [],

  setOpen: (open) => set({ open }),
  setStep: (step) => set({ step }),
  setSource: (source) => set({ source }),
  setSearchQuery: (searchQuery) => set({ searchQuery }),
  setSelectedModel: (selectedModel) => set({ selectedModel }),
  updateForm: (updates) => set((state) => ({ form: { ...state.form, ...updates } })),
  setHardware: (selectedNode, selectedGpuIndices) => set({ selectedNode, selectedGpuIndices }),
  reset: () => set({
    step: 'source',
    source: 'huggingface',
    searchQuery: '',
    selectedModel: null,
    form: INITIAL_FORM,
    selectedNode: null,
    selectedGpuIndices: [],
  }),
}))
