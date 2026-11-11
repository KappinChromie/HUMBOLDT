import * as echarts from 'echarts';
import {
  CompareSnapshotsResponse,
  ComparisonStats,
  DataPoint,
  SnapshotSummary,
  compareSnapshots,
  getSnapshotCsvUrl,
  listSnapshots,
  saveSnapshot,
} from '../services/api';

export interface ComparisonManager {
  chart: echarts.ECharts;
  updateComparison: (data: CompareSnapshotsResponse) => void;
  exportPng: (filename?: string) => void;
  resize: () => void;
}

/**
 * Formats a timestamp into human-readable UTC or relative days.
 */
function formatTimeLabel(ts: number, mode: 'calendar' | 'relative'): string {
  if (mode === 'relative') {
    const days = ts / (1000 * 60 * 60 * 24);
    return `Day ${days.toFixed(1)}`;
  }
  const d = new Date(ts);
  return `${d.getUTCMonth() + 1}/${d.getUTCDate()} ${d.getUTCHours().toString().padStart(2, '0')}:${d.getUTCMinutes().toString().padStart(2, '0')}`;
}

/**
 * Initializes the Apache ECharts comparison visualization instance.
 */
export function initComparisonChart(containerId: string): ComparisonManager {
  const container = document.getElementById(containerId);
  if (!container) {
    throw new Error(`Element #${containerId} not found`);
  }

  const chart = echarts.init(container, 'dark', { renderer: 'canvas' });

  const initialOption: echarts.EChartsOption = {
    backgroundColor: '#161b22',
    tooltip: {
      trigger: 'axis',
      backgroundColor: '#21262d',
      borderColor: '#30363d',
      textStyle: { color: '#c9d1d9', fontFamily: 'monospace', fontSize: 11 },
    },
    legend: {
      top: 4,
      right: 12,
      textStyle: { color: '#8b949e', fontSize: 10, fontFamily: 'monospace' },
    },
    grid: {
      top: 36,
      right: 16,
      bottom: 24,
      left: 36,
      containLabel: true,
    },
    xAxis: {
      type: 'category',
      data: [],
      axisLine: { lineStyle: { color: '#30363d' } },
      axisLabel: { color: '#8b949e', fontSize: 10, fontFamily: 'monospace' },
    },
    yAxis: {
      type: 'value',
      name: 'Value',
      nameTextStyle: { color: '#8b949e', fontSize: 9, fontFamily: 'monospace' },
      axisLine: { lineStyle: { color: '#30363d' } },
      splitLine: { lineStyle: { color: '#21262d' } },
      axisLabel: { color: '#8b949e', fontSize: 10, fontFamily: 'monospace' },
    },
    series: [
      {
        name: 'Series A (Baseline)',
        type: 'line',
        smooth: true,
        data: [],
        itemStyle: { color: '#58a6ff' },
        lineStyle: { width: 2, color: '#58a6ff' },
      },
      {
        name: 'Series B (Comparison)',
        type: 'line',
        smooth: true,
        data: [],
        itemStyle: { color: '#f0883e' },
        lineStyle: { width: 2, color: '#f0883e' },
      },
    ],
  };

  chart.setOption(initialOption);

  window.addEventListener('resize', () => {
    chart.resize();
  });

  return {
    chart,
    updateComparison: (data: CompareSnapshotsResponse) => {
      const mode = data.mode;
      const count = Math.max(data.series_a.length, data.series_b.length);
      const labels: string[] = [];

      for (let i = 0; i < count; i++) {
        const pt = data.series_a[i] || data.series_b[i];
        if (pt) {
          labels.push(formatTimeLabel(pt.timestamp, mode));
        } else {
          labels.push(`Pt ${i + 1}`);
        }
      }

      chart.setOption({
        legend: {
          data: [data.snapshot_a_name, data.snapshot_b_name],
        },
        xAxis: {
          name: mode === 'relative' ? 'Relative Elapsed Days' : 'Calendar Time',
          nameLocation: 'middle',
          nameGap: 20,
          nameTextStyle: { color: '#8b949e', fontSize: 9, fontFamily: 'monospace' },
          data: labels,
        },
        series: [
          {
            name: data.snapshot_a_name,
            data: data.series_a.map((p) => p.value),
          },
          {
            name: data.snapshot_b_name,
            data: data.series_b.map((p) => p.value),
          },
        ],
      });
    },
    exportPng: (filename = 'humboldt_snapshot_comparison.png') => {
      const url = chart.getDataURL({
        type: 'png',
        pixelRatio: 2,
        backgroundColor: '#161b22',
      });
      const link = document.createElement('a');
      link.download = filename;
      link.href = url;
      link.click();
    },
    resize: () => chart.resize(),
  };
}

/**
 * Controller class wiring the comparison controls, real-time variance banner,
 * snapshot freezing, and export routines.
 */
export class SnapshotComparisonUI {
  private manager: ComparisonManager | null = null;
  private currentSnapshots: SnapshotSummary[] = [];
  private activeMode: 'calendar' | 'relative' = 'calendar';
  private selectedSnapA: string | null = null;
  private selectedSnapB: string | null = null;

  constructor(
    private chartContainerId: string,
    private bannerElId: string,
    private selectAId: string,
    private selectBId: string,
    private modeToggleId: string
  ) {}

  public async initialize(): Promise<void> {
    this.manager = initComparisonChart(this.chartContainerId);
    this.bindEvents();
    await this.refreshSnapshotList();
  }

  public async refreshSnapshotList(): Promise<void> {
    this.currentSnapshots = await listSnapshots();
    const selectA = document.getElementById(this.selectAId) as HTMLSelectElement | null;
    const selectB = document.getElementById(this.selectBId) as HTMLSelectElement | null;

    if (!selectA || !selectB) return;

    selectA.innerHTML = '<option value="">Select Snapshot A (Baseline)</option>';
    selectB.innerHTML = '<option value="">Select Snapshot B (Target)</option>';

    for (const snap of this.currentSnapshots) {
      const optA = document.createElement('option');
      optA.value = snap.id;
      optA.textContent = `${snap.name} (${snap.metric_name})`;
      selectA.appendChild(optA);

      const optB = document.createElement('option');
      optB.value = snap.id;
      optB.textContent = `${snap.name} (${snap.metric_name})`;
      selectB.appendChild(optB);
    }

    if (this.currentSnapshots.length >= 2) {
      selectA.selectedIndex = 1;
      selectB.selectedIndex = 2;
      this.selectedSnapA = this.currentSnapshots[0].id;
      this.selectedSnapB = this.currentSnapshots[1].id;
      await this.runComparison();
    } else if (this.currentSnapshots.length === 1) {
      selectA.selectedIndex = 1;
      this.selectedSnapA = this.currentSnapshots[0].id;
    }
  }

  public async runComparison(): Promise<void> {
    if (!this.selectedSnapA || !this.selectedSnapB || !this.manager) {
      return;
    }

    try {
      const comp = await compareSnapshots(this.selectedSnapA, this.selectedSnapB, this.activeMode);
      this.manager.updateComparison(comp);
      this.renderVarianceBanner(comp.stats);
    } catch (err) {
      console.warn('[HUMBOLDT] Comparison failed:', err);
    }
  }

  private renderVarianceBanner(stats: ComparisonStats): void {
    const banner = document.getElementById(this.bannerElId);
    if (!banner) return;

    const formatDiff = (v: number) => {
      const prefix = v > 0 ? '+' : '';
      return `${prefix}${v.toFixed(2)}`;
    };

    const nrmsdPct = (stats.nrmsd * 100).toFixed(1);

    banner.innerHTML = `
      <div class="variance-item">
        <span class="var-label">Δ MEAN</span>
        <span class="var-val ${stats.delta_mean >= 0 ? 'pos' : 'neg'}">${formatDiff(stats.delta_mean)}</span>
      </div>
      <div class="variance-item">
        <span class="var-label">Δ MAX</span>
        <span class="var-val ${stats.delta_max >= 0 ? 'pos' : 'neg'}">${formatDiff(stats.delta_max)}</span>
      </div>
      <div class="variance-item">
        <span class="var-label">Δ MIN</span>
        <span class="var-val ${stats.delta_min >= 0 ? 'pos' : 'neg'}">${formatDiff(stats.delta_min)}</span>
      </div>
      <div class="variance-item">
        <span class="var-label">NRMSD</span>
        <span class="var-val accent">${nrmsdPct}%</span>
      </div>
    `;
  }

  private bindEvents(): void {
    const selectA = document.getElementById(this.selectAId) as HTMLSelectElement | null;
    const selectB = document.getElementById(this.selectBId) as HTMLSelectElement | null;
    const modeBtn = document.getElementById(this.modeToggleId);

    if (selectA) {
      selectA.addEventListener('change', async () => {
        this.selectedSnapA = selectA.value || null;
        await this.runComparison();
      });
    }

    if (selectB) {
      selectB.addEventListener('change', async () => {
        this.selectedSnapB = selectB.value || null;
        await this.runComparison();
      });
    }

    if (modeBtn) {
      modeBtn.addEventListener('click', async () => {
        this.activeMode = this.activeMode === 'calendar' ? 'relative' : 'calendar';
        modeBtn.textContent =
          this.activeMode === 'calendar' ? 'MODE: CALENDAR' : 'MODE: RELATIVE';
        await this.runComparison();
      });
    }
  }

  public exportCurrentPng(): void {
    if (this.manager) {
      this.manager.exportPng('humboldt_comparison_export.png');
    }
  }

  public exportSelectedCsv(): void {
    const id = this.selectedSnapA || this.selectedSnapB;
    if (id) {
      window.open(getSnapshotCsvUrl(id), '_blank');
    }
  }

  public resize(): void {
    this.manager?.resize();
  }

  public async freezeSnapshotFromLive(name: string, layer: string, metric: string, points: DataPoint[]): Promise<SnapshotSummary> {
    const snap = await saveSnapshot({
      name,
      layer,
      metric_name: metric,
      data_points: points,
    });
    await this.refreshSnapshotList();
    return snap;
  }
}
