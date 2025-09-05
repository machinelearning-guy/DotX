export interface PlotConfig {
  showForwardStrand: boolean
  showReverseStrand: boolean
  swapAxes: boolean
  reverseComplementY: boolean
  theme: Theme
  strandFilter: StrandFilter
}

export interface ViewPort {
  x: number
  y: number
  zoom: number
  width: number
  height: number
}

export interface PlotStats {
  anchorCount: number
  fps: number
  visibleAnchors: number
  verificationStatus: 'none' | 'partial' | 'complete'
}

export type Theme = 'light' | 'dark'
export type StrandFilter = 'both' | 'forward' | 'reverse'
export type ExportFormat = 'svg' | 'png' | 'pdf'

export interface Anchor {
  queryStart: number
  queryEnd: number
  targetStart: number
  targetEnd: number
  strand: '+' | '-'
  identity?: number
}

export interface Tooltip {
  x: number
  y: number
  content: string
  visible: boolean
}

export interface ROISelection {
  startX: number
  startY: number
  endX: number
  endY: number
  active: boolean
}

export interface Preset {
  name: string
  description: string
  config: {
    engine: string
    parameters: Record<string, any>
  }
}

export const PRESETS: Preset[] = [
  {
    name: 'Bacterial',
    description: 'Small genomes (bacteria) - high dot density',
    config: {
      engine: 'minimap2',
      parameters: { preset: 'asm5' }
    }
  },
  {
    name: 'Large Contigs',
    description: 'Large contigs/chromosomes with frequency masking',
    config: {
      engine: 'strobemer',
      parameters: { masking: true }
    }
  },
  {
    name: 'Reads→Ref (ONT)',
    description: 'Oxford Nanopore reads to reference',
    config: {
      engine: 'minimap2',
      parameters: { preset: 'map-ont' }
    }
  },
  {
    name: 'Self-dot',
    description: 'Self-alignment for structure exploration',
    config: {
      engine: 'syncmer',
      parameters: { seedOnly: true, alpha: 0.8 }
    }
  }
]